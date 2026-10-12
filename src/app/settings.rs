use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::browser;
use crate::config::{self, Confirm};
use crate::doc::{self, Document};
use crate::exec::Interpreters;
use crate::paths;
use crate::reader::Reader;
use crate::render::{self, theme::Theme};
use crate::term::{self, Background};
use crate::{images, themes};

/// Options shared by every document the app opens.
#[derive(Clone)]
pub struct Settings {
    pub theme: Theme,
    pub max_width: u16,
    /// Run blocks without asking, except those marked `confirm`.
    pub assume_yes: bool,
    pub interpreters: Interpreters,
    /// Show line numbers in readers.
    pub line_numbers: bool,
    /// Which files the browser lists, and in what order.
    pub scan: browser::Scan,
    /// strftime format of the browser's dates; `None` hides them.
    pub date_format: Option<String>,
    /// Image size and tab width settings.
    pub render: render::Options,
    /// Make links clickable terminal hyperlinks.
    pub hyperlinks: bool,
    /// Show scrollbars.
    pub scrollbar: bool,
    /// Editor command line from the config; `$VISUAL`/`$EDITOR` without.
    pub editor: Option<String>,
    /// Scroll with the mouse wheel (captures the mouse, so terminal text
    /// selection needs a modifier key).
    pub mouse: bool,
    /// Show the logo at the top of the file browser.
    pub logo: bool,
    /// How the terminal draws pictures, once asked; `None` until then.
    pub images: Option<images::Support>,
    /// Draw pictures for images; off leaves them as their alt text.
    pub show_images: bool,
    /// How the file browser marks the selected file.
    pub selector: config::Selector,
    /// How the file list lays out each file.
    pub list_layout: config::ListLayout,
}

impl Settings {
    /// Turn resolved configuration into what the app runs with. `colored` says
    /// whether the theme's colors will show; if not, `auto` doesn't ask the
    /// terminal for its background.
    pub fn from_resolved(
        r: config::Resolved,
        themes_dir: Option<&Path>,
        colored: bool,
        tui: bool,
    ) -> Result<(Settings, String)> {
        let light =
            colored && r.theme.value == "auto" && term::background() == Some(Background::Light);
        let name = config::theme_name(&r, light).to_string();
        let scan = browser::Scan {
            all: r.all.value,
            sort: r.sort.value,
            extensions: r.extensions.value,
            ignore: r.ignore.value,
        };
        scan.overrides(Path::new("."))?;
        let date_format = r.dates.value.then_some(r.date_format.value);
        if let Some(f) = &date_format {
            config::check_date_format(f)?;
        }
        let settings = Settings {
            theme: themes::load(&name, themes_dir)?,
            max_width: r.width.value,
            assume_yes: r.confirm.value == Confirm::Never,
            line_numbers: r.line_numbers.value,
            scan,
            date_format,
            render: render::Options {
                image_rows: r.image_height.value,
                tab_width: r.tab_width.value,
            },
            hyperlinks: r.hyperlinks.value,
            scrollbar: r.scrollbar.value,
            editor: Some(r.editor.value).filter(|e| !e.trim().is_empty()),
            mouse: r.mouse.value,
            logo: r.logo.value,
            // Only the TUI draws pictures.
            images: (tui && r.images.value).then(images::Support::detect),
            show_images: r.images.value,
            selector: r.selector.value,
            list_layout: r.list_layout.value,
            interpreters: Interpreters::new(r.interpreters),
        };
        Ok((settings, name))
    }

    /// A reader for `doc`, parsed from `text`, with code that runs in
    /// `base_dir`.
    pub fn reader(&self, name: String, doc: Document, text: String, base_dir: PathBuf) -> Reader {
        let mut r = Reader::new(name, doc, self.theme.clone(), self.max_width);
        r.text = text;
        r.base_dir = base_dir;
        r.assume_yes = self.assume_yes;
        r.interpreters = self.interpreters.clone();
        r.line_numbers = self.line_numbers;
        r.render_opts = self.render;
        r.hyperlinks = self.hyperlinks;
        r.scrollbar = self.scrollbar;
        r.images = images::Images::new(self.image_support());
        r
    }

    /// The support readers draw pictures with: `None` while they're off.
    pub(super) fn image_support(&self) -> Option<images::Support> {
        self.images.clone().filter(|_| self.show_images)
    }

    /// A reader for a fetched document. Its code blocks run in the current
    /// directory and always ask first; it has no file to watch or edit.
    pub fn remote(&self, doc: crate::remote::RemoteDoc) -> Reader {
        let parsed = doc.parse();
        let mut r = self.reader(doc.name, parsed, doc.text, PathBuf::from("."));
        r.remote_base = Some(doc.base);
        // Untrusted source: every run asks first, even with `--yes`.
        r.assume_yes = false;
        r
    }

    /// Open a file in a reader that reloads when the file changes.
    pub fn open(&self, path: &Path, name: String) -> Result<Reader> {
        let src = crate::text::read_text(path)?;
        let doc = doc::parse_for(&src, Some(path));
        let code_file = doc.code_file;
        let mut r = self.reader(name, doc, src, paths::base_dir(path));
        // Source files always show their line numbers.
        r.line_numbers |= code_file;
        r.watch(path.to_path_buf());
        Ok(r)
    }
}
