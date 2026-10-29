use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use mft::attribute::header::ResidentialHeader;
use mft::attribute::{MftAttributeContent, MftAttributeType};
use mft::{EntryHeader, MftEntry};
use rayon::prelude::*;
use ntfs::{Ntfs, NtfsReadSeek};

use super::disk_scan::{
    attribute_letters, finalize_scan, DiskScanProgress, DiskScanResult, DiskVolumeStat, FlatNode,
};

const MFT_DUMP_VERSION: u32 = 2;
const WIZTREE_FOOTER_MAGIC: &[u8; 8] = b"VOLDATA2";
const WIZTREE_FOOTER_SIZE: u64 = 0x69;
use super::volume_reader::AlignedVolumeReader;

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
    let mut encoder = GzEncoder::new(
        std::io::BufWriter::with_capacity(1_048_576, file),
        Compression::fast(),
    );
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
    attributes: u32,
}

fn record_from_entry(entry: &MftEntry) -> Option<MftRecord> {
    if !entry.is_allocated() {
        return None;
    }
    let name_attr = entry.find_best_name_attribute()?;
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
    let attributes = standard_info
        .as_ref()
        .map(|info| info.file_flags.bits())
        .unwrap_or_else(|| name_attr.flags.bits());
    let name = name_attr.name;
    if name.is_empty() {
        return None;
    }
    // The volume root record names itself "."; keep record 5 so the tree has a
    // root, but discard dot entries elsewhere.
    if (name == "." || name == "..") && entry.header.record_number != 5 {
        return None;
    }
    let is_dir = entry.is_dir();
    let (data_logical, data_allocated) = if is_dir {
        (0, 0)
    } else {
        entry
            .iter_attributes_matching(Some(vec![MftAttributeType::DATA]))
            .filter_map(Result::ok)
            // File size refers to the unnamed $DATA stream; named streams such
            // as $BadClus::$Bad would otherwise inflate sparse system files to
            // the whole volume size.
            .filter(|attribute| attribute.header.name.is_empty())
            .fold((0u64, 0u64), |(logical, allocated), attribute| {
                match &attribute.header.residential_header {
                    ResidentialHeader::NonResident(non_resident) => (
                        logical.saturating_add(non_resident.file_size),
                        allocated.saturating_add(non_resident.allocated_length),
                    ),
                    ResidentialHeader::Resident(_) => {
                        if let MftAttributeContent::AttrX80(data) = &attribute.data {
                            let length = data.data().len() as u64;
                            (
                                logical.saturating_add(length),
                                allocated.saturating_add(length),
                            )
                        } else {
                            (logical, allocated)
                        }
                    }
                }
            })
    };

    Some(MftRecord {
        name,
        parent: name_attr.parent.entry,
        is_dir,
        // FILE_NAME sizes can be stale or zero for hardlinked/system files;
        // prefer the $DATA attributes that actually describe the file.
        size_bytes: if is_dir {
            0
        } else if data_logical > 0 {
            data_logical
        } else {
            name_attr.logical_size
        },
        allocated_bytes: if is_dir {
            0
        } else if data_allocated > 0 {
            data_allocated
        } else if name_attr.physical_size > 0 {
            // physical_size already accounts for compression and sparse runs.
            name_attr.physical_size
        } else {
            name_attr.logical_size
        },
        modified_epoch_ms,
        attributes,
    })
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

struct MftTree {
    root_path: String,
    nodes: Vec<FlatNode>,
    file_total: u64,
    dir_total: u64,
}

fn build_mft_tree(
    mut records: HashMap<u64, MftRecord>,
    root: &str,
    target_components: &[String],
) -> Result<MftTree, String> {
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

    let root_record_id = find_subtree_root(&records, &children_by_parent, target_components)
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
        mft_record: Some(root_record_id),
        attributes: attribute_letters(root_record.attributes),
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
                mft_record: Some(*child_record),
                attributes: attribute_letters(record.attributes),
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

    Ok(MftTree {
        root_path,
        nodes,
        file_total,
        dir_total,
    })
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

    let volume = File::open(&volume_path)
        .map_err(|e| format!("MFT scan requires Administrator access to {}: {}", volume_path, e))?;
    let mut volume = AlignedVolumeReader::new(volume);
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
    let errors = 0u64;
    // Volume attribute reads are more reliable in smaller chunks.
    let chunk_records = (4 * 1024 * 1024 / record_size).max(1) as u64;
    let mut base = 0u64;
    while base < total_records {
        if cancel.load(Ordering::Relaxed) {
            return Err("Disk scan cancelled".to_string());
        }
        let count = (total_records - base).min(chunk_records);
        let mut buffer = vec![0u8; count as usize * record_size];
        mft_data
            .seek(&mut volume, std::io::SeekFrom::Start(base * record_size as u64))
            .map_err(|error| format!("Could not seek the $MFT: {}", error))?;
        mft_data
            .read_exact(&mut volume, &mut buffer)
            .map_err(|error| format!("Could not read the $MFT: {}", error))?;

        let parsed: Vec<Option<MftRecord>> = (0..count as usize)
            .into_par_iter()
            .map(|index| {
                let start = index * record_size;
                let entry = MftEntry::from_buffer(
                    buffer[start..start + record_size].to_vec(),
                    base + index as u64,
                )
                .ok()?;
                record_from_entry(&entry)
            })
            .collect();

        for (index, record) in parsed.into_iter().enumerate() {
            let Some(record) = record else { continue; };
            if record.is_dir {
                parsed_dirs += 1;
            } else {
                parsed_files += 1;
                scanned_bytes = scanned_bytes.saturating_add(record.size_bytes);
            }
            records.insert(base + index as u64, record);
        }

        base += count;
        append_progress(
            &mut on_progress,
            root,
            parsed_files,
            parsed_dirs,
            scanned_bytes,
        );
    }

    let tree = build_mft_tree(records, root, &target_components)?;

    append_progress(
        &mut on_progress,
        root,
        tree.file_total,
        tree.dir_total,
        scanned_bytes,
    );

    if let Some(dump_path) = dump_path {
        write_mft_dump(dump_path, &tree.root_path, &tree.nodes)?;
    }

    Ok(finalize_scan(
        tree.nodes,
        tree.root_path,
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

fn has_mft_record_signature(buffer: &[u8], offset: usize) -> bool {
    buffer
        .get(offset..offset + 4)
        .map(|signature| signature == b"FILE" || signature == b"BAAD")
        .unwrap_or(false)
}

struct WizTreeFooter {
    drive_letter: Option<char>,
    data_length: u64,
    total_bytes: u64,
    free_bytes: u64,
}

fn read_wiztree_footer(
    file: &mut File,
    length: u64,
) -> Result<Option<WizTreeFooter>, String> {
    if length < WIZTREE_FOOTER_SIZE {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(length - WIZTREE_FOOTER_SIZE))
        .map_err(|e| e.to_string())?;
    let mut footer = vec![0u8; WIZTREE_FOOTER_SIZE as usize];
    file.read_exact(&mut footer).map_err(|e| e.to_string())?;
    if footer.get(0..8) != Some(WIZTREE_FOOTER_MAGIC) {
        return Ok(None);
    }

    let read_u64 = |offset: usize| -> u64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&footer[offset..offset + 8]);
        u64::from_le_bytes(bytes)
    };
    let read_u32 = |offset: usize| -> u32 {
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&footer[offset..offset + 4]);
        u32::from_le_bytes(bytes)
    };

    // The 0x60 bytes after the magic are an NTFS_VOLUME_DATA_BUFFER.
    let metadata = 8usize;
    let total_clusters = read_u64(metadata + 16);
    let free_clusters = read_u64(metadata + 24);
    let bytes_per_cluster = read_u32(metadata + 44) as u64;
    let drive_letter = footer
        .last()
        .copied()
        .filter(|byte| byte.is_ascii_alphabetic())
        .map(|byte| (byte as char).to_ascii_uppercase());

    Ok(Some(WizTreeFooter {
        drive_letter,
        data_length: length - WIZTREE_FOOTER_SIZE,
        total_bytes: total_clusters.saturating_mul(bytes_per_cluster),
        free_bytes: free_clusters.saturating_mul(bytes_per_cluster),
    }))
}

pub fn wiztree_mft_drive(path: &str) -> Option<char> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    read_wiztree_footer(&mut file, length)
        .ok()
        .flatten()?
        .drive_letter
}

/// WizTree writes raw `$MFT` records; some versions may prefix the records
/// with a small header. Find the first record-aligned "FILE"/"BAAD" signature
/// instead of assuming the data starts at byte zero.
fn detect_mft_data_offset(file: &mut File, length: u64) -> Result<u64, String> {
    let probe_len = usize::try_from(length.min(1_048_576)).unwrap_or(0);
    let mut probe = vec![0u8; probe_len];
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    file.read_exact(&mut probe).map_err(|e| e.to_string())?;

    if has_mft_record_signature(&probe, 0) {
        return Ok(0);
    }
    for offset in 0..probe_len.saturating_sub(4) {
        if !has_mft_record_signature(&probe, offset) {
            continue;
        }
        for record_size in [1024usize, 4096] {
            let next = offset + record_size;
            if next + 4 <= probe_len {
                if has_mft_record_signature(&probe, next) {
                    return Ok(offset as u64);
                }
            } else if next as u64 >= length {
                return Ok(offset as u64);
            }
        }
    }

    Err(
        "This file does not look like a raw NTFS MFT dump: no FILE/BAAD records were found."
            .to_string(),
    )
}

fn detect_record_size(file: &mut File, offset: u64, length: u64) -> Result<usize, String> {
    if length <= offset {
        return Err("The MFT dump has no record data.".to_string());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut probe = [0u8; 64];
    file.read_exact(&mut probe)
        .map_err(|error| format!("Could not read the first MFT record: {}", error))?;
    let mut cursor = Cursor::new(&probe[..]);
    let header = EntryHeader::from_reader(&mut cursor, 0)
        .map_err(|error| format!("Could not decode the first MFT record: {}", error))?;
    let record_size = header.total_entry_size as usize;
    if record_size == 0
        || record_size > 1_048_576
        || (length - offset) < record_size as u64
    {
        return Err("The MFT dump reports an invalid record size.".to_string());
    }
    Ok(record_size)
}

pub fn load_wiztree_mft<F>(
    path: &str,
    root: &str,
    max_depth: u32,
    max_children: usize,
    find_duplicates: bool,
    duplicate_include_date: bool,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> Result<DiskScanResult, String>
where
    F: FnMut(DiskScanProgress),
{
    let started = Instant::now();
    let (_, target_components) = parse_scan_target(root)
        .ok_or_else(|| "Choose a drive or folder path for this MFT dump.".to_string())?;

    let mut file = File::open(path).map_err(|error| format!("Could not open the MFT file: {}", error))?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    let footer = read_wiztree_footer(&mut file, length)?;
    let (footer_drive, data_length, total_bytes, free_bytes) = match footer {
        Some(footer) => (
            footer.drive_letter,
            footer.data_length,
            footer.total_bytes,
            footer.free_bytes,
        ),
        None => (None, length, 0, 0),
    };
    if let (Some(letter), Some(root_letter)) = (footer_drive, root.chars().next()) {
        if !letter.eq_ignore_ascii_case(&root_letter) {
            return Err(format!(
                "This MFT dump belongs to drive {}:, not {}:.",
                letter, root_letter.to_ascii_uppercase()
            ));
        }
    }
    let data_offset = detect_mft_data_offset(&mut file, data_length)?;
    file.seek(SeekFrom::Start(data_offset))
        .map_err(|e| e.to_string())?;

    let record_size = detect_record_size(&mut file, data_offset, data_length)?;
    let total_records = ((data_length.saturating_sub(data_offset)) / record_size as u64) as usize;
    if total_records == 0 {
        return Err("The MFT file contains no records.".to_string());
    }

    let profile = std::env::var_os("PURGEKIT_MFT_PROFILE").is_some();
    let parse_started = Instant::now();
    let mut records: HashMap<u64, MftRecord> = HashMap::with_capacity(total_records / 2);
    let mut parsed_files = 0u64;
    let mut parsed_dirs = 0u64;
    let mut scanned_bytes = 0u64;
    let errors = 0u64;

    // Parse record chunks in parallel; decoding attributes dominates load time.
    let chunk_records = (64 * 1024 * 1024 / record_size).max(1);
    file.seek(SeekFrom::Start(data_offset))
        .map_err(|e| e.to_string())?;
    let mut base = 0usize;
    while base < total_records {
        if cancel.load(Ordering::Relaxed) {
            return Err("Disk scan cancelled".to_string());
        }
        let count = (total_records - base).min(chunk_records);
        let mut buffer = vec![0u8; count * record_size];
        file.read_exact(&mut buffer)
            .map_err(|error| format!("Could not read the MFT dump: {}", error))?;

        let parsed: Vec<Option<MftRecord>> = (0..count)
            .into_par_iter()
            .map(|index| {
                let start = index * record_size;
                let entry = MftEntry::from_buffer(
                    buffer[start..start + record_size].to_vec(),
                    (base + index) as u64,
                )
                .ok()?;
                record_from_entry(&entry)
            })
            .collect();

        for (index, record) in parsed.into_iter().enumerate() {
            let Some(record) = record else { continue; };
            if record.is_dir {
                parsed_dirs += 1;
            } else {
                parsed_files += 1;
                scanned_bytes = scanned_bytes.saturating_add(record.size_bytes);
            }
            records.insert((base + index) as u64, record);
        }

        base += count;
        append_progress(
            &mut on_progress,
            root,
            parsed_files,
            parsed_dirs,
            scanned_bytes,
        );
    }

    let parse_elapsed = parse_started.elapsed();
    let tree_started = Instant::now();
    let tree = build_mft_tree(records, root, &target_components)?;
    let tree_elapsed = tree_started.elapsed();
    append_progress(
        &mut on_progress,
        root,
        tree.file_total,
        tree.dir_total,
        scanned_bytes,
    );

    let finalize_started = Instant::now();
    let tree_root_path = tree.root_path.clone();
    let mut result = finalize_scan(
        tree.nodes,
        tree.root_path,
        max_depth,
        max_children,
        find_duplicates,
        duplicate_include_date,
        false,
        errors,
        started,
        "mft-file",
    );
    if profile {
        eprintln!(
            "mft-profile parse={}ms tree={}ms finalize={}ms total={}ms",
            parse_elapsed.as_millis(),
            tree_elapsed.as_millis(),
            finalize_started.elapsed().as_millis(),
            started.elapsed().as_millis()
        );
    }
    // A dump may come from another machine; its footer carries the volume size
    // when available.
    result.volume_total_bytes = total_bytes;
    result.volume_free_bytes = free_bytes;
    if let Some(index) = result.file_index.as_mut() {
        index.volumes = if total_bytes > 0 {
            vec![DiskVolumeStat {
                path: tree_root_path,
                total_bytes,
                free_bytes,
            }]
        } else {
            Vec::new()
        };
    }
    Ok(result)
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
    let decoder = GzDecoder::new(BufReader::with_capacity(1_048_576, file));
    let dump: MftDumpFile = serde_json::from_reader(BufReader::with_capacity(1_048_576, decoder))
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
            attributes: 0,
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
    fn reads_wiztree_footer_drive_and_data_length() {
        let path = std::env::temp_dir().join(format!(
            "purgekit_wiztree_footer_{}.mft",
            std::process::id()
        ));
        let mut bytes = vec![0xAAu8; 2048];
        bytes.extend_from_slice(WIZTREE_FOOTER_MAGIC);
        let mut metadata = [0u8; 0x60];
        metadata[16..24].copy_from_slice(&1000u64.to_le_bytes());
        metadata[24..32].copy_from_slice(&250u64.to_le_bytes());
        metadata[44..48].copy_from_slice(&4096u32.to_le_bytes());
        bytes.extend_from_slice(&metadata);
        bytes.push(b'D');
        fs::write(&path, &bytes).unwrap();

        assert_eq!(wiztree_mft_drive(path.to_str().unwrap()), Some('D'));
        let mut file = File::open(&path).unwrap();
        let footer = read_wiztree_footer(&mut file, bytes.len() as u64)
            .unwrap()
            .expect("footer");
        assert_eq!(footer.drive_letter, Some('D'));
        assert_eq!(footer.data_length, 2048);
        assert_eq!(footer.total_bytes, 4_096_000);
        assert_eq!(footer.free_bytes, 1_024_000);

        let _ = fs::remove_file(&path);
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
                mft_record: None,
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
                mft_record: None,
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
                mft_record: None,
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
                mft_record: None,
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
