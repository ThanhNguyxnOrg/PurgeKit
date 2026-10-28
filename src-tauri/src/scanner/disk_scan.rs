use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use walkdir::WalkDir;

pub struct DiskScanState(pub Arc<AtomicBool>);

#[derive(Debug, Serialize, Clone)]
pub struct DiskScanProgress {
    pub current_path: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiskTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub allocated_bytes: u64,
    pub modified_epoch_ms: Option<u64>,
    pub attributes: String,
    pub file_count: u64,
    pub children: Vec<DiskTreeNode>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiskFile {
    pub name: String,
    pub path: String,
    pub extension: String,
    pub size_bytes: u64,
    pub allocated_bytes: u64,
    pub modified_epoch_ms: Option<u64>,
    pub created_epoch_ms: Option<u64>,
    pub accessed_epoch_ms: Option<u64>,
    #[serde(default)]
    pub mft_record: Option<u64>,
    pub attributes: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FileTypeStat {
    pub extension: String,
    pub size_bytes: u64,
    pub file_count: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DuplicateGroup {
    pub name: String,
    pub size_bytes: u64,
    pub file_count: usize,
    pub files: Vec<DiskFile>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DiskScanResult {
    pub root_path: String,
    pub root: DiskTreeNode,
    pub top_files: Vec<DiskFile>,
    pub file_types: Vec<FileTypeStat>,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_size: u64,
    pub total_allocated: u64,
    pub volume_total_bytes: u64,
    pub volume_free_bytes: u64,
    pub elapsed_ms: u64,
    pub errors: u64,
    pub truncated: bool,
    pub engine: String,
    pub duplicate_mode: String,
    pub protected_roots: Vec<String>,
    #[serde(skip)]
    pub file_index: Option<DiskFileIndex>,
}

pub const MAX_INDEXED_FILES: usize = 400_000;

#[derive(Debug, Clone)]
pub struct CachedDiskFile {
    pub path: String,
    pub size_bytes: u64,
    pub allocated_bytes: u64,
    pub modified_epoch_ms: Option<u64>,
    pub created_epoch_ms: Option<u64>,
    pub accessed_epoch_ms: Option<u64>,
    pub mft_record: Option<u64>,
    pub attributes: String,
}

#[derive(Debug, Clone, Default)]
pub struct DiskVolumeStat {
    pub path: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct DiskFileIndex {
    pub root_path: String,
    pub files: Vec<CachedDiskFile>,
    pub volumes: Vec<DiskVolumeStat>,
    pub complete: bool,
}

#[derive(Debug, Default)]
pub struct DiskFileCache(pub Arc<std::sync::Mutex<HashMap<String, Arc<DiskFileIndex>>>>);

#[derive(Debug, Serialize, Clone)]
pub struct DiskFileSearchResult {
    pub files: Vec<DiskFile>,
    pub total_matches: usize,
    pub indexed_files: usize,
    pub index_complete: bool,
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn compare_paths_case_insensitive(left: &str, right: &str) -> std::cmp::Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
}

fn is_under_prefix(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches(['\\', '/']);
    if prefix.is_empty() {
        return true;
    }
    let path_bytes = path.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    if path_bytes.len() <= prefix_bytes.len() {
        return false;
    }
    if !path_bytes[..prefix_bytes.len()].eq_ignore_ascii_case(prefix_bytes) {
        return false;
    }
    matches!(path_bytes[prefix_bytes.len()], b'\\' | b'/')
}

pub(crate) fn file_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_lowercase())
}

fn cached_matches<'a>(
    index: &'a DiskFileIndex,
    query: &str,
    extension: Option<&str>,
    prefix: Option<&str>,
    filter_mode: Option<&str>,
    include: Option<&str>,
    exclude: Option<&str>,
) -> Vec<&'a CachedDiskFile> {
    let query = query.trim();
    let extension = extension
        .map(|value| value.trim().trim_start_matches('.').to_lowercase())
        .filter(|value| !value.is_empty());
    let prefix = prefix.map(str::trim).filter(|value| !value.is_empty());
    let include = include.map(str::trim).filter(|value| !value.is_empty());
    let exclude = exclude.map(str::trim).filter(|value| !value.is_empty());

    let mut matches = Vec::new();
    for file in &index.files {
        if let Some(prefix) = prefix {
            if !is_under_prefix(&file.path, prefix) {
                continue;
            }
        }
        if let Some(include) = include {
            if !contains_ascii_case_insensitive(&file.path, include) {
                continue;
            }
        }
        if let Some(exclude) = exclude {
            if contains_ascii_case_insensitive(&file.path, exclude) {
                continue;
            }
        }
        let file_extension = file_extension(&file.path);
        if let Some(wanted) = extension.as_deref() {
            if !(*file_extension.as_deref().unwrap_or("(none)")).eq_ignore_ascii_case(wanted) {
                continue;
            }
        }
        if !query.is_empty() {
            let haystack = if filter_mode == Some("path") {
                file.path.as_str()
            } else {
                Path::new(&file.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&file.path)
            };
            if !contains_ascii_case_insensitive(haystack, query) {
                continue;
            }
        }
        matches.push(file);
    }
    matches
}

fn sort_cached_matches(matches: &mut [&CachedDiskFile], sort: &str) {
    match sort {
        "name" => matches.sort_unstable_by(|left, right| {
            let left_name = Path::new(&left.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&left.path);
            let right_name = Path::new(&right.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&right.path);
            compare_paths_case_insensitive(left_name, right_name)
                .then_with(|| compare_paths_case_insensitive(&left.path, &right.path))
        }),
        "modified" => matches
            .sort_unstable_by_key(|file| Reverse(file.modified_epoch_ms.unwrap_or(0))),
        _ => matches
            .sort_unstable_by_key(|file| Reverse(file.allocated_bytes.max(file.size_bytes))),
    }
}

fn disk_file_from_cached(file: &CachedDiskFile) -> DiskFile {
    DiskFile {
        name: Path::new(&file.path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| file.path.clone()),
        path: file.path.clone(),
        extension: file_extension(&file.path).unwrap_or_else(|| "(none)".to_string()),
        size_bytes: file.size_bytes,
        allocated_bytes: file.allocated_bytes,
        modified_epoch_ms: file.modified_epoch_ms,
        created_epoch_ms: file.created_epoch_ms,
        accessed_epoch_ms: file.accessed_epoch_ms,
        mft_record: file.mft_record,
        attributes: file.attributes.clone(),
    }
}

pub fn search_cached_files(
    index: &DiskFileIndex,
    query: &str,
    extension: Option<&str>,
    prefix: Option<&str>,
    filter_mode: Option<&str>,
    include: Option<&str>,
    exclude: Option<&str>,
    limit: usize,
    sort: &str,
) -> DiskFileSearchResult {
    let mut matches = cached_matches(index, query, extension, prefix, filter_mode, include, exclude);
    let total_matches = matches.len();
    sort_cached_matches(&mut matches, sort);
    matches.truncate(limit.clamp(1, 2_000));

    let files = matches
        .into_iter()
        .map(disk_file_from_cached)
        .collect();

    DiskFileSearchResult {
        files,
        total_matches,
        indexed_files: index.files.len(),
        index_complete: index.complete,
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct DiskCsvExport {
    pub content: String,
    pub exported: usize,
    pub folders: usize,
    pub total: usize,
    pub truncated: bool,
}

fn delimited_field(value: &str, delimiter: char) -> String {
    if value.contains(delimiter)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r')
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn formatted_date(epoch_ms: Option<u64>) -> String {
    epoch_ms
        .and_then(|milliseconds| chrono::DateTime::from_timestamp_millis(milliseconds as i64))
        .map(|date| date.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

pub fn export_cached_files_csv(
    index: &DiskFileIndex,
    query: &str,
    extension: Option<&str>,
    prefix: Option<&str>,
    filter_mode: Option<&str>,
    include: Option<&str>,
    exclude: Option<&str>,
    sort: &str,
    limit: usize,
    format: &str,
    include_dates: bool,
    include_folders: bool,
    include_parent_percent: bool,
    include_mft: bool,
    include_drive_info: bool,
) -> DiskCsvExport {
    let mut matches = cached_matches(index, query, extension, prefix, filter_mode, include, exclude);
    let total = matches.len();
    sort_cached_matches(&mut matches, sort);
    let limit = limit.clamp(1, 100_000);
    let truncated = total > limit;
    matches.truncate(limit);

    let delimiter = if format.eq_ignore_ascii_case("tsv") { '\t' } else { ',' };
    let root_weight: u64 = index
        .files
        .iter()
        .map(|file| file.allocated_bytes.max(file.size_bytes))
        .sum();

    let weight_of = |file: &CachedDiskFile| file.allocated_bytes.max(file.size_bytes);
    let share_of = |weight: u64| {
        if root_weight == 0 {
            0.0
        } else {
            (weight as f64 / root_weight as f64) * 100.0
        }
    };

    let mut parent_totals: HashMap<String, u64> = HashMap::new();
    if include_parent_percent {
        for file in &index.files {
            if let Some(parent) = Path::new(&file.path).parent() {
                *parent_totals
                    .entry(parent.to_string_lossy().to_string())
                    .or_insert(0) += weight_of(file);
            }
        }
    }
    let parent_percent_of = |file: &CachedDiskFile| -> String {
        let Some(parent) = Path::new(&file.path).parent() else {
            return String::new();
        };
        let parent_weight = parent_totals
            .get(&parent.to_string_lossy().to_string())
            .copied()
            .unwrap_or(0);
        if parent_weight == 0 {
            String::new()
        } else {
            format!(
                "{:.1}",
                (weight_of(file) as f64 / parent_weight as f64) * 100.0
            )
        }
    };

    let mut folders: std::collections::BTreeMap<String, (u64, u64, usize)> =
        std::collections::BTreeMap::new();

    let mut header = vec!["Name", "Path"];
    if include_mft {
        header.push("MftRecord");
    }
    header.extend(["Type", "Extension", "Modified"]);
    if include_dates {
        header.push("Created");
        header.push("LastAccessed");
    }
    header.push("Attributes");
    header.push("SharePercent");
    if include_parent_percent {
        header.push("ParentPercent");
    }
    header.extend(["LogicalBytes", "AllocatedBytes"]);
    if include_folders {
        header.push("DirectFiles");
    }

    let mut content = String::from("\u{feff}");
    content.push_str(
        &header
            .iter()
            .map(|field| delimited_field(field, delimiter))
            .collect::<Vec<_>>()
            .join(&delimiter.to_string()),
    );
    content.push('\n');

    for file in &matches {
        let weight = weight_of(file);
        let name = Path::new(&file.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&file.path);
        let extension = file_extension(&file.path).unwrap_or_else(|| "(none)".to_string());

        if include_folders {
            if let Some(parent) = Path::new(&file.path).parent() {
                let key = parent.to_string_lossy().to_string();
                let entry = folders.entry(key).or_insert((0, 0, 0));
                entry.0 += file.size_bytes;
                entry.1 += file.allocated_bytes;
                entry.2 += 1;
            }
        }

        let mut row = vec![name.to_string(), file.path.clone()];
        if include_mft {
            row.push(
                file.mft_record
                    .map(|record| record.to_string())
                    .unwrap_or_default(),
            );
        }
        row.extend([
            "File".to_string(),
            extension,
            formatted_date(file.modified_epoch_ms),
        ]);
        if include_dates {
            row.push(formatted_date(file.created_epoch_ms));
            row.push(formatted_date(file.accessed_epoch_ms));
        }
        row.push(file.attributes.clone());
        row.push(format!("{:.1}", share_of(weight)));
        if include_parent_percent {
            row.push(parent_percent_of(file));
        }
        row.push(file.size_bytes.to_string());
        row.push(file.allocated_bytes.to_string());
        if include_folders {
            row.push("1".to_string());
        }
        content.push_str(
            &row.iter()
                .map(|field| delimited_field(field, delimiter))
                .collect::<Vec<_>>()
                .join(&delimiter.to_string()),
        );
        content.push('\n');
    }

    for (path, (logical, allocated, direct_files)) in &folders {
        let name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        let mut row = vec![name, path.clone()];
        if include_mft {
            row.push(String::new());
        }
        row.extend([
            "Folder".to_string(),
            String::new(),
            String::new(),
        ]);
        if include_dates {
            row.push(String::new());
            row.push(String::new());
        }
        row.push(String::from("D"));
        row.push(format!("{:.1}", share_of((*allocated).max(*logical))));
        if include_parent_percent {
            row.push(String::new());
        }
        row.push(logical.to_string());
        row.push(allocated.to_string());
        if include_folders {
            row.push(direct_files.to_string());
        }
        content.push_str(
            &row.iter()
                .map(|field| delimited_field(field, delimiter))
                .collect::<Vec<_>>()
                .join(&delimiter.to_string()),
        );
        content.push('\n');
    }

    if include_drive_info && !index.volumes.is_empty() {
        content.push('\n');
        let drive_header = ["Drive", "TotalBytes", "FreeBytes", "UsedBytes"];
        content.push_str(
            &drive_header
                .iter()
                .map(|field| delimited_field(field, delimiter))
                .collect::<Vec<_>>()
                .join(&delimiter.to_string()),
        );
        content.push('\n');
        for volume in &index.volumes {
            let used = volume.total_bytes.saturating_sub(volume.free_bytes);
            let row = [
                volume.path.clone(),
                volume.total_bytes.to_string(),
                volume.free_bytes.to_string(),
                used.to_string(),
            ];
            content.push_str(
                &row.iter()
                    .map(|field| delimited_field(field, delimiter))
                    .collect::<Vec<_>>()
                    .join(&delimiter.to_string()),
            );
            content.push('\n');
        }
    }

    DiskCsvExport {
        content,
        exported: matches.len(),
        folders: folders.len(),
        total,
        truncated,
    }
}

pub fn export_cached_file_types_csv(index: &DiskFileIndex, format: &str) -> DiskCsvExport {
    let delimiter = if format.eq_ignore_ascii_case("tsv") { '\t' } else { ',' };
    let mut types: HashMap<String, (u64, u64, u64)> = HashMap::new();
    for file in &index.files {
        let extension = file_extension(&file.path).unwrap_or_else(|| "(none)".to_string());
        let entry = types.entry(extension).or_insert((0, 0, 0));
        entry.0 += file.size_bytes;
        entry.1 += file.allocated_bytes;
        entry.2 += 1;
    }

    let mut types: Vec<(String, u64, u64, u64)> = types
        .into_iter()
        .map(|(extension, (logical, allocated, count))| (extension, logical, allocated, count))
        .collect();
    types.sort_by_key(|(_, logical, allocated, _)| {
        std::cmp::Reverse((*allocated).max(*logical))
    });

    let header = ["Extension", "FileCount", "LogicalBytes", "AllocatedBytes"];
    let mut content = String::from("\u{feff}");
    content.push_str(
        &header
            .iter()
            .map(|field| delimited_field(field, delimiter))
            .collect::<Vec<_>>()
            .join(&delimiter.to_string()),
    );
    content.push('\n');
    for (extension, logical, allocated, count) in &types {
        let row = [
            extension.clone(),
            count.to_string(),
            logical.to_string(),
            allocated.to_string(),
        ];
        content.push_str(
            &row.iter()
                .map(|field| delimited_field(field, delimiter))
                .collect::<Vec<_>>()
                .join(&delimiter.to_string()),
        );
        content.push('\n');
    }

    DiskCsvExport {
        content,
        exported: types.len(),
        folders: 0,
        total: types.len(),
        truncated: false,
    }
}


#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FlatNode {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) parent: Option<usize>,
    pub(crate) is_dir: bool,
    pub(crate) size_bytes: u64,
    pub(crate) allocated_bytes: u64,
    pub(crate) modified_epoch_ms: Option<u64>,
    pub(crate) created_epoch_ms: Option<u64>,
    pub(crate) accessed_epoch_ms: Option<u64>,
    #[serde(default)]
    pub(crate) mft_record: Option<u64>,
    pub(crate) attributes: String,
    pub(crate) file_count: u64,
    pub(crate) extension: String,
}

struct TreeBudget {
    remaining: usize,
    truncated: bool,
}

fn allocated_or_logical(path: &Path, metadata: &fs::Metadata) -> u64 {
    let allocated = crate::winutil::allocated_size(path);
    if allocated > 0 {
        allocated
    } else {
        metadata.len()
    }
}

fn modified_millis(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn created_millis(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .created()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

fn accessed_millis(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .accessed()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

pub(crate) fn attribute_letters(flags: u32) -> String {
    const FLAGS: &[(u32, char)] = &[
        (0x0001, 'R'),
        (0x0002, 'H'),
        (0x0004, 'S'),
        (0x0010, 'D'),
        (0x0020, 'A'),
        (0x0100, 'T'),
        (0x0200, 'P'),
        (0x0800, 'C'),
        (0x1000, 'O'),
        (0x4000, 'E'),
        (0x8000, 'I'),
        (0x2000, 'N'),
    ];

    FLAGS
        .iter()
        .filter(|(bit, _)| flags & bit != 0)
        .map(|(_, letter)| *letter)
        .collect()
}

#[cfg(windows)]
fn metadata_attributes(metadata: &fs::Metadata) -> String {
    use std::os::windows::fs::MetadataExt;
    attribute_letters(metadata.file_attributes())
}

#[cfg(not(windows))]
fn metadata_attributes(_metadata: &fs::Metadata) -> String {
    String::new()
}

fn flat_to_tree_node(node: &FlatNode) -> DiskTreeNode {
    DiskTreeNode {
        name: node.name.clone(),
        path: node.path.clone(),
        is_dir: node.is_dir,
        size_bytes: node.size_bytes,
        allocated_bytes: node.allocated_bytes,
        modified_epoch_ms: node.modified_epoch_ms,
        attributes: node.attributes.clone(),
        file_count: node.file_count,
        children: Vec::new(),
    }
}

struct TreeArenaNode {
    node: DiskTreeNode,
    source: usize,
    child_indices: Vec<usize>,
}

fn materialize_tree(index: usize, arena: &[TreeArenaNode]) -> DiskTreeNode {
    let mut node = arena[index].node.clone();
    node.children = arena[index]
        .child_indices
        .iter()
        .map(|child| materialize_tree(*child, arena))
        .collect();
    node
}

/// Spend the tree budget breadth-first so every shallow folder is represented
/// before one huge subtree can consume all remaining nodes.
fn build_tree_breadth_first(
    root_index: usize,
    max_depth: u32,
    max_children: usize,
    nodes: &[FlatNode],
    children: &[Vec<usize>],
    budget: &mut TreeBudget,
) -> DiskTreeNode {
    let mut arena = vec![TreeArenaNode {
        node: flat_to_tree_node(&nodes[root_index]),
        source: root_index,
        child_indices: Vec::new(),
    }];
    let mut queue: VecDeque<(usize, u32)> = VecDeque::new();
    queue.push_back((0, 0));

    while let Some((arena_index, depth)) = queue.pop_front() {
        let source = arena[arena_index].source;
        let node = &nodes[source];
        if !node.is_dir || depth >= max_depth || budget.remaining == 0 {
            continue;
        }

        let mut child_ids = children[source].clone();
        child_ids.sort_by_key(|child| {
            Reverse(nodes[*child].allocated_bytes.max(nodes[*child].size_bytes))
        });
        let total_children = child_ids.len();
        for child in child_ids.into_iter().take(max_children) {
            if budget.remaining == 0 {
                budget.truncated = true;
                break;
            }
            budget.remaining -= 1;
            let child_index = arena.len();
            arena.push(TreeArenaNode {
                node: flat_to_tree_node(&nodes[child]),
                source: child,
                child_indices: Vec::new(),
            });
            arena[arena_index].child_indices.push(child_index);
            queue.push_back((child_index, depth + 1));
        }

        if total_children > max_children {
            budget.truncated = true;
        }
    }

    materialize_tree(0, &arena)
}

pub fn scan_disk_tree<F>(
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
    let expanded_root = crate::winutil::expand_env_strings(root);
    if !Path::new(&expanded_root).is_absolute() {
        return Err(format!(
            "Enter an absolute path to scan (for example C:\\ or C:\\Folder): {}",
            root
        ));
    }
    let root_path = crate::winutil::canonicalize_path_safety(&expanded_root);
    if !root_path.exists() {
        return Err(format!("Path does not exist: {}", root));
    }
    if !root_path.is_dir() {
        return Err(format!("Path is not a directory: {}", root));
    }

    if cancel.load(Ordering::Relaxed) {
        return Err("Disk scan cancelled".to_string());
    }

    let max_depth = max_depth.clamp(1, 8);
    let max_children = max_children.clamp(4, 100);

    let root_metadata =
        fs::symlink_metadata(&root_path).map_err(|e| format!("Failed to read root metadata: {}", e))?;

    let mut nodes = vec![FlatNode {
        name: root_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| root_path.to_string_lossy().to_string()),
        path: root_path.to_string_lossy().to_string(),
        parent: None,
        is_dir: true,
        size_bytes: 0,
        allocated_bytes: 0,
        modified_epoch_ms: modified_millis(&root_metadata),
        created_epoch_ms: created_millis(&root_metadata),
        accessed_epoch_ms: accessed_millis(&root_metadata),
        mft_record: None,
        attributes: metadata_attributes(&root_metadata),
        file_count: 0,
        extension: String::new(),
    }];

    let mut stack: Vec<(usize, usize)> = vec![(0, 0)];
    let mut errors = 0u64;
    let mut scanned_bytes = 0u64;
    let mut file_total = 0u64;
    let mut dir_total = 0u64;

    let walker = WalkDir::new(&root_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            fs::symlink_metadata(entry.path())
                .map(|metadata| !crate::winutil::is_reparse_point(&metadata))
                .unwrap_or(false)
        });

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }

        if nodes.len() % 512 == 0 && cancel.load(Ordering::Relaxed) {
            return Err("Disk scan cancelled".to_string());
        }

        let depth = entry.depth();
        while stack
            .last()
            .map(|(stack_depth, _)| *stack_depth >= depth)
            .unwrap_or(false)
        {
            stack.pop();
        }
        let parent = stack.last().map(|(_, index)| *index).unwrap_or(0);

        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let is_dir = metadata.is_dir();
        let size_bytes = if is_dir { 0 } else { metadata.len() };
        let allocated_bytes = if is_dir {
            0
        } else {
            allocated_or_logical(entry.path(), &metadata)
        };
        let extension = if is_dir {
            String::new()
        } else {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.to_lowercase())
                .unwrap_or_else(|| "(none)".to_string())
        };

        let index = nodes.len();
        nodes.push(FlatNode {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            parent: Some(parent),
            is_dir,
            size_bytes,
            allocated_bytes,
            modified_epoch_ms: modified_millis(&metadata),
            created_epoch_ms: created_millis(&metadata),
            accessed_epoch_ms: accessed_millis(&metadata),
            mft_record: None,
            attributes: metadata_attributes(&metadata),
            file_count: if is_dir { 0 } else { 1 },
            extension: extension.clone(),
        });

        if is_dir {
            dir_total += 1;
            stack.push((depth, index));
        } else {
            file_total += 1;
            scanned_bytes += size_bytes;
        }

        if nodes.len() % 1024 == 0 {
            on_progress(DiskScanProgress {
                current_path: entry.path().to_string_lossy().to_string(),
                files: file_total,
                dirs: dir_total,
                bytes: scanned_bytes,
            });
        }
    }

    Ok(finalize_scan(
        nodes,
        root_path.to_string_lossy().to_string(),
        max_depth,
        max_children,
        find_duplicates,
        duplicate_include_date,
        true,
        errors,
        started,
        "traversal",
    ))
}

fn dedupe_hardlinked_nodes(nodes: &mut [FlatNode]) {
    // Hard links share content, so candidates must share a size. Grouping by
    // size keeps file-handle opens limited to ambiguous files.
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, node) in nodes.iter().enumerate().skip(1) {
        if !node.is_dir && node.size_bytes > 0 {
            by_size.entry(node.size_bytes).or_default().push(index);
        }
    }

    let mut seen: HashSet<(u32, u64)> = HashSet::new();
    for indices in by_size.values().filter(|indices| indices.len() > 1) {
        for &index in indices {
            let Some(identity) = crate::winutil::file_identity(Path::new(&nodes[index].path)) else {
                continue;
            };
            if !seen.insert(identity) {
                nodes[index].size_bytes = 0;
                nodes[index].allocated_bytes = 0;
            }
        }
    }
}

pub(crate) fn finalize_scan(
    mut nodes: Vec<FlatNode>,
    root_path: String,
    max_depth: u32,
    max_children: usize,
    find_duplicates: bool,
    duplicate_include_date: bool,
    dedupe_hardlinks: bool,
    errors: u64,
    started: Instant,
    engine: &str,
) -> DiskScanResult {
    if dedupe_hardlinks {
        dedupe_hardlinked_nodes(&mut nodes);
    }

    // Bottom-up aggregate sizes and counts. Nodes are emitted in parent-before-
    // child order by both scanners.
    for index in (1..nodes.len()).rev() {
        let (parent, size_bytes, allocated_bytes, file_count) = {
            let node = &nodes[index];
            (
                node.parent.unwrap_or(0),
                node.size_bytes,
                node.allocated_bytes,
                node.file_count,
            )
        };
        let parent_node = &mut nodes[parent];
        parent_node.size_bytes += size_bytes;
        parent_node.allocated_bytes += allocated_bytes;
        parent_node.file_count += file_count;
    }

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for index in 1..nodes.len() {
        if let Some(parent) = nodes[index].parent {
            children[parent].push(index);
        }
    }

    let node_budget = 20_000usize;
    let mut budget = TreeBudget {
        remaining: node_budget,
        truncated: false,
    };
    let root_node = build_tree_breadth_first(
        0,
        max_depth,
        max_children,
        &nodes,
        &children,
        &mut budget,
    );
    if nodes.len() > node_budget {
        budget.truncated = true;
    }

    let mut top_heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
    let mut file_types: HashMap<String, (u64, u64)> = HashMap::new();
    let mut total_files = 0u64;
    let mut total_dirs = 0u64;
    let mut indexed_files: Vec<CachedDiskFile> = Vec::new();
    let mut index_complete = true;

    for (index, node) in nodes.iter().enumerate().skip(1) {
        if node.is_dir {
            total_dirs += 1;
            continue;
        }

        total_files += 1;
        if indexed_files.len() < MAX_INDEXED_FILES {
            indexed_files.push(CachedDiskFile {
                path: node.path.clone(),
                size_bytes: node.size_bytes,
                allocated_bytes: node.allocated_bytes,
                modified_epoch_ms: node.modified_epoch_ms,
                created_epoch_ms: node.created_epoch_ms,
                accessed_epoch_ms: node.accessed_epoch_ms,
                mft_record: node.mft_record,
                attributes: node.attributes.clone(),
            });
        } else {
            index_complete = false;
        }

        let weight = node.allocated_bytes.max(node.size_bytes);
        if top_heap.len() < 100 {
            top_heap.push(Reverse((weight, index)));
        } else if let Some(Reverse((smallest, _))) = top_heap.peek() {
            if weight > *smallest {
                top_heap.pop();
                top_heap.push(Reverse((weight, index)));
            }
        }

        let entry_stats = file_types.entry(node.extension.clone()).or_insert((0, 0));
        entry_stats.0 += node.allocated_bytes;
        entry_stats.1 += 1;
    }

    let mut top_file_indices: Vec<usize> = top_heap
        .into_iter()
        .map(|Reverse((_, index))| index)
        .collect();
    top_file_indices.sort_by_key(|index| {
        Reverse(nodes[*index].allocated_bytes.max(nodes[*index].size_bytes))
    });

    let top_files = top_file_indices
        .into_iter()
        .map(|index| {
            let node = &nodes[index];
            DiskFile {
                name: node.name.clone(),
                path: node.path.clone(),
                extension: node.extension.clone(),
                size_bytes: node.size_bytes,
                allocated_bytes: node.allocated_bytes,
                modified_epoch_ms: node.modified_epoch_ms,
                created_epoch_ms: node.created_epoch_ms,
                accessed_epoch_ms: node.accessed_epoch_ms,
                mft_record: node.mft_record,
                attributes: node.attributes.clone(),
            }
        })
        .collect();

    let mut file_types: Vec<FileTypeStat> = file_types
        .into_iter()
        .map(|(extension, (size_bytes, file_count))| FileTypeStat {
            extension,
            size_bytes,
            file_count,
        })
        .collect();
    file_types.sort_by_key(|stat| Reverse(stat.size_bytes));
    file_types.truncate(20);

    let duplicate_groups = if find_duplicates {
        let mut grouped: HashMap<(u64, String, Option<u64>), Vec<usize>> = HashMap::new();
        for (index, node) in nodes.iter().enumerate().skip(1) {
            if node.is_dir || node.size_bytes == 0 {
                continue;
            }
            let modified = if duplicate_include_date {
                Some(node.modified_epoch_ms.unwrap_or(0))
            } else {
                None
            };
            grouped
                .entry((node.size_bytes, node.name.to_lowercase(), modified))
                .or_default()
                .push(index);
        }

        let mut groups: Vec<DuplicateGroup> = grouped
            .into_iter()
            .filter(|(_, indices)| indices.len() > 1)
            .map(|((size_bytes, name, _), mut indices)| {
                indices.sort_by_key(|index| Reverse(nodes[*index].allocated_bytes));
                let file_count = indices.len();
                let files = indices
                    .into_iter()
                    .take(25)
                    .map(|index| {
                        let node = &nodes[index];
                        DiskFile {
                            name: node.name.clone(),
                            path: node.path.clone(),
                            extension: node.extension.clone(),
                            size_bytes: node.size_bytes,
                            allocated_bytes: node.allocated_bytes,
                            modified_epoch_ms: node.modified_epoch_ms,
                            created_epoch_ms: node.created_epoch_ms,
                            accessed_epoch_ms: node.accessed_epoch_ms,
                            mft_record: node.mft_record,
                            attributes: node.attributes.clone(),
                        }
                    })
                    .collect();
                DuplicateGroup {
                    name,
                    size_bytes,
                    file_count,
                    files,
                }
            })
            .collect();
        groups.sort_by_key(|group| {
            Reverse(
                group
                    .size_bytes
                    .saturating_mul(group.file_count.saturating_sub(1) as u64),
            )
        });
        groups.truncate(500);
        groups
    } else {
        Vec::new()
    };

    let total_size = root_node.size_bytes;
    let total_allocated = root_node.allocated_bytes;
    let (volume_total_bytes, volume_free_bytes) =
        crate::winutil::volume_space(&root_path).unwrap_or((0, 0));

    DiskScanResult {
        root_path: root_path.clone(),
        root: root_node,
        top_files,
        file_types,
        duplicate_groups,
        total_files,
        total_dirs,
        total_size,
        total_allocated,
        volume_total_bytes,
        volume_free_bytes,
        elapsed_ms: started.elapsed().as_millis() as u64,
        errors,
        truncated: budget.truncated,
        engine: engine.to_string(),
        duplicate_mode: if !find_duplicates {
            "off".to_string()
        } else if duplicate_include_date {
            "name_size_date".to_string()
        } else {
            "name_size".to_string()
        },
        protected_roots: vec![root_path.clone()],
        file_index: Some(DiskFileIndex {
            root_path: root_path.clone(),
            files: indexed_files,
            volumes: if engine == "csv" {
                Vec::new()
            } else {
                vec![DiskVolumeStat {
                    path: root_path.clone(),
                    total_bytes: volume_total_bytes,
                    free_bytes: volume_free_bytes,
                }]
            },
            complete: index_complete,
        }),
    }
}

fn volume_identity(path: &str) -> String {
    let normalized = path.replace('/', "\\");
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        return normalized[..2].to_ascii_lowercase();
    }
    if let Some(rest) = normalized.strip_prefix("\\\\") {
        let mut parts = rest.splitn(3, '\\');
        let server = parts.next().unwrap_or_default();
        let share = parts.next().unwrap_or_default();
        if !server.is_empty() && !share.is_empty() {
            return format!("\\\\{}\\{}", server, share).to_ascii_lowercase();
        }
    }
    normalized.to_ascii_lowercase()
}

fn duplicate_groups_from_cached(
    files: &[CachedDiskFile],
    include_date: bool,
) -> Vec<DuplicateGroup> {
    let mut grouped: HashMap<(u64, String, Option<u64>), Vec<&CachedDiskFile>> = HashMap::new();
    for file in files {
        if file.size_bytes == 0 {
            continue;
        }
        let name = Path::new(&file.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&file.path)
            .to_lowercase();
        let modified = if include_date {
            Some(file.modified_epoch_ms.unwrap_or(0))
        } else {
            None
        };
        grouped
            .entry((file.size_bytes, name, modified))
            .or_default()
            .push(file);
    }

    let mut groups: Vec<DuplicateGroup> = grouped
        .into_iter()
        .filter(|(_, entries)| entries.len() > 1)
        .map(|((size_bytes, name, _), mut entries)| {
            entries.sort_by_key(|file| Reverse(file.allocated_bytes));
            let file_count = entries.len();
            let files = entries
                .into_iter()
                .take(25)
                .map(disk_file_from_cached)
                .collect();
            DuplicateGroup {
                name,
                size_bytes,
                file_count,
                files,
            }
        })
        .collect();
    groups.sort_by_key(|group| {
        Reverse(
            group
                .size_bytes
                .saturating_mul(group.file_count.saturating_sub(1) as u64),
        )
    });
    groups.truncate(500);
    groups
}

pub fn merge_disk_scans(results: Vec<DiskScanResult>, label: &str) -> Result<DiskScanResult, String> {
    if results.is_empty() {
        return Err("No scan targets were provided.".to_string());
    }
    if results.len() == 1 {
        return Ok(results.into_iter().next().unwrap());
    }

    let duplicate_mode = results[0].duplicate_mode.clone();
    let find_duplicates = duplicate_mode != "off";
    let include_date = duplicate_mode == "name_size_date";

    let mut children = Vec::with_capacity(results.len());
    let mut files: Vec<CachedDiskFile> = Vec::new();
    let mut top_files: Vec<DiskFile> = Vec::new();
    let mut file_types: HashMap<String, (u64, u64)> = HashMap::new();
    let mut protected_roots = Vec::new();
    let mut volumes: HashMap<String, (u64, u64)> = HashMap::new();
    let mut volume_stats: Vec<DiskVolumeStat> = Vec::new();
    let mut total_files = 0u64;
    let mut total_dirs = 0u64;
    let mut total_size = 0u64;
    let mut total_allocated = 0u64;
    let mut errors = 0u64;
    let mut truncated = false;
    let mut index_complete = true;
    let mut elapsed_ms = 0u64;

    for mut result in results {
        if let Some(index) = result.file_index.take() {
            index_complete &= index.complete;
            volume_stats.extend(index.volumes);
            files.extend(index.files);
        } else {
            index_complete = false;
        }
        top_files.extend(result.top_files);
        for stat in result.file_types {
            let entry = file_types.entry(stat.extension).or_insert((0, 0));
            entry.0 += stat.size_bytes;
            entry.1 += stat.file_count;
        }
        protected_roots.push(result.root.path.clone());
        volumes.insert(
            volume_identity(&result.root.path),
            (result.volume_total_bytes, result.volume_free_bytes),
        );
        total_files += result.total_files;
        total_dirs += result.total_dirs;
        total_size += result.total_size;
        total_allocated += result.total_allocated;
        errors += result.errors;
        truncated |= result.truncated;
        elapsed_ms += result.elapsed_ms;
        children.push(result.root);
    }

    let duplicate_groups = if find_duplicates {
        duplicate_groups_from_cached(&files, include_date)
    } else {
        Vec::new()
    };

    if files.len() > MAX_INDEXED_FILES {
        files.truncate(MAX_INDEXED_FILES);
        index_complete = false;
    }

    top_files.sort_by_key(|file| Reverse(file.allocated_bytes.max(file.size_bytes)));
    top_files.truncate(100);

    let mut file_types: Vec<FileTypeStat> = file_types
        .into_iter()
        .map(|(extension, (size_bytes, file_count))| FileTypeStat {
            extension,
            size_bytes,
            file_count,
        })
        .collect();
    file_types.sort_by_key(|stat| Reverse(stat.size_bytes));
    file_types.truncate(20);

    let (volume_total_bytes, volume_free_bytes) = volumes
        .values()
        .fold((0u64, 0u64), |(total, free), (volume_total, volume_free)| {
            (total + volume_total, free + volume_free)
        });

    let root_path = format!("::multi::{}", children.len());
    let root = DiskTreeNode {
        name: format!("{} ({} roots)", label, children.len()),
        path: root_path.clone(),
        is_dir: true,
        size_bytes: total_size,
        allocated_bytes: total_allocated,
        modified_epoch_ms: None,
        attributes: "D".to_string(),
        file_count: total_files,
        children,
    };

    let mut protected_roots_out = Vec::with_capacity(protected_roots.len() + 1);
    protected_roots_out.push(root_path.clone());
    protected_roots_out.extend(protected_roots);

    Ok(DiskScanResult {
        root_path: root_path.clone(),
        root,
        top_files,
        file_types,
        duplicate_groups,
        total_files,
        total_dirs,
        total_size,
        total_allocated,
        volume_total_bytes,
        volume_free_bytes,
        elapsed_ms,
        errors,
        truncated,
        engine: "multi".to_string(),
        duplicate_mode,
        protected_roots: protected_roots_out,
        file_index: Some(DiskFileIndex {
            root_path,
            files,
            volumes: volume_stats,
            complete: index_complete,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn attribute_letters_cover_common_windows_flags() {
        assert_eq!(attribute_letters(0), "");
        assert_eq!(attribute_letters(0x1 | 0x2 | 0x800), "RHC");
        assert!(attribute_letters(0x10).contains('D'));
        assert_eq!(attribute_letters(0x80), "");
    }

    #[test]
    fn groups_duplicates_by_date_only_when_requested() {
        let root = std::env::temp_dir().join("purgekit_disk_duplicate_date_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        let left = root.join("a\\same.txt");
        let right = root.join("b\\same.txt");
        fs::write(&left, vec![5u8; 64]).unwrap();
        fs::write(&right, vec![5u8; 64]).unwrap();
        let set_time = |path: &std::path::Path, seconds: u64| {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
                .unwrap();
        };
        set_time(&left, 1_000);
        set_time(&right, 2_000);

        let cancel = AtomicBool::new(false);
        let by_name_size =
            scan_disk_tree(&root.to_string_lossy(), 3, 20, true, false, &cancel, |_| {}).unwrap();
        assert_eq!(by_name_size.duplicate_groups.len(), 1);

        let by_date =
            scan_disk_tree(&root.to_string_lossy(), 3, 20, true, true, &cancel, |_| {}).unwrap();
        assert!(by_date.duplicate_groups.is_empty());
        assert_eq!(by_date.duplicate_mode, "name_size_date");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn tree_budget_is_spent_breadth_first() {
        fn node(name: &str, is_dir: bool, size: u64) -> FlatNode {
            FlatNode {
                name: name.to_string(),
                path: format!(r"C:\{}", name),
                parent: Some(0),
                is_dir,
                size_bytes: size,
                allocated_bytes: size,
                modified_epoch_ms: None,
                created_epoch_ms: None,
                accessed_epoch_ms: None,
                mft_record: None,
                attributes: if is_dir { "D".to_string() } else { "A".to_string() },
                file_count: if is_dir { 0 } else { 1 },
                extension: String::new(),
            }
        }

        let mut nodes = vec![
            node("root", true, 0),
            node("big", true, 1000),
            node("small", true, 10),
            node("big1", false, 100),
            node("big2", false, 100),
            node("big3", false, 100),
        ];
        nodes[0].parent = None;
        let mut children = vec![vec![1, 2], vec![3, 4, 5], vec![], vec![], vec![], vec![]];
        children[1].sort_unstable();
        let mut budget = TreeBudget {
            remaining: 3,
            truncated: false,
        };

        let root = build_tree_breadth_first(0, 8, 100, &nodes, &children, &mut budget);
        assert_eq!(root.children.len(), 2);
        assert!(root.children.iter().any(|child| child.name == "small"));
        assert!(budget.truncated);
    }

    #[test]
    fn name_filter_ignores_folder_names_unless_path_mode_is_selected() {
        let root = std::env::temp_dir().join("purgekit_disk_name_filter_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Alpha")).unwrap();
        fs::write(root.join("Alpha\\Report.TXT"), b"report").unwrap();
        fs::write(root.join("Alpha\\photo.PNG"), b"photo").unwrap();

        let cancel = AtomicBool::new(false);
        let result =
            scan_disk_tree(&root.to_string_lossy(), 3, 20, false, false, &cancel, |_| {}).unwrap();
        let index = result.file_index.as_ref().unwrap();

        let by_name = search_cached_files(index, "Alpha", None, None, Some("name"), None, None, 10, "name");
        assert_eq!(by_name.total_matches, 0);

        let by_path = search_cached_files(index, "Alpha", None, None, Some("path"), None, None, 10, "name");
        assert_eq!(by_path.total_matches, 2);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scans_tree_and_aggregates_sizes() {
        let root = std::env::temp_dir().join("purgekit_disk_scan_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("node_modules\\pkg")).unwrap();
        fs::write(root.join("src\\main.rs"), b"0123456789").unwrap();
        fs::write(root.join("node_modules\\pkg\\index.js"), b"01234567890123456789").unwrap();
        fs::write(root.join("README.md"), b"hello").unwrap();

        let cancel = AtomicBool::new(false);
        let result = scan_disk_tree(
            &root.to_string_lossy(),
            3,
            20,
            false, false, &cancel,
            |_progress| {},
        )
        .unwrap();

        let _ = fs::remove_dir_all(&root);

        assert_eq!(result.total_files, 3);
        assert_eq!(result.total_dirs, 3); // src, node_modules, pkg
        assert!(result.total_size >= 35);
        assert!(result.root.allocated_bytes > 0);
        assert_eq!(result.top_files.len(), 3);
        assert!(result.file_types.iter().any(|stat| stat.extension == "rs"));
        assert!(!result.truncated);
    }

    #[test]
    #[ignore = "manual smoke test scans a real folder"]
    fn disk_scan_smoke() {
        let root = std::env::var("PURGEKIT_DISK_SMOKE")
            .unwrap_or_else(|_| r"D:\Code\PurgeKit\src-tauri\src".to_string());
        let cancel = AtomicBool::new(false);
        let result = scan_disk_tree(&root, 4, 40, true, false, &cancel, |_| {}).unwrap();
        println!(
            "disk smoke: {} files, {} dirs, {} logical, {} allocated, {} ms, truncated={}",
            result.total_files,
            result.total_dirs,
            result.total_size,
            result.total_allocated,
            result.elapsed_ms,
            result.truncated
        );
        println!(
            "top file: {:?}",
            result.top_files.first().map(|file| (&file.name, file.allocated_bytes))
        );
        assert!(result.total_files > 0);
        assert!(!result.top_files.is_empty());
    }

    #[test]
    fn finds_name_and_size_duplicates() {
        let root = std::env::temp_dir().join("purgekit_disk_duplicate_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        fs::write(root.join("a\\copy.txt"), b"0123456789").unwrap();
        fs::write(root.join("b\\copy.txt"), b"0123456789").unwrap();

        let cancel = AtomicBool::new(false);
        let result = scan_disk_tree(&root.to_string_lossy(), 3, 20, true, false, &cancel, |_| {}).unwrap();
        assert_eq!(result.duplicate_groups.len(), 1);
        assert_eq!(result.duplicate_groups[0].file_count, 2);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prefix_matching_stops_at_directory_boundaries() {
        assert!(is_under_prefix(r"C:\Tools\App\bin\a.exe", r"C:\Tools\App"));
        assert!(is_under_prefix(r"C:\Tools\App\bin\a.exe", r"C:\Tools\App\"));
        assert!(is_under_prefix(r"C:\TOOLS\APP\bin\a.exe", r"c:\tools\app"));
        assert!(!is_under_prefix(r"C:\Tools\Application\a.exe", r"C:\Tools\App"));
        assert!(!is_under_prefix(r"C:\Tools\App", r"C:\Tools\App"));
        assert!(is_under_prefix(r"C:\Tools\App\a.exe", ""));
    }

    #[test]
    fn search_filters_and_sorts_every_match() {
        use std::path::Path;

        let root = std::env::temp_dir().join("purgekit_disk_search_filters_test");
        let _ = fs::remove_dir_all(&root);
        for folder in ["Alpha", "Alphabet", "Beta"] {
            fs::create_dir_all(root.join(folder)).unwrap();
        }
        fs::write(root.join("Alpha\\Report.TXT"), vec![0u8; 40]).unwrap();
        fs::write(root.join("Alphabet\\Report.TXT"), vec![0u8; 20]).unwrap();
        fs::write(root.join("Beta\\Report.TXT"), vec![0u8; 10]).unwrap();
        fs::write(root.join("Alpha\\photo.PNG"), vec![0u8; 5]).unwrap();

        fn set_modified(path: std::path::PathBuf, seconds: u64) {
            let file = fs::File::options().write(true).open(path).unwrap();
            file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
                .unwrap();
        }
        set_modified(root.join("Alpha\\Report.TXT"), 1_000);
        set_modified(root.join("Alphabet\\Report.TXT"), 2_000);
        set_modified(root.join("Beta\\Report.TXT"), 3_000);
        set_modified(root.join("Alpha\\photo.PNG"), 500);

        let cancel = AtomicBool::new(false);
        let result = scan_disk_tree(&root.to_string_lossy(), 3, 20, false, false, &cancel, |_| {}).unwrap();
        let index = result.file_index.as_ref().unwrap();

        let parent_name = |file: &DiskFile| {
            Path::new(&file.path)
                .parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        let reports = search_cached_files(index, "REPORT", Some(".txt"), None, None, None, None, 10, "name");
        assert_eq!(reports.total_matches, 3);
        assert!(reports.files.iter().all(|file| file.extension == "txt"));

        let scoped = search_cached_files(
            index,
            "report",
            Some("txt"),
            Some(&root.join("Alpha").to_string_lossy()),
            None,
            None,
            None,
            10,
            "size",
        );
        assert_eq!(scoped.total_matches, 1);
        assert_eq!(scoped.files[0].name, "Report.TXT");
        assert_eq!(parent_name(&scoped.files[0]), "Alpha");

        let all_in_alpha = search_cached_files(
            index,
            "",
            None,
            Some(&root.join("Alpha").to_string_lossy()),
            None,
            None,
            None,
            10,
            "size",
        );
        assert_eq!(all_in_alpha.total_matches, 2);

        let by_date = search_cached_files(index, "", Some("txt"), None, None, None, None, 10, "modified");
        assert_eq!(
            by_date.files.iter().map(parent_name).collect::<Vec<_>>(),
            vec!["Beta", "Alphabet", "Alpha"]
        );

        let by_size = search_cached_files(index, "", Some("txt"), None, None, None, None, 10, "size");
        assert_eq!(
            by_size.files.iter().map(parent_name).collect::<Vec<_>>(),
            vec!["Alpha", "Alphabet", "Beta"]
        );

        let by_name = search_cached_files(index, "", None, None, None, None, None, 10, "name");
        assert_eq!(by_name.files[0].name, "photo.PNG");

        let included = search_cached_files(index, "", None, None, None, Some("alpha\\"), None, 10, "name");
        assert_eq!(included.total_matches, 2);
        assert!(included.files.iter().all(|file| file.path.contains("\\Alpha\\")));

        let excluded = search_cached_files(index, "", None, None, None, None, Some("alphabet"), 10, "name");
        assert_eq!(excluded.total_matches, 3);

        let combined = search_cached_files(index, "report", None, None, None, Some("report"), Some("alphabet"), 10, "name");
        assert_eq!(combined.total_matches, 2);
        assert!(combined.files.iter().all(|file| file.name == "Report.TXT"));

        let limited = search_cached_files(index, "", None, None, None, None, None, 2, "size");
        assert_eq!(limited.total_matches, 4);
        assert_eq!(limited.files.len(), 2);

        let clamped = search_cached_files(index, "", None, None, None, None, None, 0, "size");
        assert_eq!(clamped.files.len(), 1);

        let missing = search_cached_files(index, "does-not-exist", None, None, None, None, None, 10, "size");
        assert_eq!(missing.total_matches, 0);
        assert!(missing.files.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn merges_multiple_scan_roots_and_finds_cross_root_duplicates() {
        let base = std::env::temp_dir().join("purgekit_multi_root_merge_test");
        let left = base.join("left");
        let right = base.join("right");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::write(left.join("same.txt"), vec![7u8; 128]).unwrap();
        fs::write(right.join("same.txt"), vec![7u8; 128]).unwrap();

        let cancel = AtomicBool::new(false);
        let left_result =
            scan_disk_tree(&left.to_string_lossy(), 3, 20, true, false, &cancel, |_| {}).unwrap();
        let right_result =
            scan_disk_tree(&right.to_string_lossy(), 3, 20, true, false, &cancel, |_| {}).unwrap();
        let merged = merge_disk_scans(vec![left_result, right_result], "Multiple locations").unwrap();

        assert_eq!(merged.engine, "multi");
        assert!(merged.root_path.starts_with("::multi::"));
        assert_eq!(merged.root.children.len(), 2);
        assert_eq!(merged.total_files, 2);
        assert_eq!(merged.total_size, 256);
        assert_eq!(merged.protected_roots.len(), 3);
        assert_eq!(merged.duplicate_groups.len(), 1);
        assert_eq!(merged.duplicate_groups[0].file_count, 2);
        assert_eq!(merged.file_index.as_ref().unwrap().files.len(), 2);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn exports_all_matching_files_as_csv() {
        let root = std::env::temp_dir().join("purgekit_disk_csv_export_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("alpha.txt"), b"alpha").unwrap();
        fs::write(root.join("beta,quoted.txt"), b"beta data").unwrap();

        let cancel = AtomicBool::new(false);
        let result =
            scan_disk_tree(&root.to_string_lossy(), 2, 20, false, false, &cancel, |_| {}).unwrap();
        let index = result.file_index.as_ref().unwrap();

        let exported = export_cached_files_csv(
            index,
            "",
            None,
            None,
            Some("name"),
            None,
            None,
            "name",
            10,
            "csv",
            false,
            false,
            false,
            false,
            false,
        );
        assert_eq!(exported.total, 2);
        assert_eq!(exported.exported, 2);
        assert!(!exported.truncated);
        assert!(exported
            .content
            .contains("Name,Path,Type,Extension,Modified,Attributes"));
        assert!(exported.content.contains("alpha.txt"));
        assert!(exported.content.contains("\"beta,quoted.txt\""));

        let limited = export_cached_files_csv(
            index,
            "",
            None,
            None,
            Some("name"),
            None,
            None,
            "name",
            1,
            "csv",
            false,
            false,
            false,
            false,
            false,
        );
        assert_eq!(limited.exported, 1);
        assert!(limited.truncated);

        let included = export_cached_files_csv(
            index,
            "",
            None,
            None,
            Some("name"),
            Some("beta"),
            None,
            "name",
            10,
            "csv",
            false,
            false,
            false,
            false,
            false,
        );
        assert_eq!(included.total, 1);
        assert!(included.content.contains("beta,quoted.txt"));
        assert!(!included.content.contains("alpha.txt"));

        let excluded = export_cached_files_csv(
            index,
            "",
            None,
            None,
            Some("name"),
            None,
            Some("alpha"),
            "name",
            10,
            "csv",
            false,
            false,
            false,
            false,
            false,
        );
        assert_eq!(excluded.total, 1);
        assert!(excluded.content.contains("beta,quoted.txt"));

        let with_folders = export_cached_files_csv(
            index,
            "",
            None,
            None,
            Some("name"),
            None,
            None,
            "name",
            10,
            "tsv",
            true,
            true,
            true,
            true,
            true,
        );
        assert_eq!(with_folders.folders, 1);
        assert!(with_folders.content.contains("\tFile\t"));
        assert!(with_folders.content.contains("\tFolder\t"));
        assert!(with_folders.content.contains("Created"));
        assert!(with_folders.content.contains("LastAccessed"));
        assert!(with_folders.content.contains("DirectFiles"));
        assert!(with_folders.content.contains("ParentPercent"));
        assert!(with_folders.content.contains("MftRecord"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn exports_mft_record_numbers_when_requested() {
        let index = DiskFileIndex {
            root_path: r"C:\".to_string(),
            files: vec![CachedDiskFile {
                path: r"C:\Temp\sample.bin".to_string(),
                size_bytes: 16,
                allocated_bytes: 16,
                modified_epoch_ms: Some(1_700_000_000_000),
                created_epoch_ms: None,
                accessed_epoch_ms: None,
                mft_record: Some(42),
                attributes: "A".to_string(),
            }],
            volumes: vec![DiskVolumeStat {
                path: r"C:\".to_string(),
                total_bytes: 1000,
                free_bytes: 250,
            }],
            complete: true,
        };

        let exported = export_cached_files_csv(
            &index, "", None, None, None, None, None, "name", 10, "csv", false, false,
            false, true, true,
        );
        assert!(exported.content.contains("Name,Path,MftRecord,Type"));
        assert!(exported.content.contains("C:\\Temp\\sample.bin,42,File"));
        assert!(exported.content.contains("Drive,TotalBytes,FreeBytes,UsedBytes"));
    }

    #[test]
    fn exports_file_type_totals() {
        let index = DiskFileIndex {
            root_path: r"C:\".to_string(),
            files: vec![
                CachedDiskFile {
                    path: r"C:\a.txt".to_string(),
                    size_bytes: 10,
                    allocated_bytes: 16,
                    modified_epoch_ms: None,
                    created_epoch_ms: None,
                    accessed_epoch_ms: None,
                    mft_record: None,
                    attributes: "A".to_string(),
                },
                CachedDiskFile {
                    path: r"C:\b.txt".to_string(),
                    size_bytes: 20,
                    allocated_bytes: 32,
                    modified_epoch_ms: None,
                    created_epoch_ms: None,
                    accessed_epoch_ms: None,
                    mft_record: None,
                    attributes: "A".to_string(),
                },
                CachedDiskFile {
                    path: r"C:\c.bin".to_string(),
                    size_bytes: 5,
                    allocated_bytes: 8,
                    modified_epoch_ms: None,
                    created_epoch_ms: None,
                    accessed_epoch_ms: None,
                    mft_record: None,
                    attributes: "A".to_string(),
                },
            ],
            volumes: vec![],
            complete: true,
        };

        let exported = export_cached_file_types_csv(&index, "csv");
        assert_eq!(exported.exported, 2);
        assert!(exported.content.contains("Extension,FileCount,LogicalBytes,AllocatedBytes"));
        assert!(exported.content.contains("txt,2,30,48"));
        assert!(exported.content.contains("bin,1,5,8"));
    }

    #[test]
    fn rejects_relative_and_drive_relative_roots() {
        let cancel = AtomicBool::new(false);
        for root in [r"C:", r"relative\path", "."] {
            let result = scan_disk_tree(root, 2, 10, false, false, &cancel, |_| {});
            assert!(result.is_err(), "{} should be rejected", root);
        }
    }

    #[test]
    fn reports_cancelled_scan() {
        let cancel = AtomicBool::new(true);
        let root = std::env::temp_dir().join("purgekit_disk_scan_cancel_test");
        let _ = fs::create_dir_all(&root);
        let result = scan_disk_tree(&root.to_string_lossy(), 2, 10, false, false, &cancel, |_| {});
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn counts_hardlinked_files_once() {
        let root = std::env::temp_dir().join("purgekit_disk_hardlink_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let original = root.join("original.bin");
        fs::write(&original, vec![7u8; 4096]).unwrap();
        if fs::hard_link(&original, root.join("alias.bin")).is_err() {
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let cancel = AtomicBool::new(false);
        let result = scan_disk_tree(&root.to_string_lossy(), 2, 20, true, false, &cancel, |_| {}).unwrap();
        assert_eq!(result.total_files, 2);
        assert_eq!(result.total_size, 4096);
        assert!(result.duplicate_groups.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn does_not_group_different_names_or_empty_files() {
        let root = std::env::temp_dir().join("purgekit_disk_duplicate_negative_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        fs::write(root.join("a\\one.txt"), vec![3u8; 10]).unwrap();
        fs::write(root.join("b\\two.txt"), vec![3u8; 10]).unwrap();
        fs::write(root.join("a\\empty-one.txt"), b"").unwrap();
        fs::write(root.join("b\\empty-two.txt"), b"").unwrap();

        let cancel = AtomicBool::new(false);
        let result = scan_disk_tree(&root.to_string_lossy(), 3, 20, true, false, &cancel, |_| {}).unwrap();
        assert!(result.duplicate_groups.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn builds_searchable_file_index() {
        let root = std::env::temp_dir().join("purgekit_disk_index_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("Big_Setup.EXE"), vec![0u8; 2048]).unwrap();
        fs::write(root.join("nested\\notes.txt"), b"hello").unwrap();
        fs::write(root.join("nested\\backup.bin"), vec![0u8; 1024]).unwrap();

        let cancel = AtomicBool::new(false);
        let result = scan_disk_tree(&root.to_string_lossy(), 3, 20, false, false, &cancel, |_| {}).unwrap();
        let index = result.file_index.as_ref().unwrap();
        assert_eq!(index.files.len(), 3);
        assert!(index.complete);
        assert!(index.files.iter().all(|file| file.modified_epoch_ms.is_some()));

        let search = search_cached_files(index, "setup", None, None, None, None, None, 10, "size");
        assert_eq!(search.total_matches, 1);
        assert_eq!(search.files[0].name, "Big_Setup.EXE");
        assert_eq!(search.files[0].extension, "exe");
        assert!(search.files[0].modified_epoch_ms.is_some());

        let by_extension = search_cached_files(index, "", Some(".TXT"), None, None, None, None, 10, "size");
        assert_eq!(by_extension.total_matches, 1);
        assert_eq!(by_extension.files[0].name, "notes.txt");

        let scoped = search_cached_files(
            index,
            "",
            None,
            Some(&root.join("nested").to_string_lossy()),
            None,
            None,
            None,
            10,
            "size",
        );
        assert_eq!(scoped.total_matches, 2);

        let sorted = search_cached_files(index, "", None, None, None, None, None, 2, "size");
        assert_eq!(sorted.total_matches, 3);
        assert_eq!(sorted.files.len(), 2);
        assert_eq!(sorted.files[0].name, "Big_Setup.EXE");

        let by_name = search_cached_files(index, "", None, None, None, None, None, 10, "name");
        assert_eq!(by_name.files[0].name, "backup.bin");
        assert_eq!(by_name.files[1].name, "Big_Setup.EXE");

        let _ = fs::remove_dir_all(&root);
    }
}
