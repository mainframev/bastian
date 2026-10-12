use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use super::{Reader, collect_code};
use crate::doc::{self, Document};

impl Reader {
    /// Watch `path` and reload the document when it changes on disk.
    pub fn watch(&mut self, path: PathBuf) {
        self.modified = mtime(&path);
        self.source = Some(path);
    }

    /// Re-read the source file if its modification time moved.
    pub(super) fn check_reload(&mut self) -> bool {
        if self.last_check.elapsed() < RELOAD_CHECK {
            return false;
        }
        self.last_check = Instant::now();
        let Some(path) = &self.source else {
            return false;
        };
        let modified = mtime(path);
        if modified == self.modified {
            return false;
        }
        self.modified = modified;
        match crate::text::read_text(path) {
            Ok(src) => {
                self.replace_doc(doc::parse_for(&src, Some(path)));
                self.text = src;
                self.message = Some("reloaded".into());
            }
            Err(e) => self.message = Some(format!("reload failed: {e}")),
        }
        true
    }

    /// Swap in a new version of the document, keeping scroll and any run
    /// state whose block still exists.
    pub fn replace_doc(&mut self, doc: Document) {
        let codes = collect_code(&doc.blocks);
        // A pending prompt or queue names blocks by index. Once the blocks
        // change, those indexes point at code the user never agreed to run.
        let same = codes.len() == self.codes.len()
            && codes
                .iter()
                .zip(&self.codes)
                .all(|(a, b)| a.lang == b.lang && a.attrs == b.attrs && a.src == b.src);
        if !same {
            self.confirm = None;
            self.queue.clear();
        }
        self.runs
            .retain(|id, run| *id < codes.len() || run.is_running());
        if self.focus.is_some_and(|id| id >= codes.len()) {
            self.focus = None;
        }
        self.codes = codes;
        self.doc = doc;
        self.highlights.clear();
        self.images_requested = false;
        self.dirty = true;
    }
}

/// How often to check the source file for changes.
const RELOAD_CHECK: Duration = Duration::from_millis(500);

pub(super) fn mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}
