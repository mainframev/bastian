use std::path::Path;

use anyhow::Result;

use crate::browser;
use crate::config::{self, Confirm};
use crate::exec::Interpreters;
use crate::reader::{self, Reader, Source};
use crate::render;
use crate::term::{self, Background};
use crate::{images, themes};

/// Options shared by every document the app opens.
#[derive(Clone)]
pub struct Settings {
    /// What every reader gets: theme, width, run and link settings.
    pub reader: reader::Options,
    /// Which files the browser lists, and in what order.
    pub scan: browser::Scan,
    /// strftime format of the browser's dates; `None` hides them.
    pub date_format: Option<String>,
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
        // Only the TUI draws pictures.
        let images = (tui && r.images.value).then(images::Support::detect);
        let settings = Settings {
            reader: reader::Options {
                theme: themes::load(&name, themes_dir)?,
                max_width: r.width.value,
                assume_yes: r.confirm.value == Confirm::Never,
                interpreters: Interpreters::new(r.interpreters),
                line_numbers: r.line_numbers.value,
                render: render::Options {
                    image_rows: r.image_height.value,
                    tab_width: r.tab_width.value,
                    ..render::Options::default()
                },
                hyperlinks: r.hyperlinks.value,
                scrollbar: r.scrollbar.value,
                images: images.clone(),
                positions: r
                    .remember_position
                    .value
                    .then(crate::positions::default_store)
                    .flatten(),
                toc_width: crate::toc::DEFAULT_WIDTH,
            },
            scan,
            date_format,
            editor: Some(r.editor.value).filter(|e| !e.trim().is_empty()),
            mouse: r.mouse.value,
            logo: r.logo.value,
            images,
            show_images: r.images.value,
            selector: r.selector.value,
            list_layout: r.list_layout.value,
        };
        Ok((settings, name))
    }

    /// The support readers draw pictures with: `None` while they're off.
    pub(super) fn image_support(&self) -> Option<images::Support> {
        self.images.clone().filter(|_| self.show_images)
    }

    /// Open a file in a reader that reloads when the file changes.
    pub fn open(&self, path: &Path, name: String) -> Result<Reader> {
        let opened = crate::open::open(path)?;
        Ok(Reader::new(Source::file(name, opened), &self.reader))
    }
}
