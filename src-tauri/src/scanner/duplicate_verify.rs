//! Content verification for duplicate candidates.
//!
//! The disk scanners group candidates by metadata only (name + size, optionally
//! modified date). Before any copy is removed, the candidates are compared by
//! content in stages so most files are ruled out without being read in full:
//! current size, then a SHA-256 of the first block, then a SHA-256 of the whole
//! file.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// One call covers the listed copies of a single candidate group.
pub const MAX_VERIFY_PATHS: usize = 256;

const PREFIX_BYTES: u64 = 64 * 1024;
const READ_BUFFER_BYTES: usize = 1024 * 1024;

pub const CANCELLED: &str = "Duplicate verification cancelled";

pub struct DuplicateVerifyState(pub Arc<AtomicBool>);

impl Default for DuplicateVerifyState {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct SkippedDuplicate {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Clone, Default, PartialEq, Eq)]
pub struct DuplicateVerification {
    /// Sets of two or more paths whose contents are byte-identical.
    pub identical: Vec<Vec<String>>,
    /// Readable files whose contents match no other listed file.
    pub unique: Vec<String>,
    /// Files that were not read, with the reason.
    pub skipped: Vec<SkippedDuplicate>,
}

type Digest256 = [u8; 32];
/// Two paths with the same key are one file on disk.
#[derive(Hash, PartialEq, Eq)]
enum FileKey {
    /// Volume and file number.
    Identity(u64, u128),
    /// When an identity is unavailable, compare canonical paths without case.
    Path(String),
}

struct Candidate {
    path: String,
    len: u64,
    modified: Option<std::time::SystemTime>,
}

pub fn verify_duplicate_contents(
    paths: &[String],
    cancel: &AtomicBool,
) -> Result<DuplicateVerification, String> {
    if paths.len() > MAX_VERIFY_PATHS {
        return Err(format!(
            "Too many files to verify at once ({} > {})",
            paths.len(),
            MAX_VERIFY_PATHS
        ));
    }

    let mut verification = DuplicateVerification::default();
    let mut listed = HashSet::new();
    let mut files: HashMap<FileKey, String> = HashMap::new();
    let mut by_len: BTreeMap<u64, Vec<Candidate>> = BTreeMap::new();

    for path in paths {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        if !listed.insert(path.as_str()) {
            continue;
        }
        // Removing a second path to the same file (hard link, or the same file
        // spelled differently) frees nothing and must not count as a kept copy.
        let inspected = inspect(path).and_then(|(candidate, key)| match files.get(&key) {
            Some(first) => Err(format!("Same file on disk as {first}")),
            None => {
                files.insert(key, candidate.path.clone());
                Ok(candidate)
            }
        });
        match inspected {
            Ok(candidate) => by_len.entry(candidate.len).or_default().push(candidate),
            Err(reason) => verification.skipped.push(SkippedDuplicate {
                path: path.clone(),
                reason,
            }),
        }
    }

    let mut buffer = vec![0u8; READ_BUFFER_BYTES];
    for (len, candidates) in by_len {
        if candidates.len() < 2 {
            verification
                .unique
                .extend(candidates.into_iter().map(|c| c.path));
            continue;
        }

        let by_prefix = group_by_digest(
            candidates,
            len.min(PREFIX_BYTES),
            cancel,
            &mut buffer,
            &mut verification,
        )?;
        for (_, candidates) in by_prefix {
            if candidates.len() < 2 {
                verification
                    .unique
                    .extend(candidates.into_iter().map(|c| c.path));
                continue;
            }
            // The prefix digest already covers files no longer than one block.
            let classes = if len <= PREFIX_BYTES {
                vec![candidates]
            } else {
                group_by_digest(candidates, len, cancel, &mut buffer, &mut verification)?
                    .into_values()
                    .collect()
            };
            for class in classes {
                let mut unchanged = Vec::new();
                for candidate in class {
                    if is_unchanged(&candidate) {
                        unchanged.push(candidate.path);
                    } else {
                        verification.skipped.push(SkippedDuplicate {
                            path: candidate.path,
                            reason: "File changed during verification".into(),
                        });
                    }
                }
                if unchanged.len() < 2 {
                    verification.unique.extend(unchanged);
                } else {
                    verification.identical.push(unchanged);
                }
            }
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.into());
    }
    Ok(verification)
}

fn group_by_digest(
    candidates: Vec<Candidate>,
    limit: u64,
    cancel: &AtomicBool,
    buffer: &mut [u8],
    verification: &mut DuplicateVerification,
) -> Result<BTreeMap<Digest256, Vec<Candidate>>, String> {
    let mut groups: BTreeMap<Digest256, Vec<Candidate>> = BTreeMap::new();
    for candidate in candidates {
        match digest_file(Path::new(&candidate.path), limit, cancel, buffer) {
            Ok(Some(digest)) => groups.entry(digest).or_default().push(candidate),
            Ok(None) => return Err(CANCELLED.into()),
            Err(err) => verification.skipped.push(SkippedDuplicate {
                path: candidate.path,
                reason: format!("Could not read file: {err}"),
            }),
        }
    }
    Ok(groups)
}

/// Hashes exactly `limit` bytes; a file that ends early is reported as an error
/// so it can never match a file of the expected size. `Ok(None)` means cancelled.
fn digest_file(
    path: &Path,
    limit: u64,
    cancel: &AtomicBool,
    buffer: &mut [u8],
) -> io::Result<Option<Digest256>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut remaining = limit;
    while remaining > 0 {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let want = remaining.min(buffer.len() as u64) as usize;
        let read = file.read(&mut buffer[..want])?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "file is shorter than expected",
            ));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    Ok(Some(hasher.finalize().into()))
}

fn inspect(path: &str) -> Result<(Candidate, FileKey), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|err| format!("Could not read file: {err}"))?;
    if !metadata.file_type().is_file() || is_reparse_point(&metadata) {
        return Err("Not a regular file (link, junction or folder)".into());
    }
    if is_cloud_placeholder(&metadata) {
        return Err("Cloud-only file; reading it would download it".into());
    }
    if metadata.len() == 0 {
        return Err("Empty file".into());
    }
    let file_path = Path::new(path);
    let key = file_identity(file_path, &metadata)
        .and_then(|identity| file_key(file_path, identity))
        .map_err(|err| format!("Could not read file: {err}"))?;
    Ok((
        Candidate {
            path: path.to_string(),
            len: metadata.len(),
            modified: metadata.modified().ok(),
        },
        key,
    ))
}

fn file_key(path: &Path, identity: Option<(u64, u128)>) -> io::Result<FileKey> {
    match identity {
        Some((volume, file)) => Ok(FileKey::Identity(volume, file)),
        None => {
            fs::canonicalize(path).map(|path| FileKey::Path(path.to_string_lossy().to_lowercase()))
        }
    }
}

#[cfg(windows)]
fn file_identity(path: &Path, _metadata: &Metadata) -> io::Result<Option<(u64, u128)>> {
    use std::mem::size_of_val;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
        BY_HANDLE_FILE_INFORMATION, FILE_ID_INFO,
    };

    let file = File::open(path)?;
    let handle = file.as_raw_handle() as _;
    let mut info = FILE_ID_INFO::default();
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            size_of_val(&info) as u32,
        )
    } != 0
    {
        if let Some(id) = file_number(info.FileId.Identifier) {
            return Ok(Some((info.VolumeSerialNumber, id)));
        }
    }

    let mut legacy = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(handle, &mut legacy) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if legacy.nNumberOfLinks != 1 {
        return Err(io::Error::other("Could not identify a single-link file"));
    }
    Ok(None)
}

#[cfg(unix)]
fn file_identity(_path: &Path, metadata: &Metadata) -> io::Result<Option<(u64, u128)>> {
    use std::os::unix::fs::MetadataExt;
    Ok(Some((metadata.dev(), u128::from(metadata.ino()))))
}

/// A zero index means the volume reports no file numbers; using it as an
/// identity would make every file on that volume look like the same file.
#[cfg_attr(not(windows), allow(dead_code))]
fn file_number(id: [u8; 16]) -> Option<u128> {
    match u128::from_le_bytes(id) {
        0 => None,
        number => Some(number),
    }
}

fn is_unchanged(candidate: &Candidate) -> bool {
    fs::symlink_metadata(&candidate.path)
        .map(|metadata| {
            metadata.len() == candidate.len && metadata.modified().ok() == candidate.modified
        })
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &Metadata) -> bool {
    false
}

#[cfg(windows)]
fn is_cloud_placeholder(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    // FILE_ATTRIBUTE_OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS
    metadata.file_attributes() & (0x1000 | 0x4_0000 | 0x40_0000) != 0
}

#[cfg(not(windows))]
fn is_cloud_placeholder(_metadata: &Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "purgekit_duplicate_verify_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> String {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path.to_string_lossy().to_string()
    }

    fn verify(paths: &[String]) -> DuplicateVerification {
        verify_duplicate_contents(paths, &AtomicBool::new(false)).unwrap()
    }

    fn sorted(mut paths: Vec<String>) -> Vec<String> {
        paths.sort();
        paths
    }

    #[test]
    fn same_size_different_content_is_not_identical() {
        let dir = test_dir("differs");
        let a = write(&dir, "a.bin", &[1u8; 4096]);
        let b = write(&dir, "b.bin", &[2u8; 4096]);

        let result = verify(&[a.clone(), b.clone()]);

        assert!(result.identical.is_empty());
        assert_eq!(sorted(result.unique), sorted(vec![a, b]));
        assert!(result.skipped.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn groups_only_byte_identical_files() {
        let dir = test_dir("classes");
        let a = write(&dir, "a.bin", &[7u8; 1000]);
        let b = write(&dir, "b.bin", &[7u8; 1000]);
        let c = write(&dir, "c.bin", &[8u8; 1000]);
        let d = write(&dir, "d.bin", &[7u8; 999]);

        let result = verify(&[a.clone(), b.clone(), c.clone(), d.clone()]);

        assert_eq!(result.identical.len(), 1);
        assert_eq!(sorted(result.identical[0].clone()), sorted(vec![a, b]));
        assert_eq!(sorted(result.unique), sorted(vec![c, d]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_difference_after_the_first_block() {
        let dir = test_dir("tail");
        let len = PREFIX_BYTES as usize + READ_BUFFER_BYTES + 123;
        let base = vec![3u8; len];
        let mut tail = base.clone();
        tail[len - 1] = 4;
        let a = write(&dir, "a.bin", &base);
        let b = write(&dir, "b.bin", &base);
        let c = write(&dir, "c.bin", &tail);

        let result = verify(&[a.clone(), b.clone(), c.clone()]);

        assert_eq!(result.identical.len(), 1);
        assert_eq!(sorted(result.identical[0].clone()), sorted(vec![a, b]));
        assert_eq!(result.unique, vec![c]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_missing_empty_and_non_file_paths() {
        let dir = test_dir("skipped");
        let a = write(&dir, "a.bin", &[1u8; 10]);
        let b = write(&dir, "b.bin", &[1u8; 10]);
        let empty = write(&dir, "empty.bin", &[]);
        let missing = dir.join("missing.bin").to_string_lossy().to_string();
        let folder = dir.to_string_lossy().to_string();

        let result = verify(&[
            a.clone(),
            b.clone(),
            empty.clone(),
            missing.clone(),
            folder.clone(),
        ]);

        assert_eq!(result.identical.len(), 1);
        assert_eq!(sorted(result.identical[0].clone()), sorted(vec![a, b]));
        let skipped: Vec<String> = result.skipped.into_iter().map(|s| s.path).collect();
        assert_eq!(sorted(skipped), sorted(vec![empty, missing, folder]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_symlinks() {
        let dir = test_dir("symlink");
        let a = write(&dir, "a.bin", &[1u8; 10]);
        let link = dir.join("link.bin");
        std::os::unix::fs::symlink(&a, &link).unwrap();
        let link = link.to_string_lossy().to_string();

        let result = verify(&[a.clone(), link.clone()]);

        assert!(result.identical.is_empty());
        assert_eq!(result.unique, vec![a]);
        assert_eq!(result.skipped[0].path, link);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_same_path_listed_twice_is_not_its_own_duplicate() {
        let dir = test_dir("repeat");
        let a = write(&dir, "a.bin", &[1u8; 10]);

        let result = verify(&[a.clone(), a.to_uppercase(), a.clone()]);

        assert!(result.identical.is_empty());
        assert_eq!(result.unique, vec![a]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hard_link_is_not_a_second_copy() {
        let dir = test_dir("hardlink");
        let a = write(&dir, "a.bin", &[1u8; 10]);
        let link = dir.join("link.bin");
        fs::hard_link(&a, &link).unwrap();
        let link = link.to_string_lossy().to_string();

        let result = verify(&[a.clone(), link.clone()]);

        assert!(result.identical.is_empty());
        assert_eq!(result.unique, vec![a]);
        assert_eq!(result.skipped[0].path, link);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_numbers_preserve_all_128_bits_and_reject_zero() {
        assert_eq!(file_number([0; 16]), None);
        let id = (1_u128 << 127) | 2;
        assert_eq!(file_number(id.to_le_bytes()), Some(id));
        assert_ne!(
            file_number(id.to_le_bytes()),
            file_number(2_u128.to_le_bytes())
        );
    }

    #[test]
    fn missing_file_identity_uses_the_same_key_for_path_aliases() {
        let dir = test_dir("path-alias");
        let file = write(&dir, "a.bin", &[1u8]);
        let alias_dir = dir.join("nested");
        fs::create_dir(&alias_dir).unwrap();
        let alias = alias_dir.join("..").join("a.bin");
        let key = file_key(Path::new(&file), None).unwrap();
        let mut seen = HashSet::from([key]);
        assert!(!seen.insert(file_key(&alias, None).unwrap()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn names_differing_only_by_case_are_separate_files() {
        let dir = test_dir("case");
        let upper = write(&dir, "Same.bin", &[1u8; 10]);
        let lower = write(&dir, "same.bin", &[1u8; 10]);

        let result = verify(&[upper.clone(), lower.clone()]);

        assert_eq!(result.identical.len(), 1);
        assert_eq!(
            sorted(result.identical[0].clone()),
            sorted(vec![upper, lower])
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stops_when_cancelled() {
        let dir = test_dir("cancel");
        let a = write(&dir, "a.bin", &[1u8; 10]);
        let b = write(&dir, "b.bin", &[1u8; 10]);

        let result = verify_duplicate_contents(&[a, b], &AtomicBool::new(true));

        assert_eq!(result, Err(CANCELLED.to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stops_when_cancelled_without_hashing() {
        let dir = test_dir("cancel-single");
        let a = write(&dir, "a.bin", &[1u8; 10]);
        assert_eq!(
            verify_duplicate_contents(&[a], &AtomicBool::new(true)),
            Err(CANCELLED.to_string())
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_oversized_requests() {
        let paths: Vec<String> = (0..=MAX_VERIFY_PATHS).map(|i| format!("f{i}")).collect();
        assert!(verify_duplicate_contents(&paths, &AtomicBool::new(false)).is_err());
    }
}
