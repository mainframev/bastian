use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::browser;
use crate::config::{self, Confirm};
use crate::doc::Document;
use crate::exec::Interpreters;
use crate::reader::{self, Reader, Source};
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
    /// Where reading positions are remembered; `None` doesn't.
    pub positions: Option<PathBuf>,
}

impl Settings {
    /// Turn resolved configuration into what the app runs with. `colored` says
    /// whether the theme's colors will show; if not, `auto` doesn't look at the
    /// background. `stdout_tty` says whether stdout is a terminal: behind a
    /// pipe the reader (`less -R`, say) shares the terminal, and the query's
    /// raw mode and reply would get mixed up with its own, so only
    /// `COLORFGBG` is consulted.
    pub fn from_resolved(
        r: config::Resolved,
        themes_dir: Option<&Path>,
        colored: bool,
        tui: bool,
        stdout_tty: bool,
    ) -> Result<(Settings, String)> {
        // The TUI only runs with a terminal on stdout.
        let query = tui || stdout_tty;
        let light = colored
            && r.theme.value == "auto"
            && term::background(query) == Some(Background::Light);
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
            positions: r
                .remember_position
                .value
                .then(crate::positions::default_store)
                .flatten(),
            interpreters: Interpreters::new(r.interpreters),
        };
        Ok((settings, name))
    }

    /// What every reader opened with these settings gets.
    pub fn options(&self) -> reader::Options {
        reader::Options {
            theme: self.theme.clone(),
            max_width: self.max_width,
            assume_yes: self.assume_yes,
            interpreters: self.interpreters.clone(),
            line_numbers: self.line_numbers,
            render: self.render,
            hyperlinks: self.hyperlinks,
            scrollbar: self.scrollbar,
            images: self.image_support(),
            positions: self.positions.clone(),
        }
    }

    /// A reader for `doc`, parsed from `text`, with code that runs in
    /// `base_dir`.
    pub fn reader(&self, name: String, doc: Document, text: String, base_dir: PathBuf) -> Reader {
        let source = Source {
            text,
            base_dir,
            ..Source::in_memory(name, doc)
        };
        Reader::new(source, &self.options())
    }

    /// The support readers draw pictures with: `None` while they're off.
    pub(super) fn image_support(&self) -> Option<images::Support> {
        self.images.clone().filter(|_| self.show_images)
    }

    /// A reader for a fetched document.
    pub fn remote(&self, doc: crate::remote::RemoteDoc) -> Reader {
        Reader::new(Source::remote(doc), &self.options())
    }

    /// Open a file in a reader that reloads when the file changes.
    pub fn open(&self, path: &Path, name: String) -> Result<Reader> {
        let opened = crate::open::open(path)?;
        Ok(Reader::new(Source::file(name, opened), &self.options()))
    }
}
