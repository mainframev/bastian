mod details;
mod follow;
mod goto;
mod history;
mod keys;
mod location;
mod reload;
mod run;
mod scroll;
mod search;
mod toc;

pub use history::{HistoryPicker, canonical};
pub use location::Location;
pub use search::{Match, Search};

use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::doc::{CodeBlock, Document};
use crate::exec::{BlockRun, Interpreters, Runner};
use crate::images::{Images, Source as ImageSource};
use crate::links::Link;
use crate::nav;
use crate::prompt::Prompt;
use crate::render::theme::Theme;
use crate::render::{self, Highlights, Rendered};
use run::collect_code;
use search::find;

pub enum Action {
    None,
    /// Go back to the previous document or the browser, or quit.
    Back,
    Quit,
    /// Open a Markdown document in bastian, at the heading `anchor` names
    /// when there is one.
    OpenDocument {
        path: PathBuf,
        anchor: Option<String>,
    },
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
    /// List the documents opened this session.
    ShowHistory,
}

/// The link list opened with `L`.
pub struct LinkPicker {
    pub links: Vec<Link>,
    /// Index into `shown()`.
    pub selected: usize,
    /// Only links whose text or URL contains this, ignoring case.
    pub filter: String,
    /// The filter being typed, after `/`.
    pub prompt: Option<Prompt>,
    /// Rows the popup had room for when last drawn, a page for `PgDn` and
    /// the like; 0 before it's drawn.
    pub rows: Cell<usize>,
}

impl LinkPicker {
    pub fn new(links: Vec<Link>) -> Self {
        Self {
            links,
            selected: 0,
            filter: String::new(),
            prompt: None,
            rows: Cell::new(0),
        }
    }

    /// Indexes of the links that match the filter, in document order.
    pub fn shown(&self) -> Vec<usize> {
        let filter = self.filter.to_lowercase();
        (0..self.links.len())
            .filter(|&i| {
                let link = &self.links[i];
                link.text.to_lowercase().contains(&filter)
                    || link.url.to_lowercase().contains(&filter)
            })
            .collect()
    }

    /// The selected link, if any matches the filter.
    pub fn current(&self) -> Option<&Link> {
        self.shown().get(self.selected).map(|&i| &self.links[i])
    }

    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.selected = 0;
    }
}

/// What has the keyboard instead of the document. At most one is open;
/// opening another replaces it, draft and all.
pub enum Overlay {
    /// The key help, opened with `?`.
    Help(nav::Help),
    /// The link list opened with `L`.
    Links(LinkPicker),
    /// Where the document came from, opened with `p`.
    Location(Location),
    /// Blocks waiting for a yes/no before they run.
    RunConfirm(Vec<usize>),
    /// Link target waiting for a yes/no before the system opener gets it.
    OpenConfirm(String),
    /// The search prompt opened with `/`.
    Prompt(Prompt),
    /// The `Line:` prompt opened with `:`.
    GoTo(Prompt),
    /// The documents opened this session, listed with `H`.
    History(HistoryPicker),
}

/// The document a reader shows and where it came from. The origin decides
/// how links and images resolve, whether the file is watched and can be
/// edited, whether runs ask first, and whether the reading position is
/// remembered.
pub struct Source {
    /// Shown in the status bar.
    pub name: String,
    pub doc: Document,
    /// The source text, for `c`; empty when unknown.
    pub text: String,
    /// Directory block `cwd`s and relative links resolve from.
    pub base_dir: PathBuf,
    /// The file it was read from: watched for changes, editable, and its
    /// reading position remembered.
    pub file: Option<PathBuf>,
    /// Fetched over HTTP: links and images resolve against this URL, and
    /// every run asks first, whatever `Options::assume_yes` says.
    pub remote_base: Option<url::Url>,
}

impl Source {
    /// A document from nowhere in particular: stdin, the theme preview,
    /// tests. Relative links resolve from the current directory.
    pub fn in_memory(name: impl Into<String>, doc: Document) -> Self {
        Self {
            name: name.into(),
            doc,
            text: String::new(),
            base_dir: PathBuf::from("."),
            file: None,
            remote_base: None,
        }
    }

    /// A document read from a file.
    pub fn file(name: String, opened: crate::open::Opened) -> Self {
        Self {
            name,
            doc: opened.doc,
            text: opened.text,
            base_dir: opened.base_dir,
            file: Some(opened.path),
            remote_base: None,
        }
    }

    /// A document fetched over HTTP. Its code blocks run in the current
    /// directory and it has no file to watch or edit.
    pub fn remote(doc: crate::remote::RemoteDoc) -> Self {
        let parsed = doc.parse();
        Self {
            name: doc.name,
            doc: parsed,
            text: doc.text,
            base_dir: PathBuf::from("."),
            file: None,
            remote_base: Some(doc.base),
        }
    }
}

/// Settings every reader shares. `app::Settings` holds one; the theme
/// picker and `i` change it, and `set_theme`/`set_images` push those
/// changes into the readers already open.
#[derive(Clone)]
pub struct Options {
    pub theme: Theme,
    pub max_width: u16,
    /// Run blocks without asking, except those marked `confirm`.
    pub assume_yes: bool,
    pub interpreters: Interpreters,
    /// Draw a gutter of line numbers. Code files always do.
    pub line_numbers: bool,
    /// Image size and tab width settings.
    pub render: render::Options,
    /// Make links clickable terminal hyperlinks.
    pub hyperlinks: bool,
    /// Show a scrollbar when the document overflows.
    pub scrollbar: bool,
    /// How pictures are drawn; `None` leaves images as their alt text.
    pub images: Option<crate::images::Support>,
    /// Where reading positions are remembered; `None` doesn't.
    pub positions: Option<PathBuf>,
    /// Columns the table of contents opens with. The app keeps it at the
    /// last width set with `<` and `>`, for the rest of the session.
    pub toc_width: u16,
    /// The clock reloads are throttled by. Tests pass a fake one so they
    /// don't sleep through the throttle.
    pub now: fn() -> Instant,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            theme: Theme::dark(),
            max_width: 80,
            assume_yes: false,
            interpreters: Interpreters::default(),
            line_numbers: false,
            render: render::Options::default(),
            hyperlinks: true,
            scrollbar: true,
            images: None,
            positions: None,
            toc_width: crate::toc::DEFAULT_WIDTH,
            now: Instant::now,
        }
    }
}

/// State of the document reader view.
pub struct Reader {
    pub name: String,
    doc: Document,
    theme: Theme,
    pub max_width: u16,
    rendered: Rendered,
    scroll: usize,
    pub search: Option<Search>,
    /// The open Overlay, if any.
    pub overlay: Option<Overlay>,
    /// Focused code block id.
    pub focus: Option<usize>,
    /// Focused link, as an index into `link_stops()`.
    pub link_focus: Option<usize>,
    /// Where the focused link leads, for the status bar.
    pub link_preview: String,
    runs: HashMap<usize, BlockRun>,
    /// One-shot status bar message, cleared on the next key.
    message: Option<String>,
    /// Directory that block `cwd`s are relative to.
    base_dir: PathBuf,
    /// Skip confirmation, except for blocks marked `confirm`.
    assume_yes: bool,
    /// The table of contents is shown beside the document (`t`).
    pub toc: bool,
    /// Columns asked for the table of contents; `<` and `>` change it.
    toc_width: u16,
    /// The table of contents entry the last digit or `J`/`K` jump aimed
    /// at, and the scroll it left: near the end, the view stops short of
    /// putting the heading at the top, so the scroll alone can't say where
    /// it went. An entry, not a row, so a new layout finds it again;
    /// cleared when the view moves or the document is replaced.
    toc_jump: Option<(usize, usize)>,
    /// Rendered rows of the line `:` went to, highlighted until the next
    /// key, a move of the view or a new layout.
    goto_mark: Option<std::ops::Range<usize>>,
    /// Width of the screen at the last draw, which the table of contents
    /// is sized against.
    screen: u16,
    /// Draw a gutter of rendered line numbers.
    pub line_numbers: bool,
    /// File the document came from, watched for changes.
    source: Option<PathBuf>,
    /// The document's source text, for copying it whole.
    text: String,
    /// Set for documents fetched over HTTP: links resolve against this URL.
    remote_base: Option<url::Url>,
    modified: Option<SystemTime>,
    last_check: Instant,
    now: fn() -> Instant,
    interpreters: Interpreters,
    codes: Vec<CodeBlock>,
    runner: Runner,
    /// Remaining blocks of a run-all, started one after another.
    queue: VecDeque<usize>,
    timed_out: HashSet<usize>,
    /// Runner ids of processes stopped by a reload that changed or moved
    /// their block: their remaining events belong to no block.
    detached: HashSet<usize>,
    /// Block whose output the view keeps in sight until the user scrolls.
    follow: Option<usize>,
    dirty: bool,
    /// Pictures for the document's image blocks.
    pub(crate) images: Images,
    /// Whether this document's images were asked for yet. Done on the first
    /// layout, once `base_dir` and `remote_base` are final.
    images_requested: bool,
    /// Syntax highlighting per code block, reused across re-renders.
    highlights: Highlights,
    /// Details blocks opened or closed from how they start.
    toggled_details: HashSet<usize>,
    /// Id of the focused details block.
    pub details_focus: Option<usize>,
    /// Diagram blocks switched to show their source.
    as_source: HashSet<usize>,
    /// Image size and tab width settings.
    render_opts: render::Options,
    /// Make links clickable terminal hyperlinks.
    pub hyperlinks: bool,
    /// Show a scrollbar when the document overflows.
    pub scrollbar: bool,
    /// Where each of `rendered.links` leads as a terminal hyperlink, or
    /// `None` for links that must not open without asking. Worked out at
    /// layout, since it needs a look at the filesystem.
    hyperlink_targets: Vec<Option<String>>,
    /// Where reading positions are remembered; `None` doesn't remember
    /// this document's.
    positions: Option<PathBuf>,
    /// Source line to scroll to, kept until the user moves so it survives
    /// re-renders (e.g. pictures loading).
    restore: Option<crate::positions::Position>,
    /// Where the last restore left the view; any other scroll position
    /// means the user moved, which ends the restore.
    restored_at: Option<usize>,
    /// Heading anchor to scroll to once the document is laid out.
    pending_anchor: Option<String>,
    width: u16,
    height: usize,
}

impl Reader {
    pub fn new(source: Source, opts: &Options) -> Self {
        let Source {
            name,
            doc,
            text,
            base_dir,
            file,
            remote_base,
        } = source;
        let codes = collect_code(&doc.blocks);
        let code_file = doc.code_file;
        // Only files are remembered, and never where a source file was.
        let positions = opts.positions.as_ref().filter(|_| !code_file);
        let restore = match (&file, positions) {
            (Some(path), Some(store)) => crate::positions::load(store, path),
            _ => None,
        };
        Self {
            name,
            doc,
            theme: opts.theme.clone(),
            max_width: opts.max_width,
            rendered: Rendered::default(),
            scroll: 0,
            search: None,
            overlay: None,
            focus: None,
            link_focus: None,
            link_preview: String::new(),
            runs: HashMap::new(),
            message: None,
            base_dir,
            // Untrusted source: every run asks first, even with `--yes`.
            assume_yes: opts.assume_yes && remote_base.is_none(),
            toc: false,
            toc_width: opts.toc_width,
            toc_jump: None,
            goto_mark: None,
            screen: 0,
            // Source files read from disk always show their line numbers.
            line_numbers: opts.line_numbers || (file.is_some() && code_file),
            modified: file.as_deref().and_then(reload::mtime),
            source: file,
            text,
            remote_base,
            last_check: (opts.now)(),
            now: opts.now,
            interpreters: opts.interpreters.clone(),
            codes,
            runner: Runner::default(),
            queue: VecDeque::new(),
            timed_out: HashSet::new(),
            detached: HashSet::new(),
            follow: None,
            dirty: false,
            images: Images::new(opts.images.clone()),
            images_requested: false,
            highlights: Highlights::default(),
            toggled_details: HashSet::new(),
            as_source: HashSet::new(),
            details_focus: None,
            render_opts: opts.render,
            hyperlinks: opts.hyperlinks,
            scrollbar: opts.scrollbar,
            hyperlink_targets: Vec::new(),
            positions: positions.cloned(),
            restore,
            restored_at: None,
            pending_anchor: None,
            width: 0,
            height: 0,
        }
    }

    /// Start loading every image block's picture.
    fn request_images(&mut self) {
        if !self.images.enabled() {
            return;
        }
        for url in self.visible_images() {
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
            return web(crate::remote::resolve_link(base, url)?);
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

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The search prompt, while it's being typed into.
    pub fn prompt(&self) -> Option<&Prompt> {
        match &self.overlay {
            Some(Overlay::Prompt(p)) => Some(p),
            _ => None,
        }
    }

    /// The `Line:` prompt, while it's being typed into.
    pub fn goto(&self) -> Option<&Prompt> {
        match &self.overlay {
            Some(Overlay::GoTo(p)) => Some(p),
            _ => None,
        }
    }

    /// The document as laid out by the last `layout`.
    pub fn rendered(&self) -> &Rendered {
        &self.rendered
    }

    /// The rendered line at the top of the view.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Put rendered line `line` at the top of the view, as far as the
    /// document allows. Counts as the user moving: it ends a restore.
    pub fn scroll_to(&mut self, line: usize) {
        self.jump_to(line);
    }

    /// The one-shot status bar message, if any.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Show `msg` in the status bar until the next key.
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = Some(msg.into());
    }

    /// Run state by code block id.
    pub fn runs(&self) -> &HashMap<usize, BlockRun> {
        &self.runs
    }

    /// Directory block `cwd`s and relative links resolve from.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Where each of `rendered().links` leads as a terminal hyperlink;
    /// `None` for links that only open behind a prompt. Empty when
    /// hyperlinks are off.
    pub fn hyperlink_targets(&self) -> &[Option<String>] {
        &self.hyperlink_targets
    }

    /// Re-render when the width or run output changed, and keep scroll in range.
    pub fn layout(&mut self, width: u16, height: u16) {
        if !self.images_requested {
            self.images_requested = true;
            self.request_images();
        }
        // A new width (a resized terminal or the table of contents opening,
        // closing or resizing) rewraps the text: keep the same part of the
        // source at the top rather than the same rendered row.
        let keep = if width != self.width && self.width != 0 {
            self.position()
        } else {
            None
        };
        if width != self.width || self.dirty {
            let images = self.images.sizes();
            let cx = render::Context {
                runs: &self.runs,
                interpreters: &self.interpreters,
                highlights: &self.highlights,
                images: &images,
                toggled_details: Some(&self.toggled_details),
                as_source: &self.as_source,
                opts: self.render_opts,
            };
            self.rendered = render::render_with_runs(&self.doc, width, &self.theme, cx);
            // Its rows may have moved.
            self.goto_mark = None;
            self.width = width;
            self.dirty = false;
            self.refind();
            if self.hyperlinks {
                self.hyperlink_targets = self
                    .rendered
                    .links
                    .iter()
                    .map(|l| self.hyperlink_target(&l.url))
                    .collect();
            }
            let stops = self.link_stops().len();
            if self.link_focus.is_some_and(|i| i >= stops) {
                self.link_focus = None;
            }
            // A details block inside one that closed is no longer shown.
            if let Some(id) = self.details_focus
                && !self.rendered.details.iter().any(|d| d.id == id)
            {
                self.details_focus = None;
            }
        }
        self.height = usize::from(height);
        // A restore still in progress places the view itself.
        let restoring =
            self.restore.is_some() && self.restored_at.is_none_or(|at| at == self.scroll);
        if let Some(position) = keep
            && !restoring
        {
            self.scroll_to_position(position);
        }
        if let Some(position) = self.restore {
            if self.restored_at.is_none_or(|at| at == self.scroll) {
                self.scroll_to_position(position);
                // Near the end the view can't reach the block's top; note
                // where it really is, or the clamp below looks like a move.
                self.scroll = self.scroll.min(self.max_scroll());
                self.restored_at = Some(self.scroll);
            } else {
                self.restore = None;
            }
        }
        if let Some(anchor) = self.pending_anchor.take() {
            match self.heading_line(&anchor) {
                Some(line) => self.jump_to(line),
                None => self.message = Some(format!("no heading #{anchor}")),
            }
        }
        self.follow_output();
        self.scroll = self.scroll.min(self.max_scroll());
    }

    /// Open at the heading whose anchor is `anchor` (`#getting-started`
    /// without the `#`), instead of at a remembered position.
    pub fn go_to_anchor(&mut self, anchor: &str) {
        self.pending_anchor = Some(anchor.to_string());
        self.restore = None;
    }

    /// Search the rendered text again, keeping the current match in range.
    fn refind(&mut self) {
        if let Some(s) = &mut self.search {
            s.matches = find(&self.rendered, &s.query);
            s.current = s.current.min(s.matches.len().saturating_sub(1));
        }
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

    /// Upkeep for a reader that isn't shown (on the history stack): kill
    /// cancelled blocks that ignore SIGTERM. Output stays queued for `tick`.
    pub fn tick_hidden(&mut self) {
        self.runner.escalate();
    }

    /// Apply process events and timeouts. Returns true when anything changed.
    pub fn tick(&mut self) -> bool {
        let reloaded = self.check_reload();
        let events = self.runner.drain();
        let images = self.images.poll();
        let rerender = reloaded || images || !events.is_empty();
        let changed = rerender || self.is_running();
        for event in events {
            self.on_run_event(event);
        }
        self.cancel_expired();
        self.runner.escalate();

        // Without new events, only running blocks' timers move: patch
        // those lines instead of laying out the whole document again, and
        // search the patched text so matches stay in step with it.
        if rerender || self.dirty {
            self.dirty |= changed;
        } else if changed {
            self.refresh_timers();
            self.refind();
        }
        changed
    }
}
