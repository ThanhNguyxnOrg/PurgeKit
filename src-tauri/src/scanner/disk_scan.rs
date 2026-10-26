use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
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
    pub elapsed_ms: u64,
    pub errors: u64,
    pub truncated: bool,
    pub engine: String,
}

pub(crate) struct FlatNode {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) parent: Option<usize>,
    pub(crate) is_dir: bool,
    pub(crate) size_bytes: u64,
    pub(crate) allocated_bytes: u64,
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
    let root_path = crate::winutil::canonicalize_path_safety(root);
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

    fs::symlink_metadata(&root_path)
        .map_err(|e| format!("Failed to read root metadata: {}", e))?;

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
        errors,
        started,
        "traversal",
    ))
}

pub(crate) fn finalize_scan(
    mut nodes: Vec<FlatNode>,
    root_path: String,
    max_depth: u32,
    max_children: usize,
    find_duplicates: bool,
    errors: u64,
    started: Instant,
    engine: &str,
) -> DiskScanResult {
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

    for (index, node) in nodes.iter().enumerate().skip(1) {
        if node.is_dir {
            total_dirs += 1;
            continue;
        }

        total_files += 1;
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

    DiskScanResult {
        root_path,
        root: root_node,
        top_files,
        file_types,
        duplicate_groups,
        total_files,
        total_dirs,
        total_size,
        total_allocated,
        elapsed_ms: started.elapsed().as_millis() as u64,
        errors,
        truncated: budget.truncated,
        engine: engine.to_string(),
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
    fn reports_cancelled_scan() {
        let cancel = AtomicBool::new(true);
        let root = std::env::temp_dir().join("purgekit_disk_scan_cancel_test");
        let _ = fs::create_dir_all(&root);
        let result = scan_disk_tree(&root.to_string_lossy(), 2, 10, false, &cancel, |_| {});
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
