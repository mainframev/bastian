mod follow;
mod keys;
mod reload;
mod run;
mod scroll;
mod search;

pub use search::{Match, Search};

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use crate::doc::{self, Block, CodeBlock, Document};
use crate::exec::{BlockRun, Interpreters, Runner};
use crate::images::{Images, Source as ImageSource};
use crate::links::Link;
use crate::outline::Outline;
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
    /// Runner ids of processes stopped by a reload that changed or moved
    /// their block: their remaining events belong to no block.
    detached: HashSet<usize>,
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
    /// Where reading positions are remembered; `None` doesn't remember
    /// this document's.
    pub positions: Option<PathBuf>,
    /// Source line to scroll to, kept until the user moves so it survives
    /// re-renders (e.g. pictures loading).
    pub restore: Option<crate::positions::Position>,
    /// Where the last restore left the view; any other scroll position
    /// means the user moved, which ends the restore.
    restored_at: Option<usize>,
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
            detached: HashSet::new(),
            follow: None,
            dirty: false,
            images: Images::default(),
            images_requested: false,
            highlights: Highlights::default(),
            render_opts: render::Options::default(),
            hyperlinks: true,
            scrollbar: true,
            positions: None,
            restore: None,
            restored_at: None,
            width: 0,
            height: 0,
        }
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

    /// Re-render when the width or run output changed, and keep scroll in range.
    pub fn layout(&mut self, width: u16, height: u16) {
        if !self.images_requested {
            self.images_requested = true;
            self.request_images();
        }
        if width != self.width || self.dirty {
            let images = self.images.sizes();
            let cx = render::Context {
                runs: &self.runs,
                interpreters: &self.interpreters,
                highlights: &self.highlights,
                images: &images,
                opts: self.render_opts,
            };
            self.rendered = render::render_with_runs(&self.doc, width, &self.theme, cx);
            self.width = width;
            self.dirty = false;
            self.refind();
        }
        self.height = usize::from(height);
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
        self.follow_output();
        self.scroll = self.scroll.min(self.max_scroll());
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
