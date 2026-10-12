use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::browser::{self};
use crate::doc::{self, Document};
use crate::exec::Interpreters;
use crate::paths;
use crate::reader::Reader;
use crate::render::{self, theme::Theme};

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
    pub images: Option<crate::images::Support>,
    /// Draw pictures for images; off leaves them as their alt text.
    pub show_images: bool,
    /// How the file browser marks the selected file.
    pub selector: crate::config::Selector,
    /// How the file list lays out each file.
    pub list_layout: crate::config::ListLayout,
}

impl Settings {
    pub fn reader(&self, name: String, doc: Document, base_dir: PathBuf) -> Reader {
        let mut r = Reader::new(name, doc, self.theme.clone(), self.max_width);
        r.base_dir = base_dir;
        r.assume_yes = self.assume_yes;
        r.interpreters = self.interpreters.clone();
        r.line_numbers = self.line_numbers;
        r.render_opts = self.render;
        r.hyperlinks = self.hyperlinks;
        r.scrollbar = self.scrollbar;
        r.images = crate::images::Images::new(self.image_support());
        r
    }

    /// The support readers draw pictures with: `None` while they're off.
    pub(super) fn image_support(&self) -> Option<crate::images::Support> {
        self.images.clone().filter(|_| self.show_images)
    }

    /// A reader for a fetched document. Its code blocks run in the current
    /// directory and always ask first; it has no file to watch or edit.
    pub fn remote(&self, doc: crate::remote::RemoteDoc) -> Reader {
        let parsed = doc.parse();
        let mut r = self.reader(doc.name, parsed, PathBuf::from("."));
        r.text = doc.text;
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
        let mut r = self.reader(name, doc, paths::base_dir(path));
        // Source files always show their line numbers.
        r.line_numbers |= code_file;
        r.text = src;
        r.watch(path.to_path_buf());
        Ok(r)
    }
}
