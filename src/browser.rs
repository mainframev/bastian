use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::SystemTime;

use ignore::WalkState;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str, Utf32String};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::prompt::{self, Prompt, PromptEvent};

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
    /// What the filter matches against: the path, then the title.
    key: Utf32String,
}

impl Entry {
    pub fn new(
        path: PathBuf,
        rel: String,
        title: Option<String>,
        modified: Option<SystemTime>,
        created: Option<SystemTime>,
    ) -> Self {
        let hay = match &title {
            Some(t) => format!("{rel} {t}"),
            None => rel.clone(),
        };
        // Same representation `Utf32Str::new` gives, kept for every keystroke.
        let mut buf = Vec::new();
        let key = match Utf32Str::new(&hay, &mut buf) {
            Utf32Str::Ascii(_) => Utf32String::Ascii(hay.into()),
            Utf32Str::Unicode(chars) => Utf32String::Unicode(chars.into()),
        };
        Self {
            path,
            rel,
            title,
            modified,
            created,
            key,
        }
    }
}

pub enum Action {
    None,
    Quit,
    Open(PathBuf),
    Edit(PathBuf),
}

pub struct Browser {
    pub root: PathBuf,
    /// `root` for display, with the home directory as `~`.
    pub root_label: String,
    /// List hidden and gitignored files too.
    pub all: bool,
    pub entries: Vec<Entry>,
    /// Indices into `entries` that match the filter, best first.
    pub visible: Vec<usize>,
    pub selected: usize,
    pub filter: String,
    pub prompt: Option<Prompt>,
    /// One-shot error shown in the status bar until the next key.
    pub message: Option<String>,
    pub height: usize,
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
    /// `tick` to pick them up. `all` also lists hidden and gitignored files.
    pub fn new(root: PathBuf, all: bool) -> Self {
        let mut b = Self {
            root_label: display_root(&root),
            root,
            all,
            visible: Vec::new(),
            entries: Vec::new(),
            selected: 0,
            filter: String::new(),
            prompt: None,
            message: None,
            height: 0,
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
        let all = self.all;
        std::thread::spawn(move || scan(&root, all, tx));
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
        let max = self.visible.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(max);
    }

    /// Set how many entries fit on a page. The list shows the page holding
    /// the selection, turning a whole screen at a time rather than
    /// scrolling entry by entry.
    pub fn layout(&mut self, rows: usize) {
        self.height = rows.max(1);
    }

    /// Index of the first entry on the current page.
    pub fn offset(&self) -> usize {
        let per = self.height.max(1);
        self.selected / per * per
    }

    /// Current page and page count, both 1-based.
    pub fn page(&self) -> (usize, usize) {
        let per = self.height.max(1);
        let pages = self.visible.len().div_ceil(per).max(1);
        (self.selected / per + 1, pages)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        self.message = None;
        // Navigation keys end filter input and act right away, so moving
        // through the results doesn't need an Enter first.
        if self.prompt.is_some() && is_navigation(&key) {
            self.prompt = None;
        }
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
        let ctrl = prompt::ctrl(&key);
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
            KeyCode::Char('/') => self.prompt = Some(Prompt::new(&self.filter)),
            KeyCode::Char('j' | 'n') if ctrl => self.move_by(1),
            KeyCode::Char('k' | 'p') if ctrl => self.move_by(-1),
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

/// Keys that move the selection rather than edit filter text. Plain
/// letters (even `j`/`k`) stay text, and ctrl-u keeps clearing the input.
fn is_navigation(key: &KeyEvent) -> bool {
    let ctrl = prompt::ctrl(key);
    match key.code {
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::PageUp
        | KeyCode::PageDown => true,
        KeyCode::Char('j' | 'k' | 'n' | 'p' | 'd' | 'f' | 'b') => ctrl,
        _ => false,
    }
}

/// Markdown files under `root`, honoring .gitignore and skipping hidden
/// paths, newest first.
pub fn discover(root: &Path, all: bool) -> Vec<Entry> {
    let (tx, rx) = mpsc::channel();
    scan(root, all, tx);
    let mut entries: Vec<Entry> = rx.into_iter().collect();
    sort(&mut entries);
    entries
}

/// Absolute path with the home directory shortened to `~`.
fn display_root(root: &Path) -> String {
    let abs = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    match std::env::var_os("HOME").and_then(|h| abs.strip_prefix(h).ok().map(Path::to_path_buf)) {
        Some(rel) if rel.as_os_str().is_empty() => "~".to_string(),
        Some(rel) => format!("~/{}", rel.display()),
        None => abs.display().to_string(),
    }
}

fn sort(entries: &mut [Entry]) {
    entries.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.rel.cmp(&b.rel)));
}

/// Walk `root` on several threads, sending each Markdown file as it's found.
/// Returns when the walk is done. Hidden and gitignored paths are skipped
/// unless `all` is set; unreadable entries and broken symlinks are skipped.
pub fn scan(root: &Path, all: bool, tx: Sender<Entry>) {
    ignore::WalkBuilder::new(root)
        .hidden(!all)
        .git_ignore(!all)
        .git_global(!all)
        .git_exclude(!all)
        .ignore(!all)
        // Even with `all`, git's own data is never documentation.
        .filter_entry(|e| e.file_name() != ".git")
        .build_parallel()
        .run(|| {
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
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                let entry = Entry::new(
                    path.clone(),
                    rel,
                    read_title(&path),
                    meta.as_ref().and_then(|m| m.modified().ok()),
                    meta.as_ref()
                        .and_then(|m| m.created().or_else(|_| m.modified()).ok()),
                );
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
    title_of(&crate::text::decode_lossy(&buf))
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
        .is_some_and(crate::doc::is_markdown_ext)
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
    use ratatui::crossterm::event::KeyModifiers;

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

        let mut rels: Vec<String> = discover(root, false).into_iter().map(|e| e.rel).collect();
        rels.sort();
        assert_eq!(rels, ["README.md", "docs/guide.markdown"]);

        // With `all`, hidden and ignored files are listed too.
        std::fs::create_dir_all(root.join(".notes")).unwrap();
        std::fs::write(root.join(".notes/todo.mkd"), "# todo\n").unwrap();
        let mut rels: Vec<String> = discover(root, true).into_iter().map(|e| e.rel).collect();
        rels.sort();
        assert_eq!(
            rels,
            [
                ".notes/todo.mkd",
                "README.md",
                "build/out.md",
                "docs/guide.markdown"
            ]
        );
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
        let mut b = Browser::new(dir.path().to_path_buf(), false);
        wait_for_scan(&mut b);
        assert_eq!(b.entries.len(), 50);
        assert_eq!(b.visible.len(), 50);

        // A file appearing during a scan doesn't move the cursor off its file.
        b.selected = 10;
        let picked = b.selected_entry().unwrap().path.clone();
        let (tx, rx) = mpsc::channel();
        b.incoming = Some(rx);
        tx.send(Entry::new(
            dir.path().join("new.md"),
            "new.md".into(),
            None,
            Some(SystemTime::now() + std::time::Duration::from_secs(60)),
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
        let mut b = Browser::new(dir.path().to_path_buf(), false);
        wait_for_scan(&mut b);
        assert_eq!(b.selected, 0);
    }

    #[test]
    fn ctrl_paging_moves_selection() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..40 {
            std::fs::write(dir.path().join(format!("f{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), false);
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
        let mut b = Browser::new(dir.path().to_path_buf(), false);
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
        let mut b = Browser::new(dir.path().to_path_buf(), false);
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
        let mut b = Browser::new(dir.path().to_path_buf(), false);
        wait_for_scan(&mut b);
        b.layout(10);
        assert_eq!((b.offset(), b.page()), (0, (1, 3)));
        b.selected = 9;
        b.layout(10);
        assert_eq!((b.offset(), b.page()), (0, (1, 3)));
        // One step past the last row turns to the next page.
        b.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        b.layout(10);
        assert_eq!((b.offset(), b.page()), (10, (2, 3)));
        b.selected = 24;
        b.layout(10);
        assert_eq!((b.offset(), b.page()), (20, (3, 3)));

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
    fn navigation_keys_leave_filter_input() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..30 {
            std::fs::write(dir.path().join(format!("doc{i}.md")), "x\n").unwrap();
        }
        let mut b = Browser::new(dir.path().to_path_buf(), false);
        wait_for_scan(&mut b);
        b.layout(10);
        let key = |c, m| KeyEvent::new(c, m);
        b.on_key(key(KeyCode::Char('/'), KeyModifiers::NONE));
        for c in "doc".chars() {
            b.on_key(key(KeyCode::Char(c), KeyModifiers::NONE));
        }
        // Plain j/k are still filter text.
        b.on_key(key(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(b.prompt.as_ref().unwrap().text, "docj");
        b.on_key(key(KeyCode::Backspace, KeyModifiers::NONE));

        // ctrl-j closes the input, keeps the filter and moves.
        b.on_key(key(KeyCode::Char('j'), KeyModifiers::CONTROL));
        assert!(b.prompt.is_none());
        assert_eq!(b.filter, "doc");
        assert_eq!(b.selected, 1);

        // Same for paging straight from the input.
        b.on_key(key(KeyCode::Char('/'), KeyModifiers::NONE));
        b.on_key(key(KeyCode::Right, KeyModifiers::NONE));
        assert!(b.prompt.is_none());
        assert_eq!(b.selected, 11);

        // ctrl-u still clears the text rather than paging.
        b.on_key(key(KeyCode::Char('/'), KeyModifiers::NONE));
        b.on_key(key(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(b.prompt.as_ref().unwrap().text, "");
    }

    #[test]
    fn filter_ranks_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("install.md"), "# Installation\n").unwrap();
        std::fs::write(dir.path().join("usage.md"), "# Usage\n").unwrap();
        let mut b = Browser::new(dir.path().to_path_buf(), false);
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
