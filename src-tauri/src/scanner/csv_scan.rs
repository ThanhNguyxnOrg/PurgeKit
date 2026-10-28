use std::collections::HashMap;
use std::time::Instant;

use chrono::{Local, NaiveDateTime, TimeZone};

use super::disk_scan::{finalize_scan, DiskScanResult, FlatNode};

const MAX_CSV_BYTES: usize = 256 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 1_500_000;
const MAX_IMPORT_NODES: usize = 3_000_000;

struct CsvColumns {
    path: usize,
    size: Option<usize>,
    allocated: Option<usize>,
    modified: Option<usize>,
    created: Option<usize>,
    accessed: Option<usize>,
    attributes: Option<usize>,
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('\u{feff}')
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn find_column(headers: &csv::StringRecord, aliases: &[&str]) -> Option<usize> {
    for alias in aliases {
        if let Some(index) = headers
            .iter()
            .position(|header| normalize_header(header) == *alias)
        {
            return Some(index);
        }
    }
    None
}

fn columns_from_headers(headers: &csv::StringRecord) -> Result<CsvColumns, String> {
    let path = find_column(
        headers,
        &[
            "path",
            "full path",
            "fullpath",
            "file path",
            "filepath",
            "file name",
            "filename",
        ],
    )
    .ok_or_else(|| {
        "CSV needs a Path or File Name column containing absolute paths. WizTree puts the full path in File Name.".to_string()
    })?;

    Ok(CsvColumns {
        path,
        size: find_column(
            headers,
            &[
                "size",
                "logical size",
                "logical bytes",
                "logicalbytes",
                "file size",
                "filesize",
                "bytes",
            ],
        ),
        allocated: find_column(
            headers,
            &[
                "allocated",
                "allocated size",
                "allocated bytes",
                "allocatedbytes",
                "size on disk",
                "physical size",
                "physicalsize",
            ],
        ),
        modified: find_column(
            headers,
            &[
                "modified",
                "modified date",
                "date modified",
                "last modified",
                "last write time",
                "modified time",
            ],
        ),
        created: find_column(
            headers,
            &["created", "created date", "date created", "creation time", "created time"],
        ),
        accessed: find_column(
            headers,
            &[
                "last accessed",
                "accessed",
                "last access",
                "access time",
                "last accessed time",
            ],
        ),
        attributes: find_column(headers, &["attributes", "attribute", "attr", "attrs"]),
    })
}

fn detect_delimiter(content: &str) -> u8 {
    let first_line = content.lines().next().unwrap_or_default();
    let commas = first_line.matches(',').count();
    let tabs = first_line.matches('\t').count();
    let semicolons = first_line.matches(';').count();
    if tabs > commas && tabs >= semicolons {
        b'\t'
    } else if semicolons > commas {
        b';'
    } else {
        b','
    }
}

fn parse_size(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let upper = trimmed.to_ascii_uppercase();
    let multiplier = if upper.ends_with("PIB") || upper.ends_with("PB") {
        1u64 << 50
    } else if upper.ends_with("TIB") || upper.ends_with("TB") {
        1u64 << 40
    } else if upper.ends_with("GIB") || upper.ends_with("GB") {
        1u64 << 30
    } else if upper.ends_with("MIB") || upper.ends_with("MB") {
        1u64 << 20
    } else if upper.ends_with("KIB") || upper.ends_with("KB") {
        1u64 << 10
    } else {
        1
    };
    let numeric: String = trimmed
        .chars()
        .filter(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    if numeric.is_empty() {
        return None;
    }
    let number: f64 = numeric.parse().ok()?;
    Some((number * multiplier as f64).round() as u64)
}

fn parse_modified_epoch_ms(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|character| character.is_ascii_digit()) {
        let number: u64 = trimmed.parse().ok()?;
        return if number > 10_000_000_000 {
            Some(number)
        } else if number > 1_000_000 {
            Some(number.saturating_mul(1000))
        } else {
            None
        };
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(parsed.timestamp_millis().max(0) as u64);
    }
    const FORMATS: &[&str] = &[
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%m/%d/%Y %I:%M:%S %p",
        "%m/%d/%Y %H:%M:%S",
        "%d/%m/%Y %H:%M:%S",
    ];
    for format in FORMATS {
        if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, format) {
            if let Some(local) = Local.from_local_datetime(&naive).single() {
                return Some(local.timestamp_millis().max(0) as u64);
            }
        }
    }
    None
}

fn attributes_indicate_directory(value: &str) -> bool {
    let upper = value.trim().to_ascii_uppercase();
    if upper.is_empty() {
        return false;
    }
    if upper.contains("DIRECTORY") || upper.contains("FOLDER") {
        return true;
    }
    upper
        .split([' ', ',', ';', '|', '-'])
        .any(|token| {
            !token.is_empty()
                && token.len() <= 6
                && token.chars().all(|character| "RHSDATPCONEI".contains(character))
                && token.contains('D')
        })
}

fn normalize_import_path(raw: &str) -> Result<(String, Vec<String>), String> {
    let mut value = raw.trim().replace('/', "\\");
    if value.starts_with("\\\\?\\UNC\\") {
        value = format!("\\\\{}", &value[8..]);
    } else if value.starts_with("\\\\?\\") {
        value = value[4..].to_string();
    }
    if value.is_empty() {
        return Err("empty path".to_string());
    }

    if value.as_bytes().len() >= 2
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value.as_bytes()[1] == b':'
    {
        if value.len() > 2 && value.as_bytes()[2] != b'\\' {
            return Err("drive-relative path".to_string());
        }
        if value.len() > 2 && value.as_bytes()[2] == b'\\' && !value.starts_with("\\\\") {
            let root = format!("{}:\\", &value[..1].to_ascii_uppercase());
            let rest = value[3..].trim_matches('\\');
            return Ok((root, split_components(rest)?));
        }
    }

    if let Some(rest) = value.strip_prefix("\\\\") {
        let mut parts = rest.splitn(3, '\\');
        let server = parts.next().unwrap_or_default();
        let share = parts.next().unwrap_or_default();
        if server.is_empty() || share.is_empty() {
            return Err("incomplete UNC path".to_string());
        }
        let root = format!("\\\\{}\\{}\\", server, share);
        let rest = parts.next().unwrap_or_default().trim_matches('\\');
        return Ok((root, split_components(rest)?));
    }

    Err("relative path".to_string())
}

fn split_components(rest: &str) -> Result<Vec<String>, String> {
    let mut components = Vec::new();
    for component in rest.split('\\') {
        let component = component.trim();
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            return Err("parent traversal is not allowed".to_string());
        }
        if component.contains(':') {
            return Err("alternate data stream path".to_string());
        }
        components.push(component.to_string());
    }
    Ok(components)
}

fn join_windows_path(parent: &str, name: &str) -> String {
    if parent.ends_with('\\') {
        format!("{}{}", parent, name)
    } else {
        format!("{}\\{}", parent, name)
    }
}

pub fn import_scan_csv(
    csv_text: &str,
    source_name: &str,
    find_duplicates: bool,
    duplicate_include_date: bool,
) -> Result<DiskScanResult, String> {
    let started = Instant::now();
    if csv_text.len() > MAX_CSV_BYTES {
        return Err("CSV is larger than 256 MB; export a narrower subtree.".to_string());
    }
    let content = csv_text.strip_prefix('\u{feff}').unwrap_or(csv_text);
    if content.trim().is_empty() {
        return Err("The selected CSV is empty.".to_string());
    }

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(detect_delimiter(content))
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());
    let headers = reader
        .headers()
        .map_err(|error| format!("Failed to read the CSV header: {}", error))?
        .clone();
    let columns = columns_from_headers(&headers)?;

    let mut nodes: Vec<FlatNode> = Vec::new();
    let mut paths: HashMap<String, usize> = HashMap::new();
    let mut root_path: Option<String> = None;
    let mut root_modified: Option<u64> = None;
    let mut root_attributes = String::new();
    let mut errors = 0u64;
    let mut rows = 0usize;

    for record in reader.records() {
        rows += 1;
        if rows > MAX_IMPORT_ROWS {
            errors += 1;
            break;
        }
        let record = match record {
            Ok(record) => record,
            Err(_) => {
                errors += 1;
                continue;
            }
        };

        let raw_path = record.get(columns.path).unwrap_or_default();
        if raw_path.is_empty() {
            errors += 1;
            continue;
        }
        let path_marks_directory = raw_path.ends_with('\\') || raw_path.ends_with('/');
        let attributes = columns
            .attributes
            .and_then(|index| record.get(index))
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        let is_directory = path_marks_directory || attributes_indicate_directory(&attributes);
        let modified = columns
            .modified
            .and_then(|index| record.get(index))
            .and_then(parse_modified_epoch_ms);
        let created = columns
            .created
            .and_then(|index| record.get(index))
            .and_then(parse_modified_epoch_ms);
        let accessed = columns
            .accessed
            .and_then(|index| record.get(index))
            .and_then(parse_modified_epoch_ms);

        let (parsed_root, components) = match normalize_import_path(raw_path) {
            Ok(parsed) => parsed,
            Err(_) => {
                errors += 1;
                continue;
            }
        };

        let root = match root_path.as_ref() {
            Some(root) => {
                if !root.eq_ignore_ascii_case(&parsed_root) {
                    errors += 1;
                    continue;
                }
                root.clone()
            }
            None => {
                nodes.push(FlatNode {
                    name: parsed_root.clone(),
                    path: parsed_root.clone(),
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
                });
                paths.insert(parsed_root.to_ascii_lowercase(), 0);
                root_path = Some(parsed_root.clone());
                parsed_root
            }
        };

        if components.is_empty() {
            if modified.is_some() {
                root_modified = modified;
            }
            if !attributes.is_empty() {
                root_attributes = attributes;
            }
            continue;
        }

        if nodes.len() + components.len() > MAX_IMPORT_NODES {
            errors += 1;
            break;
        }

        let file_size = columns
            .size
            .and_then(|index| record.get(index))
            .and_then(parse_size)
            .unwrap_or(0);
        let allocated_size = columns
            .allocated
            .and_then(|index| record.get(index))
            .and_then(parse_size)
            .unwrap_or(file_size);

        let mut parent = 0usize;
        let mut current_path = root.clone();
        let mut row_invalid = false;

        for (position, component) in components.iter().enumerate() {
            let is_last = position + 1 == components.len();
            current_path = join_windows_path(&current_path, component);
            let key = current_path.to_ascii_lowercase();
            let existing = paths.get(&key).copied();

            if is_last && !is_directory {
                if let Some(existing) = existing {
                    let node = &mut nodes[existing];
                    if node.is_dir {
                        row_invalid = true;
                        break;
                    }
                    node.size_bytes = file_size;
                    node.allocated_bytes = allocated_size;
                    if modified.is_some() {
                        node.modified_epoch_ms = modified;
                    }
                    if created.is_some() {
                        node.created_epoch_ms = created;
                    }
                    if accessed.is_some() {
                        node.accessed_epoch_ms = accessed;
                    }
                    if !attributes.is_empty() {
                        node.attributes = attributes.clone();
                    }
                } else {
                    let index = nodes.len();
                    nodes.push(FlatNode {
                        name: component.clone(),
                        path: current_path.clone(),
                        parent: Some(parent),
                        is_dir: false,
                        size_bytes: file_size,
                        allocated_bytes: allocated_size,
                        modified_epoch_ms: modified,
                        created_epoch_ms: created,
                        accessed_epoch_ms: accessed,
                        attributes: attributes.clone(),
                        file_count: 1,
                        extension: super::disk_scan::file_extension(&current_path)
                            .unwrap_or_else(|| "(none)".to_string()),
                    });
                    paths.insert(key, index);
                }
                break;
            }

            if let Some(existing) = existing {
                let node = &mut nodes[existing];
                if !node.is_dir {
                    row_invalid = true;
                    break;
                }
                if is_last {
                    if modified.is_some() {
                        node.modified_epoch_ms = modified;
                    }
                    if created.is_some() {
                        node.created_epoch_ms = created;
                    }
                    if accessed.is_some() {
                        node.accessed_epoch_ms = accessed;
                    }
                    if !attributes.is_empty() {
                        node.attributes = if attributes_indicate_directory(&attributes) {
                            attributes.clone()
                        } else {
                            format!("{}D", attributes)
                        };
                    }
                }
                parent = existing;
            } else {
                let index = nodes.len();
                let directory_attributes = if is_last && !attributes.is_empty() {
                    attributes.clone()
                } else {
                    "D".to_string()
                };
                nodes.push(FlatNode {
                    name: component.clone(),
                    path: current_path.clone(),
                    parent: Some(parent),
                    is_dir: true,
                    size_bytes: 0,
                    allocated_bytes: 0,
                    modified_epoch_ms: if is_last { modified } else { None },
                    created_epoch_ms: if is_last { created } else { None },
                    accessed_epoch_ms: if is_last { accessed } else { None },
                    attributes: directory_attributes,
                    file_count: 0,
                    extension: String::new(),
                });
                paths.insert(key, index);
                parent = index;
            }
        }

        if row_invalid {
            errors += 1;
        }
    }

    let root_path = root_path.ok_or_else(|| {
        format!(
            "No absolute file paths were found in {}.",
            source_name.trim()
        )
    })?;
    if let Some(root) = nodes.first_mut() {
        root.modified_epoch_ms = root_modified;
        if !root_attributes.is_empty() {
            root.attributes = root_attributes;
        }
    }

    Ok(finalize_scan(
        nodes,
        root_path,
        8,
        100,
        find_duplicates,
        duplicate_include_date,
        false,
        errors,
        started,
        "csv",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes_dates_and_attributes() {
        assert_eq!(parse_size("1,024"), Some(1_024));
        assert_eq!(parse_size("1.5 KB"), Some(1_536));
        assert_eq!(parse_size("2 GiB"), Some(2 * 1024 * 1024 * 1024));
        assert_eq!(parse_size(""), None);
        assert!(parse_modified_epoch_ms("2026-10-26 10:00:00").is_some());
        assert!(parse_modified_epoch_ms("2026-10-26T10:00:00Z").is_some());
        assert!(attributes_indicate_directory("D"));
        assert!(attributes_indicate_directory("R-D"));
        assert!(!attributes_indicate_directory("A"));
    }

    #[test]
    fn imports_wiztree_style_csv() {
        let csv = "\u{feff}File Name,Size,Allocated,Modified,Attributes,Files,Folders\n\
\"C:\\\",\"9999999\",\"9999999\",\"2026-10-26 09:00:00\",\"D\",3,4\n\
\"C:\\Temp\\\",\"9999999\",\"9999999\",\"2026-10-26 09:00:00\",\"D\",3,2\n\
\"C:\\Temp\\Alpha\\a.txt\",\"1,024\",\"4,096\",\"2026-10-26 10:00:00\",\"A\",1,0\n\
\"C:\\Temp\\Alpha\\photo,one.png\",\"2,048\",\"8,192\",\"2026-10-26 11:00:00\",\"A\",1,0\n\
\"C:\\Temp\\Alpha\\sub\\b.bin\",\"3,072\",\"4,096\",\"2026-10-26 12:00:00\",\"A\",1,0\n\
\"C:\\Temp\\Beta\\\",\"0\",\"0\",\"2026-10-26 09:00:00\",\"D\",0,0\n";

        let result = import_scan_csv(csv, "sample.csv", false, false).unwrap();
        assert_eq!(result.engine, "csv");
        assert_eq!(result.root_path, "C:\\");
        assert_eq!(result.total_files, 3);
        assert_eq!(result.total_dirs, 4);
        assert_eq!(result.total_size, 6_144);
        assert_eq!(result.total_allocated, 16_384);

        let index = result.file_index.as_ref().unwrap();
        assert_eq!(index.files.len(), 3);
        assert!(index
            .files
            .iter()
            .any(|file| file.path == "C:\\Temp\\Alpha\\photo,one.png"));
        assert!(index
            .files
            .iter()
            .all(|file| file.modified_epoch_ms.is_some()));
        assert_eq!(
            index
                .files
                .iter()
                .find(|file| file.path == "C:\\Temp\\Alpha\\a.txt")
                .unwrap()
                .allocated_bytes,
            4_096
        );
    }

    #[test]
    fn accepts_generic_headers_and_tab_delimiters() {
        let csv = "Full Path\tLogical Bytes\tAllocated Bytes\tModified\tAttributes\n\
C:\\Data\\one.txt\t10\t20\t2026-10-26 10:00:00\tA\n\
C:\\Data\\two.txt\t30\t40\t2026-10-26 10:00:00\tA\n";
        let result = import_scan_csv(csv, "generic.tsv", false, false).unwrap();
        assert_eq!(result.total_files, 2);
        assert_eq!(result.total_size, 40);
        assert_eq!(result.total_allocated, 60);
    }

    #[test]
    fn skips_rows_from_other_roots_and_relative_paths() {
        let csv = "Path,Size\n\
C:\\Data\\one.txt,10\n\
D:\\Other\\two.txt,20\n\
relative.txt,30\n\
C:\\Data\\one.txt,40\n";
        let result = import_scan_csv(csv, "mixed.csv", false, false).unwrap();
        assert_eq!(result.total_files, 1);
        assert_eq!(result.total_size, 40);
        assert_eq!(result.errors, 2);
    }

    #[test]
    fn rejects_csv_without_paths() {
        let error = import_scan_csv("Name,Size\nalpha.txt,10\n", "bad.csv", false, false)
            .unwrap_err();
        assert!(error.contains("absolute"));
    }
}
