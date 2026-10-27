use std::num::NonZeroUsize;
use std::sync::Mutex;

use usn_journal_rs::prelude::{JournalIterOptions, Usn, UsnReason, Volume};

#[derive(Clone)]
pub struct TrackingSession {
    pub start_usns: Vec<(char, u64)>,
    pub name: String,
    pub reg_baseline: Vec<String>,
    pub reg_value_baseline: Vec<String>,
}

pub struct ActiveTracking(pub std::sync::Arc<Mutex<Option<TrackingSession>>>);

/// Query the current tail of the volume USN journal. Requires Administrator
/// and an existing journal; tracking never creates a journal as a side effect.
pub fn query_current_usn(drive: char) -> Result<u64, String> {
    let volume = Volume::from_drive_letter(drive).map_err(|error| {
        format!(
            "Installation tracking requires Administrator access to the {}: volume: {}",
            drive, error
        )
    })?;
    let journal = volume.journal().query().map_err(|error| {
        format!(
            "No active USN journal is available on {}: ({}). Enable it with 'fsutil usn createjournal m=... a=...' or choose an NTFS volume with journaling enabled.",
            drive, error
        )
    })?;

    u64::try_from(journal.next_usn.get())
        .map_err(|_| "USN journal returned a negative cursor".to_string())
}

/// Merge USN changes from every tracked volume, skipping volumes that have
/// gone away since tracking started.
pub fn collect_usn_changes<F>(start_usns: &[(char, u64)], mut read: F) -> Vec<String>
where
    F: FnMut(char, u64) -> Result<Vec<String>, String>,
{
    let mut changes = Vec::new();
    for &(drive, start_usn) in start_usns {
        if let Ok(mut paths) = read(drive, start_usn) {
            changes.append(&mut paths);
        }
    }
    changes.sort();
    changes.dedup();
    changes
}

/// Select one USN cursor per journaled volume, always requiring the system
/// volume. Volumes without an active journal are skipped so a USB/FAT volume
/// cannot block tracking.
pub fn collect_start_cursors<F>(
    drives: &[char],
    system_drive: char,
    mut query: F,
) -> Result<Vec<(char, u64)>, String>
where
    F: FnMut(char) -> Result<u64, String>,
{
    let system_drive = system_drive.to_ascii_uppercase();
    let mut letters: Vec<char> = drives
        .iter()
        .map(|drive| drive.to_ascii_uppercase())
        .collect();
    if !letters.contains(&system_drive) {
        letters.push(system_drive);
    }
    letters.sort_unstable();
    letters.dedup();

    let mut cursors = Vec::new();
    for letter in letters {
        match query(letter) {
            Ok(usn) => cursors.push((letter, usn)),
            Err(error) => {
                if letter == system_drive {
                    return Err(error);
                }
            }
        }
    }
    Ok(cursors)
}

/// Read all available journal records from `start_usn` and resolve them to
/// current full paths. The iterator loops until the journal tail, so records
/// larger than one buffer are no longer silently dropped.
pub fn read_usn_changes(drive: char, start_usn: u64) -> Result<Vec<String>, String> {
    let volume = Volume::from_drive_letter(drive).map_err(|error| {
        format!(
            "Installation tracking requires Administrator access to the {}: volume: {}",
            drive, error
        )
    })?;
    let resolver = volume.path_resolver();

    let start_usn = i64::try_from(start_usn)
        .map_err(|_| "USN journal start cursor is too large".to_string())?;
    let options = JournalIterOptions::builder()
        .start_usn(Usn::new(start_usn))
        .reason_mask(UsnReason::ALL)
        .wait_for_more(false)
        .buffer_bytes(
            NonZeroUsize::new(256 * 1024)
                .ok_or_else(|| "Invalid USN journal buffer size".to_string())?,
        )
        .build()
        .map_err(|error| format!("Failed to configure USN journal iteration: {}", error))?;

    let mut paths = Vec::new();
    let entries = volume
        .journal()
        .try_iter_with_options(options)
        .map_err(|error| format!("Failed to read USN journal: {}", error))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to parse USN journal entry: {}", error))?;
        if entry.reason.is_file_delete() {
            continue;
        }

        if let Some(path) = resolver.resolve_path(&entry) {
            let path = path.to_string_lossy().to_string();
            if !path.is_empty() {
                paths.push(path);
            }
        }
    }

    paths.sort();
    paths.dedup();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_cursors_for_all_journaled_drives() {
        let cursors = collect_start_cursors(&['d', 'c'], 'C', |drive| match drive {
            'C' => Ok(10),
            'D' => Ok(20),
            _ => Err("unexpected drive".to_string()),
        })
        .unwrap();

        assert_eq!(cursors, vec![('C', 10), ('D', 20)]);
    }

    #[test]
    fn skips_volumes_without_a_journal_but_requires_the_system_drive() {
        let cursors = collect_start_cursors(&['C', 'D', 'E'], 'C', |drive| match drive {
            'C' => Ok(5),
            'D' => Err("no active journal".to_string()),
            'E' => Ok(7),
            _ => Err("unexpected drive".to_string()),
        })
        .unwrap();
        assert_eq!(cursors, vec![('C', 5), ('E', 7)]);

        let system_failure = collect_start_cursors(&['C', 'D'], 'C', |drive| {
            if drive == 'C' {
                Err("administrator access required".to_string())
            } else {
                Ok(1)
            }
        });
        assert!(system_failure.is_err());
    }

    #[test]
    fn always_includes_the_system_drive_and_deduplicates_letters() {
        let cursors = collect_start_cursors(&['D', 'D'], 'C', |drive| match drive {
            'C' => Ok(1),
            'D' => Ok(2),
            _ => Err("unexpected drive".to_string()),
        })
        .unwrap();

        assert_eq!(cursors, vec![('C', 1), ('D', 2)]);
    }

    #[test]
    fn merges_changes_across_volumes_and_deduplicates_paths() {
        let changes = collect_usn_changes(&[('C', 1), ('D', 2), ('E', 3)], |drive, _| match drive {
            'C' => Ok(vec![r"C:\a.txt".to_string(), r"C:\shared.txt".to_string()]),
            'D' => Ok(vec![r"D:\b.txt".to_string(), r"C:\shared.txt".to_string()]),
            _ => Err("volume went away".to_string()),
        });

        assert_eq!(
            changes,
            vec![
                r"C:\a.txt".to_string(),
                r"C:\shared.txt".to_string(),
                r"D:\b.txt".to_string()
            ]
        );
    }
}
