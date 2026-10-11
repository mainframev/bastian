use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::doc::{Block, CodeBlock, Document};
use crate::exec::{self, BlockRun, RunEvent, RunSpec, Runner, Status};
use crate::prompt::{Prompt, PromptEvent};
use crate::render::theme::Theme;
use crate::render::{self, Rendered};

pub enum Action {
    None,
    /// Go back to the browser, or quit when there is none.
    Back,
    Quit,
}

/// A search hit in rendered text, in char offsets within the line.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Default)]
pub struct Search {
    pub query: String,
    pub matches: Vec<Match>,
    pub current: usize,
}

/// State of the document reader view.
pub struct Reader {
    pub name: String,
    pub doc: Document,
    pub theme: Theme,
    pub max_width: u16,
    pub rendered: Rendered,
    pub scroll: usize,
    pub search: Option<Search>,
    pub prompt: Option<Prompt>,
    /// Focused code block id.
    pub focus: Option<usize>,
    pub runs: HashMap<usize, BlockRun>,
    /// Blocks waiting for a yes/no before they run.
    pub confirm: Option<Vec<usize>>,
    /// One-shot status bar message, cleared on the next key.
    pub message: Option<String>,
    /// Directory that block `cwd`s are relative to.
    pub base_dir: PathBuf,
    /// Skip confirmation, except for blocks marked `confirm`.
    pub assume_yes: bool,
    codes: Vec<CodeBlock>,
    runner: Runner,
    /// Remaining blocks of a run-all, started one after another.
    queue: VecDeque<usize>,
    timed_out: HashSet<usize>,
    /// Block whose output the view keeps in sight until the user scrolls.
    follow: Option<usize>,
    dirty: bool,
    width: u16,
    height: usize,
}

impl Reader {
    pub fn new(name: String, doc: Document, theme: Theme, max_width: u16) -> Self {
        let mut codes = Vec::new();
        collect_code(&doc.blocks, &mut codes);
        Self {
            name,
            doc,
            theme,
            max_width,
            rendered: Rendered::default(),
            scroll: 0,
            search: None,
            prompt: None,
            focus: None,
            runs: HashMap::new(),
            confirm: None,
            message: None,
            base_dir: PathBuf::from("."),
            assume_yes: false,
            codes,
            runner: Runner::default(),
            queue: VecDeque::new(),
            timed_out: HashSet::new(),
            follow: None,
            dirty: false,
            width: 0,
            height: 0,
        }
    }

    /// Re-render when the width or run output changed, and keep scroll in range.
    pub fn layout(&mut self, width: u16, height: u16) {
        if width != self.width || self.dirty {
            self.rendered = render::render_with_runs(&self.doc, width, &self.theme, &self.runs);
            self.width = width;
            self.dirty = false;
            if let Some(s) = &mut self.search {
                s.matches = find(&self.rendered, &s.query);
                s.current = s.current.min(s.matches.len().saturating_sub(1));
            }
        }
        self.height = usize::from(height);
        self.follow_output();
        self.scroll = self.scroll.min(self.max_scroll());
    }

    pub fn max_scroll(&self) -> usize {
        self.rendered.lines.len().saturating_sub(self.height)
    }

    /// Scroll down so the followed block's output stays visible, without
    /// pushing the top of the block off screen.
    fn follow_output(&mut self) {
        let Some(range) = self
            .follow
            .and_then(|id| self.rendered.code_blocks.iter().find(|c| c.id == id))
        else {
            return;
        };
        let height = self.height.max(1);
        if range.output_end > self.scroll + height {
            let want = range.output_end - height;
            self.scroll = want.min(range.lines.start).max(self.scroll);
        }
    }

    fn scroll_by(&mut self, delta: isize) {
        self.follow = None;
        self.scroll = self
            .scroll
            .saturating_add_signed(delta)
            .min(self.max_scroll());
    }

    /// Scroll so `line` is visible, placing it a third of the way down.
    fn reveal(&mut self, line: usize) {
        let visible = self.scroll..self.scroll + self.height;
        if !visible.contains(&line) {
            self.scroll = line.saturating_sub(self.height / 3).min(self.max_scroll());
        }
    }

    fn set_query(&mut self, query: &str) {
        if query.is_empty() {
            self.search = None;
            return;
        }
        let matches = find(&self.rendered, query);
        // Start from the first hit at or below the current view.
        let current = matches
            .iter()
            .position(|m| m.line >= self.scroll)
            .unwrap_or(0);
        if let Some(m) = matches.get(current) {
            let line = m.line;
            self.reveal(line);
        }
        self.search = Some(Search {
            query: query.to_string(),
            matches,
            current,
        });
    }

    fn step_match(&mut self, forward: bool) {
        let Some(s) = &mut self.search else { return };
        let n = s.matches.len();
        if n == 0 {
            return;
        }
        s.current = if forward {
            (s.current + 1) % n
        } else {
            (s.current + n - 1) % n
        };
        let line = s.matches[s.current].line;
        self.reveal(line);
    }

    pub fn code(&self, id: usize) -> Option<&CodeBlock> {
        self.codes.iter().find(|c| c.id == id)
    }

    /// Position of the focused block among all code blocks, and their count.
    pub fn focus_position(&self) -> Option<(usize, usize)> {
        let id = self.focus?;
        let i = self.rendered.code_blocks.iter().position(|c| c.id == id)?;
        Some((i + 1, self.rendered.code_blocks.len()))
    }

    fn move_focus(&mut self, forward: bool) {
        let ranges = &self.rendered.code_blocks;
        if ranges.is_empty() {
            self.message = Some("no code blocks".into());
            return;
        }
        let current = self
            .focus
            .and_then(|id| ranges.iter().position(|c| c.id == id));
        let next = match (current, forward) {
            (Some(i), true) => (i + 1).min(ranges.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
            (None, true) => ranges
                .iter()
                .position(|c| c.lines.start >= self.scroll)
                .unwrap_or(ranges.len() - 1),
            (None, false) => ranges
                .iter()
                .rposition(|c| c.lines.start < self.scroll + self.height)
                .unwrap_or(0),
        };
        let range = ranges[next].lines.clone();
        self.focus = Some(ranges[next].id);
        // Show the whole block when it fits, otherwise its top.
        if range.start < self.scroll || range.end > self.scroll + self.height {
            self.scroll = if range.len() <= self.height {
                range.start.saturating_sub(2)
            } else {
                range.start
            }
            .min(self.max_scroll());
        }
    }

    /// How to run block `id`, or why it can't run.
    fn spec(&self, id: usize) -> Result<RunSpec, String> {
        let c = self.code(id).ok_or("no such block")?;
        if c.attrs.norun {
            return Err("block is marked norun".into());
        }
        let lang = c.lang.as_deref().ok_or("block has no language")?;
        let (program, args) =
            exec::interpreter(lang).ok_or_else(|| format!("no interpreter for `{lang}`"))?;
        let mut args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        args.push(c.src.clone());
        let cwd = match &c.attrs.cwd {
            Some(dir) => self.base_dir.join(dir),
            None => self.base_dir.clone(),
        };
        Ok(RunSpec {
            program: program.to_string(),
            args,
            cwd,
            env: c.attrs.env.clone(),
        })
    }

    /// Ask to run `ids` in order, confirming first unless that's switched off.
    fn request_run(&mut self, ids: Vec<usize>) {
        if ids.is_empty() {
            return;
        }
        if let Some(&id) = ids.iter().find(|&&id| self.runner.is_running(id)) {
            self.message = Some(format!("{} is already running", self.label(id)));
            return;
        }
        let must_ask = ids
            .iter()
            .any(|&id| self.code(id).is_some_and(|c| c.attrs.confirm));
        if self.assume_yes && !must_ask {
            self.start_queue(ids);
        } else {
            self.confirm = Some(ids);
        }
    }

    fn start_queue(&mut self, ids: Vec<usize>) {
        self.follow = ids.first().copied();
        self.queue = ids.into();
        self.start_next();
    }

    fn start_next(&mut self) {
        let Some(id) = self.queue.pop_front() else {
            return;
        };
        if self.follow.is_some() {
            self.follow = Some(id);
        }
        let mut run = BlockRun::started();
        match self.spec(id).and_then(|spec| {
            self.runner
                .start(id, &spec)
                .map_err(|e| format!("{}: {e}", spec.program))
        }) {
            Ok(()) => {}
            Err(e) => {
                run.status = Status::Error(e);
                self.queue.clear();
            }
        }
        self.runs.insert(id, run);
        self.dirty = true;
    }

    /// Display name of a block: its `name` attribute, or its position.
    pub fn label(&self, id: usize) -> String {
        match self.code(id).and_then(|c| c.attrs.name.clone()) {
            Some(name) => format!("`{name}`"),
            None => format!("block {}", id + 1),
        }
    }

    pub fn is_running(&self) -> bool {
        self.runs.values().any(BlockRun::is_running)
    }

    /// Apply process events and timeouts. Returns true when anything changed.
    pub fn tick(&mut self) -> bool {
        let events = self.runner.drain();
        let changed = !events.is_empty() || self.is_running();
        for event in events {
            match event {
                RunEvent::Output { id, text, stderr } => {
                    if let Some(run) = self.runs.get_mut(&id) {
                        run.push(text, stderr);
                    }
                }
                RunEvent::Exit {
                    id,
                    code,
                    cancelled,
                    took,
                } => {
                    let status = if self.timed_out.remove(&id) {
                        Status::TimedOut(took)
                    } else if cancelled {
                        Status::Cancelled(took)
                    } else if code == Some(0) {
                        Status::Ok(took)
                    } else {
                        Status::Failed { code, took }
                    };
                    let ok = status == Status::Ok(took);
                    if let Some(run) = self.runs.get_mut(&id) {
                        run.status = status;
                    }
                    if ok {
                        self.start_next();
                    } else {
                        self.queue.clear();
                    }
                }
            }
        }

        let expired: Vec<usize> = self
            .runs
            .iter()
            .filter_map(|(&id, run)| {
                let Status::Running(since) = run.status else {
                    return None;
                };
                let limit = self.code(id)?.attrs.timeout?;
                (since.elapsed() > limit && !self.timed_out.contains(&id)).then_some(id)
            })
            .collect();
        for id in expired {
            self.timed_out.insert(id);
            self.runner.cancel(id);
        }

        self.dirty |= changed;
        changed
    }

    fn on_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                if let Some(ids) = self.confirm.take() {
                    self.start_queue(ids);
                }
            }
            KeyCode::Char('n' | 'N' | 'q') | KeyCode::Esc => self.confirm = None,
            _ => {}
        }
    }

    fn run_focused(&mut self, all_from_here: bool) {
        if self.focus.is_none() {
            self.move_focus(true);
        }
        let Some(focus) = self.focus else { return };
        if !all_from_here {
            match self.spec(focus) {
                Ok(_) => self.request_run(vec![focus]),
                Err(e) => self.message = Some(e),
            }
            return;
        }
        let ids: Vec<usize> = self
            .rendered
            .code_blocks
            .iter()
            .map(|c| c.id)
            .skip_while(|&id| id != focus)
            .filter(|&id| self.spec(id).is_ok())
            .collect();
        if ids.is_empty() {
            self.message = Some("no runnable blocks from here".into());
        } else {
            self.request_run(ids);
        }
    }

    fn cancel(&mut self) {
        self.queue.clear();
        let ids: Vec<usize> = match self.focus {
            Some(id) if self.runner.is_running(id) => vec![id],
            _ => self
                .runs
                .iter()
                .filter(|(_, r)| r.is_running())
                .map(|(&id, _)| id)
                .collect(),
        };
        for id in ids {
            self.runner.cancel(id);
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        self.message = None;
        if self.confirm.is_some() {
            self.on_confirm_key(key);
            return Action::None;
        }
        if let Some(prompt) = &mut self.prompt {
            match prompt.on_key(key) {
                PromptEvent::Changed => {
                    let q = prompt.text.clone();
                    self.set_query(&q);
                }
                PromptEvent::Submit => self.prompt = None,
                PromptEvent::Cancel => {
                    self.prompt = None;
                    self.search = None;
                }
                PromptEvent::None => {}
            }
            return Action::None;
        }

        let page = self.height.max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Esc if self.search.is_some() => self.search = None,
            KeyCode::Esc if self.focus.is_some() => self.focus = None,
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => return Action::Back,
            KeyCode::Char('d') if ctrl => self.scroll_by(page / 2),
            KeyCode::Char('u') if ctrl => self.scroll_by(-page / 2),
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::Char(' ' | 'f') | KeyCode::PageDown => self.scroll_by(page),
            KeyCode::Char('b') | KeyCode::PageUp => self.scroll_by(-page),
            KeyCode::Char('g') | KeyCode::Home => {
                self.follow = None;
                self.scroll = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.follow = None;
                self.scroll = self.max_scroll();
            }
            KeyCode::Char('/') => self.prompt = Some(Prompt::new("/", "")),
            KeyCode::Char('n') => self.step_match(true),
            KeyCode::Char('N') => self.step_match(false),
            KeyCode::Tab => self.move_focus(true),
            KeyCode::BackTab => self.move_focus(false),
            KeyCode::Enter | KeyCode::Char('r') => self.run_focused(false),
            KeyCode::Char('R') => self.run_focused(true),
            KeyCode::Char('x') => self.cancel(),
            KeyCode::Char('o') => {
                if let Some(run) = self.focus.and_then(|id| self.runs.get_mut(&id)) {
                    run.expanded = !run.expanded;
                    self.dirty = true;
                }
            }
            _ => {}
        }
        Action::None
    }
}

fn collect_code(blocks: &[Block], out: &mut Vec<CodeBlock>) {
    for b in blocks {
        match b {
            Block::Code(c) => out.push(c.clone()),
            Block::Quote(inner) => collect_code(inner, out),
            Block::List { items, .. } => items.iter().for_each(|i| collect_code(i, out)),
            _ => {}
        }
    }
}

/// Find `query` in the rendered lines. Smart case: a lowercase query matches
/// case-insensitively. Matches don't cross wrapped line boundaries.
pub fn find(rendered: &Rendered, query: &str) -> Vec<Match> {
    let insensitive = !query.chars().any(char::is_uppercase);
    let fold = |c: char| {
        if insensitive {
            c.to_lowercase().next().unwrap_or(c)
        } else {
            c
        }
    };
    let needle: Vec<char> = query.chars().map(fold).collect();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (i, line) in rendered.lines.iter().enumerate() {
        let hay: Vec<char> = line
            .spans
            .iter()
            .flat_map(|s| s.content.chars())
            .map(fold)
            .collect();
        let mut start = 0;
        while start + needle.len() <= hay.len() {
            if hay[start..start + needle.len()] == needle[..] {
                out.push(Match {
                    line: i,
                    start,
                    end: start + needle.len(),
                });
                start += needle.len();
            } else {
                start += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;

    fn rendered(src: &str) -> Rendered {
        render::render(&parse(src), 40, &Theme::dark())
    }

    #[test]
    fn smart_case() {
        let r = rendered("Foo foo FOO\n");
        assert_eq!(find(&r, "foo").len(), 3);
        assert_eq!(find(&r, "Foo").len(), 1);
    }

    #[test]
    fn match_positions_are_chars() {
        let r = rendered("héllo wörld\n");
        assert_eq!(
            find(&r, "wör"),
            [Match {
                line: 0,
                start: 6,
                end: 9
            }]
        );
    }

    #[test]
    fn search_jumps_and_cycles() {
        let src: String = (0..100).map(|i| format!("line {i}\n\n")).collect();
        let mut r = Reader::new("t".into(), parse(&src), Theme::dark(), 80);
        r.layout(80, 10);
        r.set_query("line 50");
        assert!(
            r.scroll <= 100 && r.scroll + 10 > 100,
            "scroll {}",
            r.scroll
        );
        r.set_query("line 1");
        let n = r.search.as_ref().unwrap().matches.len();
        // "line 1" plus "line 10".."line 19"
        assert_eq!(n, 11);
        r.step_match(false);
        assert_eq!(r.search.as_ref().unwrap().current, n - 1);
    }
}
