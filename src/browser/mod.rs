mod entry;
mod keys;
mod scan;

pub use entry::Entry;
pub use scan::{Scan, discover};

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

use crate::nav;
use crate::paths;
use crate::prompt::Prompt;

pub enum Action {
    None,
    Quit,
    Open(PathBuf),
    Edit(PathBuf),
    /// Open the theme picker.
    PickTheme,
}

pub struct Browser {
    pub root: PathBuf,
    /// `root` for display, with the home directory as `~`.
    pub root_label: String,
    /// Which files to list, and how to order them.
    pub scan: Scan,
    pub entries: Vec<Entry>,
    /// Indices into `entries` that match the filter, best first.
    pub visible: Vec<usize>,
    pub selected: nav::Selection,
    pub filter: String,
    pub prompt: Option<Prompt>,
    /// One-shot error shown in the status bar until the next key.
    pub message: Option<String>,
    /// Entries per page, at least 1: only `layout` sets it.
    height: usize,
    /// Entries still arriving from the background scan.
    incoming: Option<Receiver<Entry>>,
    /// When the scan started, for the loading spinner.
    pub started: std::time::Instant,
    /// Key help overlay is open.
    pub help: nav::Help,
    matcher: Matcher,
}

impl Browser {
    /// Start browsing `root`. Files are found in the background; call
    /// `tick` to pick them up.
    pub fn new(root: PathBuf, scan: Scan) -> Self {
        let mut b = Self {
            root_label: paths::tilde(
                &std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone()),
            ),
            root,
            scan,
            visible: Vec::new(),
            entries: Vec::new(),
            selected: nav::Selection::default(),
            filter: String::new(),
            prompt: None,
            message: None,
            height: 1,
            incoming: None,
            started: std::time::Instant::now(),
            help: nav::Help::default(),
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
        };
        b.rescan();
        b
    }

    /// Forget the current list and scan `root` again in the background.
    pub fn rescan(&mut self) {
        let (tx, rx) = mpsc::channel();
        let root = self.root.clone();
        let opts = self.scan.clone();
        std::thread::spawn(move || scan::scan(&root, &opts, &tx));
        self.entries.clear();
        self.refilter();
        self.incoming = Some(rx);
        self.started = std::time::Instant::now();
    }

    pub fn is_scanning(&self) -> bool {
        self.incoming.is_some()
    }

    /// Add files found since the last call. Returns true when a redraw is needed.
    pub fn tick(&mut self) -> bool {
        let Some(rx) = &self.incoming else {
            return false;
        };
        let mut received = false;
        loop {
            match rx.try_recv() {
                Ok(entry) => {
                    self.entries.push(entry);
                    received = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.incoming = None;
                    break;
                }
            }
        }
        if received {
            // Until the user moves off the first row, stay at the top.
            let selected = (self.selected.get() > 0)
                .then(|| self.selected_entry().map(|e| e.path.clone()))
                .flatten();
            scan::sort(&mut self.entries, self.scan.sort);
            self.refilter();
            // Keep the cursor on the same file while the list grows.
            if let Some(path) = selected
                && let Some(pos) = self
                    .visible
                    .iter()
                    .position(|&i| self.entries[i].path == path)
            {
                self.selected = nav::Selection::at(pos);
            }
        }
        // While scanning, always redraw so the spinner animates; once done,
        // redraw one last time to drop it.
        true
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.visible
            .get(self.selected.get())
            .map(|&i| &self.entries[i])
    }

    fn refilter(&mut self) {
        self.selected = nav::Selection::default();
        if self.filter.is_empty() {
            self.visible = (0..self.entries.len()).collect();
            return;
        }
        let pattern = Pattern::parse(&self.filter, CaseMatching::Smart, Normalization::Smart);
        let mut scored: Vec<(usize, u32)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let score = pattern.score(e.key.slice(..), &mut self.matcher)?;
                Some((i, score))
            })
            .collect();
        // Stable sort keeps the recency order among equal scores.
        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        self.visible = scored.into_iter().map(|(i, _)| i).collect();
    }

    /// Move the selection by `delta` entries.
    pub fn move_by(&mut self, delta: isize) {
        self.selected.by(delta, self.visible.len());
    }

    /// Set how many entries fit on a page. The list shows the page holding
    /// the selection, turning a whole screen at a time rather than
    /// scrolling entry by entry.
    pub fn layout(&mut self, rows: usize) {
        self.height = rows.max(1);
    }

    /// Entries per page, as last set by `layout`.
    pub fn page_len(&self) -> usize {
        self.height
    }

    /// Index of the first entry on the current page.
    pub fn offset(&self) -> usize {
        let per = self.height;
        self.selected.get() / per * per
    }

    /// Current page and page count, both 1-based.
    pub fn page(&self) -> (usize, usize) {
        let per = self.height;
        let pages = self.visible.len().div_ceil(per).max(1);
        (self.selected.get() / per + 1, pages)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::time::SystemTime;

    pub(crate) fn wait_for_scan(b: &mut Browser) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while b.is_scanning() {
            assert!(std::time::Instant::now() < deadline, "scan did not finish");
            b.tick();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn background_scan_fills_list_and_keeps_selection() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..50 {
            std::fs::write(dir.path().join(format!("f{i}.md")), format!("# Doc {i}\n")).unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), Scan::default());
        wait_for_scan(&mut b);
        assert_eq!(b.entries.len(), 50);
        assert_eq!(b.visible.len(), 50);

        // A file appearing during a scan doesn't move the cursor off its file.
        b.selected = nav::Selection::at(10);
        let picked = b.selected_entry().unwrap().path.clone();
        let (tx, rx) = mpsc::channel();
        b.incoming = Some(rx);
        tx.send(Entry::new(
            dir.path().join("new.md"),
            "new.md".into(),
            None,
            Some(SystemTime::now() + std::time::Duration::from_mins(1)),
            None,
        ))
        .unwrap();
        drop(tx);
        b.tick();
        assert_eq!(b.selected_entry().unwrap().path, picked);
        assert!(!b.is_scanning());
    }

    #[test]
    fn cursor_stays_on_top_row_while_scanning() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..30 {
            std::fs::write(dir.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), Scan::default());
        wait_for_scan(&mut b);
        assert_eq!(b.selected.get(), 0);
    }

    #[test]
    fn filter_ranks_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("install.md"), "# Installation\n").unwrap();
        std::fs::write(dir.path().join("usage.md"), "# Usage\n").unwrap();
        let mut b = Browser::new(dir.path().to_path_buf(), Scan::default());
        wait_for_scan(&mut b);
        b.filter = "instal".into();
        b.refilter();
        let names: Vec<&str> = b
            .visible
            .iter()
            .map(|&i| b.entries[i].rel.as_str())
            .collect();
        assert_eq!(names, ["install.md"]);
    }
}
