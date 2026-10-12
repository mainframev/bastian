//! Where each file was last read, so reopening it picks up there. Kept as
//! source lines, not rendered rows, so a different width still lands on
//! the same section.
//!
//! Losing a position never matters enough to interrupt reading, so I/O
//! errors here are ignored.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Entries older than this are dropped.
const MAX_AGE_SECS: u64 = 90 * 24 * 60 * 60;
/// At most this many files are remembered; the oldest go first.
const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Entry {
    /// 1-based source line at the top of the view.
    line: usize,
    /// When it was saved, in seconds since the Unix epoch.
    at: u64,
}

/// `$XDG_STATE_HOME/bastian/positions.json`, else under `~/.local/state`.
pub fn default_store() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/state"),
    };
    Some(base.join("bastian").join("positions.json"))
}

/// The line `file` was last read at, if remembered.
pub fn load(store: &Path, file: &Path) -> Option<usize> {
    read(store).get(&key(file)?).map(|e| e.line)
}

/// Remember `line` for `file`; `None` forgets it (back at the top).
pub fn save(store: &Path, file: &Path, line: Option<usize>) {
    let Some(key) = key(file) else { return };
    let now = now();
    let mut entries = read(store);
    // Saving the same line again still refreshes its age.
    let changed = match line {
        Some(line) => {
            entries.insert(key, Entry { line, at: now });
            true
        }
        None => entries.remove(&key).is_some(),
    };
    let before = entries.len();
    prune(&mut entries, now);
    if changed || entries.len() != before {
        write(store, &entries);
    }
}

fn prune(entries: &mut HashMap<String, Entry>, now: u64) {
    entries.retain(|_, e| now.saturating_sub(e.at) <= MAX_AGE_SECS);
    if entries.len() > MAX_ENTRIES {
        let mut ages: Vec<u64> = entries.values().map(|e| e.at).collect();
        ages.sort_unstable_by(|a, b| b.cmp(a));
        let cutoff = ages[MAX_ENTRIES - 1];
        entries.retain(|_, e| e.at >= cutoff);
        // Ties at the cutoff can leave a few extra; drop them arbitrarily.
        let extra: Vec<String> = entries.keys().skip(MAX_ENTRIES).cloned().collect();
        for k in extra {
            entries.remove(&k);
        }
    }
}

/// Files are keyed by their canonical path, so `./a.md` and `a.md` match.
fn key(file: &Path) -> Option<String> {
    Some(file.canonicalize().ok()?.to_string_lossy().into_owned())
}

fn read(store: &Path) -> HashMap<String, Entry> {
    std::fs::read(store)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Write via a temporary file and rename, so a crash or a second bastian
/// never leaves a half-written store.
fn write(store: &Path, entries: &HashMap<String, Entry>) {
    let Some(dir) = store.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let Ok(json) = serde_json::to_vec(entries) else {
        return;
    };
    let tmp = store.with_extension(format!("json.{}", std::process::id()));
    if std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, store).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_forget() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("state/positions.json");
        let file = dir.path().join("a.md");
        std::fs::write(&file, "# A\n").unwrap();

        assert_eq!(load(&store, &file), None);
        save(&store, &file, Some(42));
        assert_eq!(load(&store, &file), Some(42));
        // The same file by another path.
        let other = dir.path().join("state/../a.md");
        assert_eq!(load(&store, &other), Some(42));
        save(&store, &file, None);
        assert_eq!(load(&store, &file), None);
    }

    #[test]
    fn missing_or_broken_store_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("positions.json");
        let file = dir.path().join("a.md");
        std::fs::write(&file, "").unwrap();
        std::fs::write(&store, "not json").unwrap();
        assert_eq!(load(&store, &file), None);
        save(&store, &file, Some(3));
        assert_eq!(load(&store, &file), Some(3));
    }

    #[test]
    fn prunes_old_and_excess_entries() {
        let mut entries: HashMap<String, Entry> = (0..MAX_ENTRIES + 10)
            .map(|i| {
                (
                    format!("f{i}"),
                    Entry {
                        line: 1,
                        at: 1_000 + i as u64,
                    },
                )
            })
            .collect();
        entries.insert("stale".into(), Entry { line: 1, at: 0 });
        prune(&mut entries, 1_000 + MAX_AGE_SECS);
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert!(!entries.contains_key("stale"));
        // The oldest of the rest went.
        assert!(!entries.contains_key("f0"));
        assert!(entries.contains_key(&format!("f{}", MAX_ENTRIES + 9)));
    }
}
