use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
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
}

#[derive(Debug, Clone, Default)]
pub struct DiskFileIndex {
    pub root_path: String,
    pub files: Vec<CachedDiskFile>,
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

fn is_under_prefix(path: &str, prefix: &str) -> bool {    let prefix = prefix.trim_end_matches(['\\', '/']);
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

fn file_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_lowercase())
}

pub fn search_cached_files(
    index: &DiskFileIndex,
    query: &str,
    extension: Option<&str>,
    prefix: Option<&str>,
    limit: usize,
    sort: &str,
) -> DiskFileSearchResult {
    let query = query.trim();
    let extension = extension
        .map(|value| value.trim().trim_start_matches('.').to_lowercase())
        .filter(|value| !value.is_empty());
    let prefix = prefix.map(str::trim).filter(|value| !value.is_empty());
    let limit = limit.clamp(1, 2_000);

    let mut matches: Vec<&CachedDiskFile> = Vec::new();
    for file in &index.files {
        if let Some(prefix) = prefix {
            if !is_under_prefix(&file.path, prefix) {
                continue;
            }
        }
        let file_extension = file_extension(&file.path);
        if let Some(wanted) = extension.as_deref() {
            if !(*file_extension.as_deref().unwrap_or("(none)")).eq_ignore_ascii_case(wanted) {
                continue;
            }
        }
        if !query.is_empty() && !contains_ascii_case_insensitive(&file.path, query) {
            continue;
        }
        matches.push(file);
    }

    let total_matches = matches.len();
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
    matches.truncate(limit);

    let files = matches
        .into_iter()
        .map(|file| DiskFile {
            name: Path::new(&file.path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| file.path.clone()),
            path: file.path.clone(),
            extension: file_extension(&file.path).unwrap_or_else(|| "(none)".to_string()),
            size_bytes: file.size_bytes,
            allocated_bytes: file.allocated_bytes,
            modified_epoch_ms: file.modified_epoch_ms,
        })
        .collect();

    DiskFileSearchResult {
        files,
        total_matches,
        indexed_files: index.files.len(),
        index_complete: index.complete,
    }
}


pub(crate) struct FlatNode {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) parent: Option<usize>,
    pub(crate) is_dir: bool,
    pub(crate) size_bytes: u64,
    pub(crate) allocated_bytes: u64,
    pub(crate) modified_epoch_ms: Option<u64>,
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

fn build_tree_node(
    index: usize,
    depth: u32,
    max_depth: u32,
    max_children: usize,
    nodes: &[FlatNode],
    children: &[Vec<usize>],
    budget: &mut TreeBudget,
) -> DiskTreeNode {
    let node = &nodes[index];
    let mut child_nodes = Vec::new();

    if node.is_dir && depth < max_depth && budget.remaining > 0 {
        let mut child_ids = children[index].clone();
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
            child_nodes.push(build_tree_node(
                child,
                depth + 1,
                max_depth,
                max_children,
                nodes,
                children,
                budget,
            ));
        }

        if total_children > max_children {
            budget.truncated = true;
        }
    }

    DiskTreeNode {
        name: node.name.clone(),
        path: node.path.clone(),
        is_dir: node.is_dir,
        size_bytes: node.size_bytes,
        allocated_bytes: node.allocated_bytes,
        modified_epoch_ms: node.modified_epoch_ms,
        file_count: node.file_count,
        children: child_nodes,
    }
}

pub fn scan_disk_tree<F>(
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
    let root_node = build_tree_node(
        0,
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
        let mut grouped: HashMap<(u64, String), Vec<usize>> = HashMap::new();
        for (index, node) in nodes.iter().enumerate().skip(1) {
            if node.is_dir || node.size_bytes == 0 {
                continue;
            }
            grouped
                .entry((node.size_bytes, node.name.to_lowercase()))
                .or_default()
                .push(index);
        }

        let mut groups: Vec<DuplicateGroup> = grouped
            .into_iter()
            .filter(|(_, indices)| indices.len() > 1)
            .map(|((size_bytes, name), mut indices)| {
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
        file_index: Some(DiskFileIndex {
            root_path,
            files: indexed_files,
            complete: index_complete,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::AtomicBool;

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
            false,
            &cancel,
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
        let result = scan_disk_tree(&root, 4, 40, true, &cancel, |_| {}).unwrap();
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
        let result = scan_disk_tree(&root.to_string_lossy(), 3, 20, true, &cancel, |_| {}).unwrap();
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
        let result = scan_disk_tree(&root.to_string_lossy(), 3, 20, false, &cancel, |_| {}).unwrap();
        let index = result.file_index.as_ref().unwrap();

        let parent_name = |file: &DiskFile| {
            Path::new(&file.path)
                .parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        let reports = search_cached_files(index, "REPORT", Some(".txt"), None, 10, "name");
        assert_eq!(reports.total_matches, 3);
        assert!(reports.files.iter().all(|file| file.extension == "txt"));

        let scoped = search_cached_files(
            index,
            "report",
            Some("txt"),
            Some(&root.join("Alpha").to_string_lossy()),
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
            10,
            "size",
        );
        assert_eq!(all_in_alpha.total_matches, 2);

        let by_date = search_cached_files(index, "", Some("txt"), None, 10, "modified");
        assert_eq!(
            by_date.files.iter().map(parent_name).collect::<Vec<_>>(),
            vec!["Beta", "Alphabet", "Alpha"]
        );

        let by_size = search_cached_files(index, "", Some("txt"), None, 10, "size");
        assert_eq!(
            by_size.files.iter().map(parent_name).collect::<Vec<_>>(),
            vec!["Alpha", "Alphabet", "Beta"]
        );

        let by_name = search_cached_files(index, "", None, None, 10, "name");
        assert_eq!(by_name.files[0].name, "photo.PNG");

        let limited = search_cached_files(index, "", None, None, 2, "size");
        assert_eq!(limited.total_matches, 4);
        assert_eq!(limited.files.len(), 2);

        let clamped = search_cached_files(index, "", None, None, 0, "size");
        assert_eq!(clamped.files.len(), 1);

        let missing = search_cached_files(index, "does-not-exist", None, None, 10, "size");
        assert_eq!(missing.total_matches, 0);
        assert!(missing.files.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_relative_and_drive_relative_roots() {
        let cancel = AtomicBool::new(false);
        for root in [r"C:", r"relative\path", "."] {
            let result = scan_disk_tree(root, 2, 10, false, &cancel, |_| {});
            assert!(result.is_err(), "{} should be rejected", root);
        }
    }

    #[test]
    fn reports_cancelled_scan() {
        let cancel = AtomicBool::new(true);
        let root = std::env::temp_dir().join("purgekit_disk_scan_cancel_test");
        let _ = fs::create_dir_all(&root);
        let result = scan_disk_tree(&root.to_string_lossy(), 2, 10, false, &cancel, |_| {});
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
        let result = scan_disk_tree(&root.to_string_lossy(), 2, 20, true, &cancel, |_| {}).unwrap();
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
        let result = scan_disk_tree(&root.to_string_lossy(), 3, 20, true, &cancel, |_| {}).unwrap();
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
        let result = scan_disk_tree(&root.to_string_lossy(), 3, 20, false, &cancel, |_| {}).unwrap();
        let index = result.file_index.as_ref().unwrap();
        assert_eq!(index.files.len(), 3);
        assert!(index.complete);
        assert!(index.files.iter().all(|file| file.modified_epoch_ms.is_some()));

        let search = search_cached_files(index, "setup", None, None, 10, "size");
        assert_eq!(search.total_matches, 1);
        assert_eq!(search.files[0].name, "Big_Setup.EXE");
        assert_eq!(search.files[0].extension, "exe");
        assert!(search.files[0].modified_epoch_ms.is_some());

        let by_extension = search_cached_files(index, "", Some(".TXT"), None, 10, "size");
        assert_eq!(by_extension.total_matches, 1);
        assert_eq!(by_extension.files[0].name, "notes.txt");

        let scoped = search_cached_files(
            index,
            "",
            None,
            Some(&root.join("nested").to_string_lossy()),
            10,
            "size",
        );
        assert_eq!(scoped.total_matches, 2);

        let sorted = search_cached_files(index, "", None, None, 2, "size");
        assert_eq!(sorted.total_matches, 3);
        assert_eq!(sorted.files.len(), 2);
        assert_eq!(sorted.files[0].name, "Big_Setup.EXE");

        let by_name = search_cached_files(index, "", None, None, 10, "name");
        assert_eq!(by_name.files[0].name, "backup.bin");
        assert_eq!(by_name.files[1].name, "Big_Setup.EXE");

        let _ = fs::remove_dir_all(&root);
    }
}
