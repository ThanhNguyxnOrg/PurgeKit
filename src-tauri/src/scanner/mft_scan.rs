use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use mft::attribute::header::ResidentialHeader;
use mft::attribute::{MftAttributeContent, MftAttributeType};
use mft::{MftEntry, MftParser};
use ntfs::{Ntfs, NtfsReadSeek};

use super::disk_scan::{
    attribute_letters, finalize_scan, DiskScanProgress, DiskScanResult, DiskVolumeStat, FlatNode,
};

const MFT_DUMP_VERSION: u32 = 2;
const WIZTREE_FOOTER_MAGIC: &[u8; 8] = b"VOLDATA2";
const WIZTREE_FOOTER_SIZE: u64 = 0x69;
const VOLUME_BLOCK_SIZE: u64 = 64 * 1024;

/// Volume handles require sector-aligned reads. `binrw` (used by the ntfs
/// crate for the boot sector) performs tiny 3–8 byte reads, which Windows
/// rejects with ERROR_INVALID_PARAMETER. Serve arbitrary reads from an
/// aligned 64 KB block cache instead.
struct AlignedVolumeReader {
    file: File,
    position: u64,
    block_start: u64,
    block: Vec<u8>,
}

impl AlignedVolumeReader {
    fn new(file: File) -> Self {
        Self {
            file,
            position: 0,
            block_start: u64::MAX,
            block: Vec::new(),
        }
    }
}

impl Read for AlignedVolumeReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut written = 0usize;
        while written < buf.len() {
            let block_start =
                (self.position / VOLUME_BLOCK_SIZE) * VOLUME_BLOCK_SIZE;
            if self.block_start != block_start {
                use std::io::{Seek, SeekFrom};
                self.file.seek(SeekFrom::Start(block_start))?;
                self.block.resize(VOLUME_BLOCK_SIZE as usize, 0);
                let read = self.file.read(&mut self.block)?;
                self.block.truncate(read);
                self.block_start = block_start;
                if read == 0 {
                    break;
                }
            }
            let offset = (self.position - block_start) as usize;
            if offset >= self.block.len() {
                break;
            }
            let count = (buf.len() - written).min(self.block.len() - offset);
            buf[written..written + count]
                .copy_from_slice(&self.block[offset..offset + count]);
            written += count;
            self.position += count as u64;
        }
        Ok(written)
    }
}

impl Seek for AlignedVolumeReader {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let new_position = match position {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::Current(offset) => self.position as i128 + offset as i128,
            SeekFrom::End(offset) => {
                let length = self.file.metadata()?.len() as i128;
                length + offset as i128
            }
        };
        if new_position < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "negative seek position",
            ));
        }
        self.position = new_position as u64;
        Ok(self.position)
    }
}

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
        let Some(record) = record_from_entry(&entry) else {
            continue;
        };

        if record.is_dir {
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

    let mut parser = MftParser::from_read_seek(
        BufReader::with_capacity(1_048_576, file),
        Some(data_length.saturating_sub(data_offset)),
    )
    .map_err(|error| format!("Could not decode the MFT file: {}", error))?;
    let total_records = parser.get_entry_count();
    if total_records == 0 {
        return Err("The MFT file contains no records.".to_string());
    }

    let mut records: HashMap<u64, MftRecord> = HashMap::new();
    let mut parsed_files = 0u64;
    let mut parsed_dirs = 0u64;
    let mut scanned_bytes = 0u64;
    let mut errors = 0u64;

    for record_number in 0..total_records {
        if record_number % 512 == 0 && cancel.load(Ordering::Relaxed) {
            return Err("Disk scan cancelled".to_string());
        }
        let entry = match parser.get_entry(record_number) {
            Ok(entry) => entry,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let Some(record) = record_from_entry(&entry) else {
            continue;
        };
        if record.is_dir {
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

    let tree = build_mft_tree(records, root, &target_components)?;
    append_progress(
        &mut on_progress,
        root,
        tree.file_total,
        tree.dir_total,
        scanned_bytes,
    );

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
    fn aligned_volume_reader_serves_small_unaligned_reads() {
        let path = std::env::temp_dir().join(format!(
            "purgekit_aligned_volume_{}.bin",
            std::process::id()
        ));
        let data: Vec<u8> = (0..200_000).map(|index| (index % 251) as u8).collect();
        fs::write(&path, &data).unwrap();
        let file = File::open(&path).unwrap();
        let mut reader = AlignedVolumeReader::new(file);

        let mut small = [0u8; 3];
        reader.read_exact(&mut small).unwrap();
        assert_eq!(&small, &data[0..3]);

        reader.seek(SeekFrom::Start(100)).unwrap();
        let mut block = vec![0u8; 1024];
        reader.read_exact(&mut block).unwrap();
        assert_eq!(&block, &data[100..1124]);

        reader.seek(SeekFrom::Start(65530)).unwrap();
        let mut across = vec![0u8; 20];
        reader.read_exact(&mut across).unwrap();
        assert_eq!(&across, &data[65530..65550]);

        let _ = fs::remove_file(&path);
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
