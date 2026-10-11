use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::doc::{self, Block, CodeBlock, Document};
use crate::exec::{BlockRun, Interpreters, RunEvent, RunSpec, Runner, Status};
use crate::images::{Images, Source as ImageSource};
use crate::links::{self, Link, Target};
use crate::outline::{self, Outline};
use crate::prompt::{self, Prompt, PromptEvent};
use crate::render::theme::Theme;
use crate::render::{self, Highlights, Rendered};

pub enum Action {
    None,
    /// Go back to the previous document or the browser, or quit.
    Back,
    Quit,
    /// Open a Markdown document in bastian.
    OpenDocument(PathBuf),
    /// Fetch and open a remote Markdown document.
    OpenRemote(url::Url),
    /// Hand a URL or file to the system opener.
    OpenExternal(String),
    /// Put text on the clipboard.
    Copy(String),
    /// Open the document's file in the user's editor, at a 1-based line.
    Edit(PathBuf, Option<usize>),
    /// Open the theme picker.
    PickTheme,
    /// Switch pictures for images on or off.
    ToggleImages,
}

/// The link list opened with `L`.
pub struct LinkPicker {
    pub links: Vec<Link>,
    pub selected: usize,
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
    pub link_picker: Option<LinkPicker>,
    /// The heading outline opened with `O`.
    pub outline: Option<Outline>,
    pub help: bool,
    /// Draw a gutter of rendered line numbers.
    pub line_numbers: bool,
    /// File the document came from, watched for changes.
    pub source: Option<PathBuf>,
    /// The document's source text, for copying it whole.
    pub text: String,
    /// Set for documents fetched over HTTP: links resolve against this URL.
    pub remote_base: Option<url::Url>,
    modified: Option<SystemTime>,
    last_check: Instant,
    pub interpreters: Interpreters,
    codes: Vec<CodeBlock>,
    runner: Runner,
    /// Remaining blocks of a run-all, started one after another.
    queue: VecDeque<usize>,
    timed_out: HashSet<usize>,
    /// Block whose output the view keeps in sight until the user scrolls.
    follow: Option<usize>,
    dirty: bool,
    /// Pictures for the document's image blocks.
    pub images: Images,
    /// Whether this document's images were asked for yet. Done on the first
    /// layout, once `base_dir` and `remote_base` are final.
    images_requested: bool,
    /// Syntax highlighting per code block, reused across re-renders.
    highlights: Highlights,
    /// Image size and tab width settings.
    pub render_opts: render::Options,
    /// Make links clickable terminal hyperlinks.
    pub hyperlinks: bool,
    /// Show a scrollbar when the document overflows.
    pub scrollbar: bool,
    width: u16,
    height: usize,
}

impl Reader {
    pub fn new(name: String, doc: Document, theme: Theme, max_width: u16) -> Self {
        let codes = collect_code(&doc.blocks);
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
            link_picker: None,
            outline: None,
            help: false,
            line_numbers: false,
            source: None,
            text: String::new(),
            remote_base: None,
            modified: None,
            last_check: Instant::now(),
            interpreters: Interpreters::default(),
            codes,
            runner: Runner::default(),
            queue: VecDeque::new(),
            timed_out: HashSet::new(),
            follow: None,
            dirty: false,
            images: Images::default(),
            images_requested: false,
            highlights: Highlights::default(),
            render_opts: render::Options::default(),
            hyperlinks: true,
            scrollbar: true,
            width: 0,
            height: 0,
        }
    }

    /// Absolute target for a link written as `url`, for terminal hyperlinks:
    /// URLs as they are, relative links against the remote document's URL
    /// or as `file://` paths. In-document `#anchors` have none.
    pub fn hyperlink_target(&self, url: &str) -> Option<String> {
        if url.starts_with('#') {
            return None;
        }
        if let Ok(abs) = url::Url::parse(url) {
            return Some(abs.to_string());
        }
        if let Some(base) = &self.remote_base {
            return base.join(url).ok().map(|u| u.to_string());
        }
        let (path, fragment) = url
            .split_once('#')
            .map_or((url, None), |(p, f)| (p, Some(f)));
        let abs = std::path::absolute(self.base_dir.join(path)).ok()?;
        let mut file = url::Url::from_file_path(abs).ok()?;
        file.set_fragment(fragment);
        Some(file.to_string())
    }

    /// Start loading every image block's picture.
    fn request_images(&mut self) {
        if !self.images.enabled() {
            return;
        }
        let mut urls = Vec::new();
        doc::visit(&self.doc.blocks, &mut |b| {
            if let Block::Image { url, .. } = b {
                urls.push(url.clone());
            }
        });
        for url in urls {
            if let Some(source) = self.image_source(&url) {
                self.images.request(&url, source);
            }
        }
    }

    /// Where the picture for an image written as `url` comes from: a web
    /// URL (GitHub file pages as raw files), relative to a remote document,
    /// or a file next to this one.
    pub fn image_source(&self, url: &str) -> Option<ImageSource> {
        let web = |u: url::Url| {
            let raw = crate::remote::raw_url(&u).unwrap_or(u);
            matches!(raw.scheme(), "http" | "https").then(|| ImageSource::Url(raw.to_string()))
        };
        if let Ok(abs) = url::Url::parse(url) {
            return web(abs);
        }
        if let Some(base) = &self.remote_base {
            return web(base.join(url).ok()?);
        }
        let path = url.split(['?', '#']).next().unwrap_or(url);
        Some(ImageSource::File(self.base_dir.join(path)))
    }

    /// Draw pictures with `support`, or leave images as their alt text
    /// with `None`.
    pub fn set_images(&mut self, support: Option<crate::images::Support>) {
        self.images.set_support(support);
        self.images_requested = false;
        self.dirty = true;
    }

    /// Switch to `theme`, re-highlighting code on the next layout.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.highlights.clear();
        self.dirty = true;
    }

    /// Re-render when the width or run output changed, and keep scroll in range.
    pub fn layout(&mut self, width: u16, height: u16) {
        if !self.images_requested {
            self.images_requested = true;
            self.request_images();
        }
        if width != self.width || self.dirty {
            self.rendered = render::render_with_runs(
                &self.doc,
                width,
                &self.theme,
                &self.runs,
                &self.interpreters,
                &self.highlights,
                &self.images.sizes(),
                self.render_opts,
            );
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

    /// 1-based source line of the block at the top of the view, if the
    /// document maps rendered lines back to its source.
    pub fn source_line(&self) -> Option<usize> {
        self.rendered
            .anchors
            .iter()
            .rev()
            .find(|(rendered, _)| *rendered <= self.scroll)
            .or_else(|| self.rendered.anchors.first())
            .map(|(_, src)| src + 1)
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

    /// Scroll by `delta` lines (negative is up).
    pub fn scroll_by(&mut self, delta: isize) {
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

    /// Directory block `id` runs in: the document's, joined with the
    /// block's `cwd` attribute if it has one.
    pub fn cwd(&self, id: usize) -> PathBuf {
        match self.code(id).and_then(|c| c.attrs.cwd.as_ref()) {
            Some(dir) => self.base_dir.join(dir),
            None => self.base_dir.clone(),
        }
    }

    /// How to run block `id`, or why it can't run.
    fn spec(&self, id: usize) -> Result<RunSpec, String> {
        let c = self.code(id).ok_or("no such block")?;
        if c.attrs.norun {
            return Err("block is marked norun".into());
        }
        let lang = c.lang.as_deref().ok_or("block has no language")?;
        let mut cmd = self
            .interpreters
            .get(lang)
            .ok_or_else(|| format!("no interpreter for `{lang}`"))?;
        let program = cmd.remove(0);
        let mut args = cmd;
        args.push(c.src.clone());
        Ok(RunSpec {
            program,
            args,
            cwd: self.cwd(id),
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
    /// Watch `path` and reload the document when it changes on disk.
    pub fn watch(&mut self, path: PathBuf) {
        self.modified = mtime(&path);
        self.source = Some(path);
    }

    /// Re-read the source file if its modification time moved.
    fn check_reload(&mut self) -> bool {
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

    pub fn tick(&mut self) -> bool {
        let reloaded = self.check_reload();
        let events = self.runner.drain();
        // Without new events, only running blocks' timers move: patch
        // those lines instead of laying out the whole document again.
        // A search re-renders so its matches stay in step with the text.
        let images = self.images.poll();
        let rerender = reloaded || images || !events.is_empty() || self.search.is_some();
        let changed = reloaded || images || !events.is_empty() || self.is_running();
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

        if rerender || self.dirty {
            self.dirty |= changed;
        } else if changed {
            self.refresh_timers();
        }
        changed
    }

    /// Redraw the elapsed time on running blocks' status lines in place.
    /// Only valid while `rendered` is current (not `dirty`).
    fn refresh_timers(&mut self) {
        for range in &self.rendered.code_blocks {
            let Some(run) = self.runs.get(&range.id).filter(|r| r.is_running()) else {
                continue;
            };
            // The status line opens the run panel right under the block,
            // after any prefix spans from enclosing lists or quotes.
            if range.output_end <= range.lines.end {
                continue;
            }
            let status = render::run_status(run, &self.theme);
            let line = &mut self.rendered.lines[range.lines.end];
            line.spans
                .truncate(line.spans.len().saturating_sub(status.spans.len()));
            line.spans.extend(status.spans);
        }
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

    fn on_link_key(&mut self, key: KeyEvent) -> Action {
        let Some(picker) = &mut self.link_picker else {
            return Action::None;
        };
        let last = picker.links.len().saturating_sub(1);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                picker.selected = (picker.selected + 1).min(last);
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                picker.selected = picker.selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                let url = picker.links[picker.selected].url.clone();
                self.link_picker = None;
                return self.follow_link(&url);
            }
            KeyCode::Esc | KeyCode::Char('q' | 'L') => self.link_picker = None,
            _ => {}
        }
        Action::None
    }

    fn follow_link(&mut self, url: &str) -> Action {
        if let Some(base) = &self.remote_base
            && !url.starts_with('#')
        {
            return match crate::remote::resolve_link(base, url) {
                Some(target) if crate::remote::is_document(&target) => Action::OpenRemote(target),
                Some(target) => Action::OpenExternal(target.to_string()),
                None => {
                    self.message = Some(format!("can't resolve link {url}"));
                    Action::None
                }
            };
        }
        match links::resolve(url, &self.base_dir) {
            Target::Document(path) => Action::OpenDocument(path),
            Target::External(target) => Action::OpenExternal(target),
            Target::Anchor(anchor) => {
                match self.heading_line(&anchor) {
                    Some(line) => {
                        self.follow = None;
                        self.scroll = line.min(self.max_scroll());
                    }
                    None => self.message = Some(format!("no heading #{anchor}")),
                }
                Action::None
            }
            Target::Missing(path) => {
                self.message = Some(format!("not found: {}", path.display()));
                Action::None
            }
        }
    }

    /// Rendered line of the heading whose GitHub-style slug is `anchor`.
    fn heading_line(&self, anchor: &str) -> Option<usize> {
        let text = heading_texts(&self.doc.blocks)
            .into_iter()
            .find(|h| links::slug(h) == anchor)?;
        self.rendered.lines.iter().position(|line| {
            let plain = render::plain(line);
            let plain = plain.trim().trim_start_matches('#').trim();
            !plain.is_empty() && text.starts_with(plain)
        })
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        self.message = None;
        if self.help {
            self.help = false;
            return Action::None;
        }
        if self.link_picker.is_some() {
            return self.on_link_key(key);
        }
        if let Some(outline) = &mut self.outline {
            match outline.on_key(key) {
                outline::Event::None => {}
                outline::Event::Close => self.outline = None,
                outline::Event::Jump(line) => {
                    self.outline = None;
                    self.scroll = line.min(self.max_scroll());
                }
            }
            return Action::None;
        }
        if self.confirm.is_some() {
            self.on_confirm_key(key);
            return Action::None;
        }
        if let Some(prompt) = &mut self.prompt {
            match prompt.on_key(key) {
                PromptEvent::Changed => {
                    let q = prompt.text().to_string();
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
        let ctrl = prompt::ctrl(&key);
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
            KeyCode::Char('/') => self.prompt = Some(Prompt::new("")),
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
            KeyCode::Char('y') => match self.focus.and_then(|id| self.code(id)) {
                Some(c) => {
                    let src = c.src.trim_end_matches('\n').to_string();
                    self.message = Some(format!("copied {}", self.label(c.id)));
                    return Action::Copy(src);
                }
                None => self.message = Some("focus a block with Tab first".into()),
            },
            KeyCode::Char('L') => {
                let links = links::collect(&self.doc);
                if links.is_empty() {
                    self.message = Some("no links in this document".into());
                } else {
                    self.link_picker = Some(LinkPicker { links, selected: 0 });
                }
            }
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('T') => return Action::PickTheme,
            KeyCode::Char('i') => return Action::ToggleImages,
            KeyCode::Char('O') => {
                self.outline = Outline::new(&self.doc, &self.rendered, self.scroll);
                if self.outline.is_none() {
                    self.message = Some("no headings in this document".into());
                }
            }
            KeyCode::Char('#') => self.line_numbers = !self.line_numbers,
            KeyCode::Char('e') => match &self.source {
                Some(path) => return Action::Edit(path.clone(), self.source_line()),
                None => self.message = Some("no local file to edit".into()),
            },
            KeyCode::Char('c') if !ctrl => {
                if self.text.is_empty() {
                    self.message = Some("nothing to copy".into());
                } else {
                    let lines = self.text.lines().count();
                    self.message = Some(format!("copied document ({lines} lines)"));
                    return Action::Copy(self.text.clone());
                }
            }
            _ => {}
        }
        Action::None
    }
}

/// How often to check the source file for changes.
const RELOAD_CHECK: Duration = Duration::from_millis(500);

fn mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn heading_texts(blocks: &[Block]) -> Vec<String> {
    let mut out = Vec::new();
    doc::visit(blocks, &mut |b| {
        if let Block::Heading { inlines, .. } = b {
            out.push(doc::plain_text(inlines));
        }
    });
    out
}

fn collect_code(blocks: &[Block]) -> Vec<CodeBlock> {
    let mut out = Vec::new();
    doc::visit(blocks, &mut |b| {
        if let Block::Code(c) = b {
            out.push(c.clone());
        }
    });
    out
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
