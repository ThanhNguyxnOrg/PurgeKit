use std::num::NonZeroUsize;
use std::sync::Mutex;

use usn_journal_rs::prelude::{JournalIterOptions, Usn, UsnReason, Volume};

pub struct TrackingSession {
    pub start_usn: u64,
    pub name: String,
    pub reg_baseline: Vec<String>,
    pub reg_value_baseline: Vec<String>,
}

pub struct ActiveTracking(pub std::sync::Arc<Mutex<Option<TrackingSession>>>);

/// Query the current tail of the volume USN journal. Requires Administrator.
pub fn query_current_usn(drive: char) -> Result<u64, String> {
    let volume = Volume::from_drive_letter(drive).map_err(|error| {
        format!(
            "Installation tracking requires Administrator access to the {}: volume: {}",
            drive, error
        )
    })?;
    let journal = volume
        .journal()
        .query_or_create()
        .map_err(|error| format!("Failed to query USN journal: {}", error))?;

    u64::try_from(journal.next_usn.get())
        .map_err(|_| "USN journal returned a negative cursor".to_string())
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
