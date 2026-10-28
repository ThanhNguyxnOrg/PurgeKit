use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CachedDerived {
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub install_location: Option<String>,
    #[serde(default)]
    pub display_icon: Option<String>,
    #[serde(default)]
    pub icon_base64: Option<String>,
    #[serde(default)]
    pub estimated_size: Option<u64>,
    #[serde(default)]
    pub install_date: Option<String>,
    #[serde(default)]
    pub is_verified: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    entries: HashMap<String, CachedDerived>,
}

#[derive(Serialize)]
struct CacheFileRef<'a> {
    version: u32,
    entries: &'a HashMap<String, CachedDerived>,
}

#[derive(Default)]
struct CacheState {
    loaded: bool,
    entries: HashMap<String, CachedDerived>,
    seen: HashSet<String>,
    dirty: bool,
}

static CACHE: OnceLock<Mutex<CacheState>> = OnceLock::new();

#[derive(Debug, Serialize)]
pub struct DerivedCacheSummary {
    pub entries: usize,
    pub bytes: u64,
}

fn cache_state() -> &'static Mutex<CacheState> {
    CACHE.get_or_init(|| Mutex::new(CacheState::default()))
}

fn with_state<R>(action: impl FnOnce(&mut CacheState) -> R) -> R {
    let mut state = cache_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !state.loaded {
        state.entries = read_cache_file();
        state.loaded = true;
    }
    action(&mut state)
}

#[cfg(not(test))]
fn cache_path() -> PathBuf {
    crate::db::get_db_path()
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("derived_cache.json")
}

#[cfg(test)]
fn cache_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "purgekit_derived_cache_test_{}.json",
        std::process::id()
    ))
}

fn read_cache_file() -> HashMap<String, CachedDerived> {
    let Ok(raw) = fs::read_to_string(cache_path()) else {
        return HashMap::new();
    };
    let Ok(file) = serde_json::from_str::<CacheFile>(&raw) else {
        return HashMap::new();
    };
    if file.version != CACHE_VERSION {
        return HashMap::new();
    }
    file.entries
}

fn write_cache_file(entries: &HashMap<String, CachedDerived>) {
    let Ok(json) = serde_json::to_vec(&CacheFileRef {
        version: CACHE_VERSION,
        entries,
    }) else {
        return;
    };
    let _ = fs::write(cache_path(), json);
}

pub fn lookup(key: &str, fingerprint: &str) -> Option<CachedDerived> {
    with_state(|state| {
        state.seen.insert(key.to_string());
        let entry = state.entries.get(key)?;
        if entry.fingerprint != fingerprint {
            return None;
        }
        Some(entry.clone())
    })
}

pub fn store(key: String, value: CachedDerived) {
    with_state(|state| {
        state.seen.insert(key.clone());
        state.entries.insert(key, value);
        state.dirty = true;
    });
}

pub fn finish_scan() {
    let mut state = cache_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !state.loaded || state.seen.is_empty() {
        return;
    }

    let seen = std::mem::take(&mut state.seen);
    let before = state.entries.len();
    state.entries.retain(|key, _| seen.contains(key));
    if state.entries.len() != before {
        state.dirty = true;
    }
    if state.dirty {
        write_cache_file(&state.entries);
        state.dirty = false;
    }
}

pub fn clear_cache() -> Result<usize, String> {
    let removed = with_state(|state| {
        let removed = state.entries.len();
        state.entries.clear();
        state.seen.clear();
        state.dirty = false;
        removed
    });
    match fs::remove_file(cache_path()) {
        Ok(()) => Ok(removed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(removed),
        Err(error) => Err(format!("Failed to delete the app cache: {}", error)),
    }
}

pub fn summary() -> DerivedCacheSummary {
    let entries = with_state(|state| state.entries.len());
    let bytes = fs::metadata(cache_path())
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    DerivedCacheSummary { entries, bytes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_derived_fields_round_trip_through_json() {
        let mut entries = HashMap::new();
        entries.insert(
            "HKLM|SOFTWARE\\App".to_string(),
            CachedDerived {
                fingerprint: "v1|App|1.0".to_string(),
                install_location: Some(r"C:\Program Files\App".to_string()),
                display_icon: Some(r"C:\Program Files\App\app.exe".to_string()),
                icon_base64: Some("AAAA".to_string()),
                estimated_size: Some(2048),
                install_date: Some("2026-10-01".to_string()),
                is_verified: Some(true),
            },
        );
        let file = CacheFile {
            version: CACHE_VERSION,
            entries,
        };
        let encoded = serde_json::to_string(&file).unwrap();
        let decoded: CacheFile = serde_json::from_str(&encoded).unwrap();
        let entry = decoded.entries.get("HKLM|SOFTWARE\\App").unwrap();
        assert_eq!(entry.fingerprint, "v1|App|1.0");
        assert_eq!(entry.estimated_size, Some(2048));
        assert_eq!(entry.is_verified, Some(true));
    }

    #[test]
    fn rejects_entries_from_other_versions() {
        let encoded = r#"{"version":999,"entries":{"a":{}}}"#;
        let file: CacheFile = serde_json::from_str(encoded).unwrap();
        assert_ne!(file.version, CACHE_VERSION);
    }
}
