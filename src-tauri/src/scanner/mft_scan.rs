use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::path::{Component, Path, Prefix};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use mft::MftEntry;
use ntfs::{Ntfs, NtfsReadSeek};

use super::disk_scan::{
    finalize_scan, DiskScanProgress, DiskScanResult, FlatNode,
};

#[derive(Debug)]
struct MftRecord {
    name: String,
    parent: u64,
    is_dir: bool,
    size_bytes: u64,
    allocated_bytes: u64,
}

fn drive_letter(root: &str) -> Option<char> {
    let path = Path::new(root);
    let mut components = path.components();
    let Component::Prefix(prefix) = components.next()? else {
        return None;
    };
    let Prefix::Disk(drive) = prefix.kind() else {
        return None;
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return None;
    }
    if components.next().is_some() {
        return None;
    }
    Some((drive as char).to_ascii_uppercase())
}

fn extension_for(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_lowercase())
        .unwrap_or_else(|| "(none)".to_string())
}

fn append_progress<F>(
    on_progress: &mut F,
    current: &str,
    file_total: u64,
    dir_total: u64,
    bytes: u64,
)
where
    F: FnMut(DiskScanProgress),
{
    on_progress(DiskScanProgress {
        current_path: current.to_string(),
        files: file_total,
        dirs: dir_total,
        bytes,
    });
}

/// Reads the live NTFS Master File Table directly through the volume handle.
/// This is the fast path; it requires an elevated process and a local NTFS volume.
pub fn scan_ntfs_mft<F>(
    root: &str,
    max_depth: u32,
    max_children: usize,
    find_duplicates: bool,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> Result<DiskScanResult, String>
where
    F: FnMut(DiskScanProgress),
{
    let started = Instant::now();
    let drive = drive_letter(root)
        .ok_or_else(|| "MFT scan requires a local drive root such as C:\\".to_string())?;
    let volume_path = format!(r"\\.\{}:", drive);

    let mut volume = File::open(&volume_path)
        .map_err(|e| format!("MFT scan requires Administrator access to {}: {}", volume_path, e))?;
    let ntfs = Ntfs::new(&mut volume).map_err(|e| format!("Not NTFS or unreadable volume: {}", e))?;

    let mft_file = ntfs
        .file(&mut volume, 0)
        .map_err(|e| format!("Could not read the $MFT record: {}", e))?;
    let mft_item = mft_file
        .data(&mut volume, "")
        .ok_or_else(|| "The $MFT file has no default data attribute".to_string())
        .and_then(|item| item.map_err(|e| e.to_string()))?;
    let mft_attribute = mft_item.to_attribute().map_err(|e| e.to_string())?;
    let mut mft_data = mft_attribute.value(&mut volume).map_err(|e| e.to_string())?;
    let mft_len = mft_data.len();
    let record_size = ntfs.file_record_size() as usize;
    if record_size == 0 {
        return Err("NTFS reported a zero MFT record size".to_string());
    }

    let total_records = mft_len / record_size as u64;
    let mut records: HashMap<u64, MftRecord> = HashMap::new();
    let mut parsed_files = 0u64;
    let mut parsed_dirs = 0u64;
    let mut scanned_bytes = 0u64;
    let mut errors = 0u64;
    let mut buffer = vec![0u8; record_size];

    for record_number in 0..total_records {
        if record_number % 512 == 0 && cancel.load(Ordering::Relaxed) {
            return Err("Disk scan cancelled".to_string());
        }

        let offset = record_number * record_size as u64;
        if mft_data
            .seek(&mut volume, std::io::SeekFrom::Start(offset))
            .is_err()
        {
            errors += 1;
            continue;
        }
        if mft_data.read_exact(&mut volume, &mut buffer).is_err() {
            errors += 1;
            continue;
        }

        let entry = match MftEntry::from_buffer(buffer.clone(), record_number) {
            Ok(entry) => entry,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        if !entry.is_allocated() {
            continue;
        }

        let Some(name_attr) = entry.find_best_name_attribute() else {
            continue;
        };
        let name = name_attr.name;
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }

        let is_dir = entry.is_dir();
        let record = MftRecord {
            name,
            parent: name_attr.parent.entry,
            is_dir,
            size_bytes: if is_dir { 0 } else { name_attr.logical_size },
            allocated_bytes: if is_dir {
                0
            } else if name_attr.physical_size > 0 {
                // physical_size already accounts for compression and sparse runs.
                name_attr.physical_size
            } else {
                // Resident data lives inside the MFT record itself.
                name_attr.logical_size
            },
        };

        if is_dir {
            parsed_dirs += 1;
        } else {
            parsed_files += 1;
            scanned_bytes = scanned_bytes.saturating_add(record.size_bytes);
        }
        records.insert(record_number, record);

        if record_number % 4096 == 0 {
            append_progress(
                &mut on_progress,
                root,
                parsed_files,
                parsed_dirs,
                scanned_bytes,
            );
        }
    }

    let Some(root_record) = records.remove(&5) else {
        return Err("MFT root record (5) was not found".to_string());
    };
    if !root_record.is_dir {
        return Err("MFT root record is not a directory".to_string());
    }

    let mut children_by_parent: HashMap<u64, Vec<u64>> = HashMap::new();
    for (record_number, record) in &records {
        if record_number != &5 {
            children_by_parent
                .entry(record.parent)
                .or_default()
                .push(*record_number);
        }
    }
    for children in children_by_parent.values_mut() {
        children.sort_unstable();
    }

    let root_path = root
        .trim_end_matches(|character| character == '\\' || character == '/')
        .to_string()
        + "\\";
    let root_name = if root_path.len() <= 3 {
        root_path.clone()
    } else {
        Path::new(&root_path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| root_path.clone())
    };

    let mut nodes = vec![FlatNode {
        name: root_name,
        path: root_path.clone(),
        parent: None,
        is_dir: true,
        size_bytes: 0,
        allocated_bytes: 0,
        file_count: 0,
        extension: String::new(),
    }];

    let mut queue: VecDeque<(u64, usize, String)> = VecDeque::new();
    queue.push_back((5, 0, root_path.clone()));
    let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
    visited.insert(5);
    let mut file_total = 0u64;
    let mut dir_total = 0u64;

    while let Some((parent_record, parent_index, parent_path)) = queue.pop_front() {
        let Some(children) = children_by_parent.get(&parent_record) else {
            continue;
        };

        for child_record in children {
            if !visited.insert(*child_record) {
                continue;
            }
            let Some(record) = records.get(child_record) else {
                continue;
            };
            let child_path = Path::new(&parent_path)
                .join(&record.name)
                .to_string_lossy()
                .to_string();
            let child_index = nodes.len();
            nodes.push(FlatNode {
                name: record.name.clone(),
                path: child_path.clone(),
                parent: Some(parent_index),
                is_dir: record.is_dir,
                size_bytes: record.size_bytes,
                allocated_bytes: record.allocated_bytes,
                file_count: if record.is_dir { 0 } else { 1 },
                extension: if record.is_dir {
                    String::new()
                } else {
                    extension_for(&child_path)
                },
            });

            if record.is_dir {
                dir_total += 1;
                queue.push_back((*child_record, child_index, child_path));
            } else {
                file_total += 1;
            }
        }
    }

    append_progress(
        &mut on_progress,
        root,
        file_total,
        dir_total,
        scanned_bytes,
    );

    Ok(finalize_scan(
        nodes,
        root_path,
        max_depth,
        max_children,
        find_duplicates,
        false,
        errors,
        started,
        "mft",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_drive_roots() {
        assert_eq!(drive_letter("C:\\"), Some('C'));
        assert_eq!(drive_letter("d:\\"), Some('D'));
        assert_eq!(drive_letter("C:\\Code"), None);
        assert_eq!(drive_letter("\\\\server\\share\\"), None);
    }
}
