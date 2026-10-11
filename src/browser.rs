use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::SystemTime;

use ignore::WalkState;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::prompt::{Prompt, PromptEvent};

/// A Markdown file found under the browsed directory.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    /// Path relative to the browsed directory, for display and matching.
    pub rel: String,
    pub title: Option<String>,
    pub modified: Option<SystemTime>,
    /// Creation time, or the modification time where the filesystem
    /// doesn't record one.
    pub created: Option<SystemTime>,
}

pub enum Action {
    None,
    Quit,
    Open(PathBuf),
    Edit(PathBuf),
}

pub struct Browser {
    pub root: PathBuf,
    pub entries: Vec<Entry>,
    /// Indices into `entries` that match the filter, best first.
    pub visible: Vec<usize>,
    pub selected: usize,
    pub filter: String,
    pub prompt: Option<Prompt>,
    /// One-shot error shown in the status bar until the next key.
    pub message: Option<String>,
    pub height: usize,
    pub offset: usize,
    /// Entries still arriving from the background scan.
    incoming: Option<Receiver<Entry>>,
    /// When the scan started, for the loading spinner.
    pub started: std::time::Instant,
    /// Key help overlay is open.
    pub help: bool,
    matcher: Matcher,
}

impl Browser {
    /// Start browsing `root`. Files are found in the background; call
    /// `tick` to pick them up.
    pub fn new(root: PathBuf) -> Self {
        let mut b = Self {
            root,
            visible: Vec::new(),
            entries: Vec::new(),
            selected: 0,
            filter: String::new(),
            prompt: None,
            message: None,
            height: 0,
            offset: 0,
            incoming: None,
            started: std::time::Instant::now(),
            help: false,
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
        };
        b.rescan();
        b
    }

    /// Forget the current list and scan `root` again in the background.
    pub fn rescan(&mut self) {
        let (tx, rx) = mpsc::channel();
        let root = self.root.clone();
        std::thread::spawn(move || scan(&root, tx));
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
            let selected = (self.selected > 0)
                .then(|| self.selected_entry().map(|e| e.path.clone()))
                .flatten();
            sort(&mut self.entries);
            self.refilter();
            // Keep the cursor on the same file while the list grows.
            if let Some(path) = selected
                && let Some(pos) = self
                    .visible
                    .iter()
                    .position(|&i| self.entries[i].path == path)
            {
                self.selected = pos;
            }
        }
        // While scanning, always redraw so the spinner animates; once done,
        // redraw one last time to drop it.
        true
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.visible.get(self.selected).map(|&i| &self.entries[i])
    }

    fn refilter(&mut self) {
        self.selected = 0;
        self.offset = 0;
        if self.filter.is_empty() {
            self.visible = (0..self.entries.len()).collect();
            return;
        }
        let pattern = Pattern::parse(&self.filter, CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(usize, u32)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let hay = match &e.title {
                    Some(t) => format!("{} {t}", e.rel),
                    None => e.rel.clone(),
                };
                let score = pattern.score(Utf32Str::new(&hay, &mut buf), &mut self.matcher)?;
                Some((i, score))
            })
            .collect();
        // Stable sort keeps the recency order among equal scores.
        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        self.visible = scored.into_iter().map(|(i, _)| i).collect();
    }

    fn move_by(&mut self, delta: isize) {
        let max = self.visible.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(max);
    }

    /// Keep the selection inside the window of `height` visible rows.
    /// Show the page holding the selection. Like glow, the list turns a
    /// whole screen at a time rather than scrolling entry by entry.
    pub fn layout(&mut self, rows: usize) {
        self.height = rows.max(1);
        self.offset = self.selected / self.height * self.height;
    }

    /// Current page and page count, both 1-based.
    pub fn page(&self) -> (usize, usize) {
        let per = self.height.max(1);
        let pages = self.visible.len().div_ceil(per).max(1);
        (self.selected / per + 1, pages)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        self.message = None;
        if let Some(prompt) = &mut self.prompt {
            match prompt.on_key(key) {
                PromptEvent::Changed => {
                    self.filter = prompt.text.clone();
                    self.refilter();
                }
                PromptEvent::Submit => self.prompt = None,
                PromptEvent::Cancel => {
                    self.prompt = None;
                    self.filter.clear();
                    self.refilter();
                }
                PromptEvent::None => {}
            }
            return Action::None;
        }

        if self.help {
            self.help = false;
            return Action::None;
        }

        let page = self.height.max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('d') if ctrl => self.move_by(page / 2),
            KeyCode::Char('u') if ctrl => self.move_by(-page / 2),
            KeyCode::Char('f') if ctrl => self.move_by(page),
            KeyCode::Char('b') if ctrl => self.move_by(-page),
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refilter();
            }
            KeyCode::Esc => return Action::Quit,
            KeyCode::Char('/') => self.prompt = Some(Prompt::new("/", &self.filter)),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::PageDown => self.move_by(page),
            KeyCode::PageUp => self.move_by(-page),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.move_by(isize::MAX),
            KeyCode::Char('l') | KeyCode::Right => self.move_by(page),
            KeyCode::Char('h') | KeyCode::Left => self.move_by(-page),
            KeyCode::Enter => {
                if let Some(e) = self.selected_entry() {
                    return Action::Open(e.path.clone());
                }
            }
            KeyCode::Char('e') => {
                if let Some(e) = self.selected_entry() {
                    return Action::Edit(e.path.clone());
                }
            }
            KeyCode::Char('r') => self.rescan(),
            KeyCode::Char('?') => self.help = true,
            _ => {}
        }
        Action::None
    }
}

/// Markdown files under `root`, honoring .gitignore and skipping hidden
/// paths, newest first.
pub fn discover(root: &Path) -> Vec<Entry> {
    let (tx, rx) = mpsc::channel();
    scan(root, tx);
    let mut entries: Vec<Entry> = rx.into_iter().collect();
    sort(&mut entries);
    entries
}

fn sort(entries: &mut [Entry]) {
    entries.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.rel.cmp(&b.rel)));
}

/// Walk `root` on several threads, sending each Markdown file as it's found.
/// Returns when the walk is done.
pub fn scan(root: &Path, tx: Sender<Entry>) {
    ignore::WalkBuilder::new(root).build_parallel().run(|| {
        let tx = tx.clone();
        Box::new(move |result| {
            let Ok(d) = result else {
                return WalkState::Continue;
            };
            if !d.file_type().is_some_and(|t| t.is_file()) || !is_markdown(d.path()) {
                return WalkState::Continue;
            }
            let path = d.path().to_path_buf();
            let meta = d.metadata().ok();
            let entry = Entry {
                rel: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
                title: read_title(&path),
                modified: meta.as_ref().and_then(|m| m.modified().ok()),
                created: meta
                    .as_ref()
                    .and_then(|m| m.created().or_else(|_| m.modified()).ok()),
                path,
            };
            if tx.send(entry).is_err() {
                return WalkState::Quit;
            }
            WalkState::Continue
        })
    });
}

/// Bytes read from the start of each file to find its title.
const TITLE_PREFIX: u64 = 8 * 1024;

fn read_title(path: &Path) -> Option<String> {
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(TITLE_PREFIX).read_to_end(&mut buf))
        .ok()?;
    title_of(&String::from_utf8_lossy(&buf))
}

/// `title:` from YAML front matter, else the first `# ` heading.
fn title_of(src: &str) -> Option<String> {
    let mut body = src;
    if let Some(rest) = src
        .strip_prefix("---\n")
        .or_else(|| src.strip_prefix("---\r\n"))
    {
        let end = rest.find("\n---").unwrap_or(rest.len());
        let front = &rest[..end];
        let title = front.lines().find_map(|l| {
            let v = l
                .strip_prefix("title:")?
                .trim()
                .trim_matches(['"', '\''])
                .trim();
            (!v.is_empty()).then(|| v.to_string())
        });
        if title.is_some() {
            return title;
        }
        body = &rest[end..];
    }
    first_heading(body)
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown" | "mdx"))
}

/// First ATX `# ` heading outside fenced code, without a full parse.
fn first_heading(src: &str) -> Option<String> {
    let mut fenced = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        } else if !fenced && let Some(h) = t.strip_prefix("# ") {
            let h = h.trim().trim_end_matches('#').trim();
            if !h.is_empty() {
                return Some(h.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_skips_fences() {
        assert_eq!(
            first_heading("```sh\n# not a heading\n```\n# Real #\n").as_deref(),
            Some("Real")
        );
        assert_eq!(first_heading("## only h2\n"), None);
    }

    #[test]
    fn discovers_markdown_and_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(root.join("build")).unwrap();
        std::fs::write(root.join("README.md"), "# Readme\n").unwrap();
        std::fs::write(root.join("docs/guide.markdown"), "text\n").unwrap();
        std::fs::write(root.join("build/out.md"), "# ignored\n").unwrap();
        std::fs::write(root.join("notes.txt"), "# not md\n").unwrap();
        std::fs::write(root.join(".gitignore"), "build/\n").unwrap();
        // .gitignore is only honored inside a git repository.
        std::fs::create_dir(root.join(".git")).unwrap();

        let mut rels: Vec<String> = discover(root).into_iter().map(|e| e.rel).collect();
        rels.sort();
        assert_eq!(rels, ["README.md", "docs/guide.markdown"]);
    }

    fn wait_for_scan(b: &mut Browser) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while b.is_scanning() {
            assert!(std::time::Instant::now() < deadline, "scan did not finish");
            b.tick();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn title_from_front_matter_or_heading() {
        assert_eq!(
            title_of("---\ntitle: \"Guide\"\nlayout: x\n---\n# Heading\n").as_deref(),
            Some("Guide")
        );
        assert_eq!(
            title_of("---\nlayout: x\n---\n\n# Heading\n").as_deref(),
            Some("Heading")
        );
        assert_eq!(title_of("no title here\n"), None);
    }

    #[test]
    fn title_only_reads_file_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let late = dir.path().join("late.md");
        let filler = "x\n".repeat(TITLE_PREFIX as usize);
        std::fs::write(&late, format!("{filler}# Too late\n")).unwrap();
        assert_eq!(read_title(&late), None);
    }

    #[test]
    fn background_scan_fills_list_and_keeps_selection() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..50 {
            std::fs::write(dir.path().join(format!("f{i}.md")), format!("# Doc {i}\n")).unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf());
        wait_for_scan(&mut b);
        assert_eq!(b.entries.len(), 50);
        assert_eq!(b.visible.len(), 50);

        // A file appearing during a scan doesn't move the cursor off its file.
        b.selected = 10;
        let picked = b.selected_entry().unwrap().path.clone();
        let (tx, rx) = mpsc::channel();
        b.incoming = Some(rx);
        tx.send(Entry {
            path: dir.path().join("new.md"),
            rel: "new.md".into(),
            title: None,
            modified: Some(SystemTime::now() + std::time::Duration::from_secs(60)),
            created: None,
        })
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
        let mut b = Browser::new(dir.path().to_path_buf());
        wait_for_scan(&mut b);
        assert_eq!(b.selected, 0);
    }

    #[test]
    fn ctrl_paging_moves_selection() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..40 {
            std::fs::write(dir.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf());
        wait_for_scan(&mut b);
        b.layout(10);
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        b.on_key(ctrl('d'));
        assert_eq!(b.selected, 5);
        b.on_key(ctrl('f'));
        assert_eq!(b.selected, 15);
        b.on_key(ctrl('u'));
        assert_eq!(b.selected, 10);
        b.on_key(ctrl('b'));
        assert_eq!(b.selected, 0);
        // Clamped at both ends.
        b.on_key(ctrl('b'));
        assert_eq!(b.selected, 0);
        for _ in 0..10 {
            b.on_key(ctrl('f'));
        }
        assert_eq!(b.selected, 39);
    }

    #[test]
    fn ctrl_u_in_filter_clears_text_instead_of_paging() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..20 {
            std::fs::write(dir.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf());
        wait_for_scan(&mut b);
        b.layout(10);
        b.selected = 8;
        b.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        b.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        b.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(b.prompt.as_ref().unwrap().text, "");
    }

    #[test]
    fn edit_rescan_and_help_keys() {
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "# A\n").unwrap();
        let mut b = Browser::new(dir.path().to_path_buf());
        wait_for_scan(&mut b);
        assert!(matches!(b.on_key(key('e')), Action::Edit(p) if p == dir.path().join("a.md")));

        std::fs::write(dir.path().join("b.md"), "# B\n").unwrap();
        b.on_key(key('r'));
        wait_for_scan(&mut b);
        assert_eq!(b.entries.len(), 2);

        b.on_key(key('?'));
        assert!(b.help);
        // The key that closes help does nothing else.
        assert!(matches!(b.on_key(key('q')), Action::None));
        assert!(!b.help);
    }

    #[test]
    fn list_turns_whole_pages() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..25 {
            std::fs::write(dir.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf());
        wait_for_scan(&mut b);
        b.layout(10);
        assert_eq!((b.offset, b.page()), (0, (1, 3)));
        b.selected = 9;
        b.layout(10);
        assert_eq!((b.offset, b.page()), (0, (1, 3)));
        // One step past the last row turns to the next page.
        b.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        b.layout(10);
        assert_eq!((b.offset, b.page()), (10, (2, 3)));
        b.selected = 24;
        b.layout(10);
        assert_eq!((b.offset, b.page()), (20, (3, 3)));

        // h/l and ←/→ turn pages, keeping the row within the page.
        b.selected = 12;
        let press = |b: &mut Browser, code| {
            b.on_key(KeyEvent::new(code, KeyModifiers::NONE));
            b.layout(10);
        };
        press(&mut b, KeyCode::Char('h'));
        assert_eq!((b.selected, b.page()), (2, (1, 3)));
        press(&mut b, KeyCode::Char('l'));
        assert_eq!((b.selected, b.page()), (12, (2, 3)));
        press(&mut b, KeyCode::Right);
        assert_eq!((b.selected, b.page()), (22, (3, 3)));
        press(&mut b, KeyCode::Left);
        assert_eq!(b.page(), (2, 3));
    }

    #[test]
    fn filter_ranks_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("install.md"), "# Installation\n").unwrap();
        std::fs::write(dir.path().join("usage.md"), "# Usage\n").unwrap();
        let mut b = Browser::new(dir.path().to_path_buf());
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
