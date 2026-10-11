use std::path::{Path, PathBuf};
use std::time::SystemTime;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::prompt::{Prompt, PromptEvent};

/// A Markdown file found under the browsed directory.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    /// Path relative to the browsed directory, for display and matching.
    pub rel: String,
    pub title: Option<String>,
    pub modified: Option<SystemTime>,
}

pub enum Action {
    None,
    Quit,
    Open(PathBuf),
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
    matcher: Matcher,
}

impl Browser {
    pub fn new(root: PathBuf) -> Self {
        let entries = discover(&root);
        let mut b = Self {
            root,
            visible: (0..entries.len()).collect(),
            entries,
            selected: 0,
            filter: String::new(),
            prompt: None,
            message: None,
            height: 0,
            offset: 0,
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
        };
        b.refilter();
        b
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
    pub fn layout(&mut self, rows: usize) {
        self.height = rows.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + self.height {
            self.offset = self.selected + 1 - self.height;
        }
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

        let page = self.height.max(1) as isize;
        match key.code {
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
            KeyCode::Enter | KeyCode::Char('l') => {
                if let Some(e) = self.selected_entry() {
                    return Action::Open(e.path.clone());
                }
            }
            _ => {}
        }
        Action::None
    }
}

/// Markdown files under `root`, honoring .gitignore and skipping hidden
/// paths, newest first.
pub fn discover(root: &Path) -> Vec<Entry> {
    let mut entries: Vec<Entry> = ignore::WalkBuilder::new(root)
        .build()
        .filter_map(Result::ok)
        .filter(|d| d.file_type().is_some_and(|t| t.is_file()) && is_markdown(d.path()))
        .map(|d| {
            let path = d.path().to_path_buf();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            Entry {
                title: std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| first_heading(&s)),
                modified: d.metadata().ok().and_then(|m| m.modified().ok()),
                rel,
                path,
            }
        })
        .collect();
    entries.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.rel.cmp(&b.rel)));
    entries
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

    #[test]
    fn filter_ranks_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("install.md"), "# Installation\n").unwrap();
        std::fs::write(dir.path().join("usage.md"), "# Usage\n").unwrap();
        let mut b = Browser::new(dir.path().to_path_buf());
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
