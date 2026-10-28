use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use mft::attribute::MftAttributeType;
use mft::MftEntry;
use ntfs::{Ntfs, NtfsReadSeek};

use super::disk_scan::{
    attribute_letters, finalize_scan, DiskScanProgress, DiskScanResult, FlatNode,
};

const MFT_DUMP_VERSION: u32 = 2;

#[derive(Debug, Serialize, Deserialize)]
struct MftDumpFile {
    version: u32,
    root_path: String,
    nodes: Vec<FlatNode>,
    #[serde(default)]
    volume_total_bytes: u64,
    #[serde(default)]
    volume_free_bytes: u64,
}

#[derive(Serialize)]
struct MftDumpRef<'a> {
    version: u32,
    root_path: &'a str,
    nodes: &'a [FlatNode],
    volume_total_bytes: u64,
    volume_free_bytes: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MftDumpInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub modified_epoch_ms: Option<u64>,
}

#[cfg(not(test))]
fn dump_dir() -> PathBuf {
    crate::db::get_db_path()
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mft_dumps")
}

#[cfg(test)]
fn dump_dir() -> PathBuf {
    std::env::temp_dir().join("purgekit_mft_dump_tests")
}

fn dump_label(root: &str) -> String {
    let trimmed = root.trim_end_matches(['\\', '/']);
    let label = Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("volume");
    let label: String = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    if label.is_empty() {
        "volume".to_string()
    } else {
        label
    }
}

fn next_dump_path(root: &str) -> PathBuf {
    let dir = dump_dir();
    let _ = fs::create_dir_all(&dir);
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let base = format!("{}_{}", dump_label(root), stamp);
    let mut path = dir.join(format!("{}.pkmft", base));
    let mut suffix = 1;
    while path.exists() {
        path = dir.join(format!("{}_{}.pkmft", base, suffix));
        suffix += 1;
    }
    path
}

fn write_mft_dump(path: &Path, root_path: &str, nodes: &[FlatNode]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create the MFT dump folder: {}", error))?;
    }
    let file = File::create(path)
        .map_err(|error| format!("Could not create the MFT dump: {}", error))?;
    let (volume_total_bytes, volume_free_bytes) =
        crate::winutil::volume_space(root_path).unwrap_or((0, 0));
    let mut encoder = GzEncoder::new(file, Compression::default());
    let dump = MftDumpRef {
        version: MFT_DUMP_VERSION,
        root_path,
        nodes,
        volume_total_bytes,
        volume_free_bytes,
    };
    serde_json::to_writer(&mut encoder, &dump)
        .map_err(|error| format!("Could not serialize the MFT dump: {}", error))?;
    encoder
        .finish()
        .map_err(|error| format!("Could not finish the MFT dump: {}", error))?;
    Ok(())
}


#[derive(Debug)]
struct MftRecord {
    name: String,
    parent: u64,
    is_dir: bool,
    size_bytes: u64,
    allocated_bytes: u64,
    modified_epoch_ms: Option<u64>,
    attributes: String,
}

fn parse_scan_target(root: &str) -> Option<(char, Vec<String>)> {
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

    let mut target_components = Vec::new();
    for component in components {
        match component {
            Component::Normal(name) => target_components.push(name.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(((drive as char).to_ascii_uppercase(), target_components))
}

fn find_subtree_root(
    records: &HashMap<u64, MftRecord>,
    children_by_parent: &HashMap<u64, Vec<u64>>,
    target_components: &[String],
) -> Option<u64> {
    let mut current = 5u64;
    for component in target_components {
        let child = children_by_parent.get(&current)?.iter().copied().find(|record_id| {
            records
                .get(record_id)
                .map(|record| record.is_dir && record.name.eq_ignore_ascii_case(component))
                .unwrap_or(false)
        })?;
        current = child;
    }
    Some(current)
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
    duplicate_include_date: bool,
    cancel: &AtomicBool,
    on_progress: F,
) -> Result<DiskScanResult, String>
where
    F: FnMut(DiskScanProgress),
{
    scan_ntfs_mft_inner(
        root,
        max_depth,
        max_children,
        find_duplicates,
        duplicate_include_date,
        cancel,
        None,
        on_progress,
    )
}

pub fn dump_ntfs_mft<F>(
    root: &str,
    max_depth: u32,
    max_children: usize,
    find_duplicates: bool,
    duplicate_include_date: bool,
    cancel: &AtomicBool,
    on_progress: F,
) -> Result<(DiskScanResult, PathBuf), String>
where
    F: FnMut(DiskScanProgress),
{
    let dump_path = next_dump_path(root);
    let result = scan_ntfs_mft_inner(
        root,
        max_depth,
        max_children,
        find_duplicates,
        duplicate_include_date,
        cancel,
        Some(&dump_path),
        on_progress,
    )?;
    Ok((result, dump_path))
}

fn scan_ntfs_mft_inner<F>(
    root: &str,
    max_depth: u32,
    max_children: usize,
    find_duplicates: bool,
    duplicate_include_date: bool,
    cancel: &AtomicBool,
    dump_path: Option<&Path>,
    mut on_progress: F,
) -> Result<DiskScanResult, String>
where
    F: FnMut(DiskScanProgress),
{
    let started = Instant::now();
    let (drive, target_components) = parse_scan_target(root)
        .ok_or_else(|| "MFT scan requires a drive root or folder on a local volume".to_string())?;
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

        let entry = match MftEntry::from_buffer(buffer, record_number) {
            Ok(entry) => entry,
            Err(_) => {
                buffer = vec![0u8; record_size];
                errors += 1;
                continue;
            }
        };
        // from_buffer consumes the record; allocate the next read buffer only
        // after the parsed entry no longer needs it.
        buffer = vec![0u8; record_size];
        if !entry.is_allocated() {
            continue;
        }

        let Some(name_attr) = entry.find_best_name_attribute() else {
            continue;
        };
        let standard_info = entry
            .iter_attributes_matching(Some(vec![MftAttributeType::StandardInformation]))
            .filter_map(Result::ok)
            .find_map(|attribute| attribute.data.into_standard_info());
        let modified_epoch_ms = standard_info
            .as_ref()
            .map(|info| info.modified)
            .unwrap_or(name_attr.modified)
            .as_millisecond();
        let modified_epoch_ms = u64::try_from(modified_epoch_ms).ok();
        let attributes = attribute_letters(
            standard_info
                .as_ref()
                .map(|info| info.file_flags.bits())
                .unwrap_or_else(|| name_attr.flags.bits()),
        );
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
            modified_epoch_ms,
            attributes,
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

    let mut children_by_parent: HashMap<u64, Vec<u64>> = HashMap::new();
    for (record_number, record) in &records {
        children_by_parent
            .entry(record.parent)
            .or_default()
            .push(*record_number);
    }
    for children in children_by_parent.values_mut() {
        children.sort_unstable();
    }

    let root_record_id = find_subtree_root(&records, &children_by_parent, &target_components)
        .ok_or_else(|| "The selected folder was not found in the MFT".to_string())?;
    let Some(root_record) = records.remove(&root_record_id) else {
        return Err("MFT root record was not found".to_string());
    };
    if !root_record.is_dir {
        return Err("MFT root record is not a directory".to_string());
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
        modified_epoch_ms: root_record.modified_epoch_ms,
        created_epoch_ms: None,
        accessed_epoch_ms: None,
        attributes: root_record.attributes.clone(),
        file_count: 0,
        extension: String::new(),
    }];

    let mut queue: VecDeque<(u64, usize, String)> = VecDeque::new();
    queue.push_back((root_record_id, 0, root_path.clone()));
    let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
    visited.insert(root_record_id);
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
                modified_epoch_ms: record.modified_epoch_ms,
                created_epoch_ms: None,
                accessed_epoch_ms: None,
                attributes: record.attributes.clone(),
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

    if let Some(dump_path) = dump_path {
        write_mft_dump(dump_path, &root_path, &nodes)?;
    }

    Ok(finalize_scan(
        nodes,
        root_path,
        max_depth,
        max_children,
        find_duplicates,
        duplicate_include_date,
        false,
        errors,
        started,
        "mft",
    ))
}

fn modified_millis(time: std::io::Result<std::time::SystemTime>) -> Option<u64> {
    time.ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
}

pub fn dump_directory() -> PathBuf {
    dump_dir()
}

pub fn list_mft_dumps() -> Vec<MftDumpInfo> {
    let Ok(entries) = fs::read_dir(dump_dir()) else {
        return Vec::new();
    };
    let mut dumps: Vec<MftDumpInfo> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("pkmft") {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            Some(MftDumpInfo {
                path: path.to_string_lossy().to_string(),
                name: entry.file_name().to_string_lossy().to_string(),
                size_bytes: metadata.len(),
                modified_epoch_ms: modified_millis(metadata.modified()),
            })
        })
        .collect();
    dumps.sort_by(|left, right| right.modified_epoch_ms.cmp(&left.modified_epoch_ms));
    dumps.truncate(100);
    dumps
}

fn validate_dump_path(path: &Path) -> Result<(), String> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("pkmft") {
        return Err("Select a PurgeKit .pkmft dump.".to_string());
    }
    let dir = dump_dir();
    let dir = dir.canonicalize().unwrap_or(dir);
    let file = path
        .canonicalize()
        .map_err(|error| format!("Could not open the MFT dump: {}", error))?;
    if !file.starts_with(&dir) {
        return Err("MFT dumps can only be loaded from the PurgeKit dump folder.".to_string());
    }
    Ok(())
}

pub fn load_mft_dump(
    path: &str,
    max_depth: u32,
    max_children: usize,
    find_duplicates: bool,
    duplicate_include_date: bool,
    cancel: &AtomicBool,
) -> Result<DiskScanResult, String> {
    let started = Instant::now();
    let path = Path::new(path);
    validate_dump_path(path)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Disk scan cancelled".to_string());
    }

    let file = File::open(path)
        .map_err(|error| format!("Could not open the MFT dump: {}", error))?;
    let decoder = GzDecoder::new(file);
    let dump: MftDumpFile = serde_json::from_reader(decoder)
        .map_err(|error| format!("Invalid or truncated MFT dump: {}", error))?;
    if dump.version != MFT_DUMP_VERSION {
        return Err(format!(
            "Unsupported MFT dump version {} (expected {}).",
            dump.version, MFT_DUMP_VERSION
        ));
    }
    if dump.root_path.trim().is_empty() || dump.nodes.is_empty() {
        return Err("The MFT dump has no root folder.".to_string());
    }
    if dump.nodes[0].parent.is_some() || !dump.nodes[0].is_dir {
        return Err("The MFT dump root is malformed.".to_string());
    }
    for (index, node) in dump.nodes.iter().enumerate().skip(1) {
        match node.parent {
            Some(parent) if parent < index => {}
            _ => return Err("The MFT dump contains an invalid parent link.".to_string()),
        }
    }

    let mut result = finalize_scan(
        dump.nodes,
        dump.root_path,
        max_depth.clamp(1, 8),
        max_children.clamp(4, 100),
        find_duplicates,
        duplicate_include_date,
        false,
        0,
        started,
        "mft-dump",
    );
    if dump.volume_total_bytes > 0 {
        result.volume_total_bytes = dump.volume_total_bytes;
        result.volume_free_bytes = dump.volume_free_bytes;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_record(name: &str, parent: u64, is_dir: bool) -> MftRecord {
        MftRecord {
            name: name.to_string(),
            parent,
            is_dir,
            size_bytes: if is_dir { 0 } else { 10 },
            allocated_bytes: if is_dir { 0 } else { 10 },
            modified_epoch_ms: None,
            attributes: String::new(),
        }
    }

    #[test]
    fn parses_drive_roots_and_folders() {
        assert_eq!(parse_scan_target("C:\\"), Some(('C', vec![])));
        assert_eq!(
            parse_scan_target("d:\\Code"),
            Some(('D', vec!["Code".to_string()]))
        );
        assert_eq!(
            parse_scan_target("C:\\Code\\src\\"),
            Some(('C', vec!["Code".to_string(), "src".to_string()]))
        );
        assert_eq!(parse_scan_target("C:"), None);
        assert_eq!(parse_scan_target("\\\\server\\share\\"), None);
    }

    #[test]
    fn resolves_a_folder_subtree_root() {
        let mut records = HashMap::new();
        records.insert(5, test_record("", 5, true));
        records.insert(10, test_record("Code", 5, true));
        records.insert(20, test_record("src", 10, true));
        records.insert(30, test_record("main.rs", 20, false));
        let mut children = HashMap::new();
        children.insert(5, vec![10]);
        children.insert(10, vec![20]);
        children.insert(20, vec![30]);

        assert_eq!(find_subtree_root(&records, &children, &[]), Some(5));
        assert_eq!(
            find_subtree_root(&records, &children, &["code".to_string()]),
            Some(10)
        );
        assert_eq!(
            find_subtree_root(&records, &children, &["Code".to_string(), "src".to_string()]),
            Some(20)
        );
        assert_eq!(
            find_subtree_root(&records, &children, &["missing".to_string()]),
            None
        );
    }

    #[test]
    fn mft_dump_round_trip_rebuilds_the_tree() {
        let path = dump_dir().join(format!("round_trip_{}.pkmft", std::process::id()));
        let nodes = vec![
            FlatNode {
                name: "DumpTest".to_string(),
                path: "C:\\DumpTest".to_string(),
                parent: None,
                is_dir: true,
                size_bytes: 0,
                allocated_bytes: 0,
                modified_epoch_ms: None,
                created_epoch_ms: None,
                accessed_epoch_ms: None,
                attributes: "D".to_string(),
                file_count: 0,
                extension: String::new(),
            },
            FlatNode {
                name: "Alpha".to_string(),
                path: "C:\\DumpTest\\Alpha".to_string(),
                parent: Some(0),
                is_dir: true,
                size_bytes: 0,
                allocated_bytes: 0,
                modified_epoch_ms: None,
                created_epoch_ms: None,
                accessed_epoch_ms: None,
                attributes: "D".to_string(),
                file_count: 0,
                extension: String::new(),
            },
            FlatNode {
                name: "a.txt".to_string(),
                path: "C:\\DumpTest\\Alpha\\a.txt".to_string(),
                parent: Some(1),
                is_dir: false,
                size_bytes: 10,
                allocated_bytes: 12,
                modified_epoch_ms: Some(1_700_000_000_000),
                created_epoch_ms: None,
                accessed_epoch_ms: None,
                attributes: "A".to_string(),
                file_count: 1,
                extension: "txt".to_string(),
            },
            FlatNode {
                name: "b.txt".to_string(),
                path: "C:\\DumpTest\\b.txt".to_string(),
                parent: Some(0),
                is_dir: false,
                size_bytes: 20,
                allocated_bytes: 24,
                modified_epoch_ms: Some(1_700_000_000_000),
                created_epoch_ms: None,
                accessed_epoch_ms: None,
                attributes: "A".to_string(),
                file_count: 1,
                extension: "txt".to_string(),
            },
        ];

        write_mft_dump(&path, "C:\\DumpTest", &nodes).unwrap();
        let cancel = AtomicBool::new(false);
        let result = load_mft_dump(
            &path.to_string_lossy(),
            4,
            20,
            false,
            false,
            &cancel,
        )
        .unwrap();

        assert_eq!(result.engine, "mft-dump");
        assert_eq!(result.total_files, 2);
        assert_eq!(result.total_dirs, 1);
        assert_eq!(result.total_size, 30);
        assert_eq!(result.total_allocated, 36);
        assert_eq!(result.root.children.len(), 2);
        assert_eq!(result.file_index.as_ref().unwrap().files.len(), 2);
        assert!(list_mft_dumps()
            .iter()
            .any(|dump| dump.path.eq_ignore_ascii_case(&path.to_string_lossy())));

        let _ = fs::remove_file(&path);
    }
}
