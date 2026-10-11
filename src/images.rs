//! Pictures for image blocks: loaded in the background, then drawn with
//! the best protocol the terminal supports (kitty, iTerm2, sixel, or
//! half-block characters).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::{Context, Result};
use image::DynamicImage;
use std::cell::OnceCell;
use std::rc::Rc;

use ratatui_image::picker::Picker;
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::protocol::StatefulProtocol;

use crate::render::ImageSizes;

/// Largest image file read or downloaded.
const MAX_BYTES: u64 = 20 * 1024 * 1024;

/// Where an image comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    File(PathBuf),
    Url(String),
}

enum Entry {
    Loading,
    Ready {
        protocol: Box<StatefulProtocol>,
        cells: (u16, u16),
    },
    /// Unreadable or unsupported (SVG, for one): the alt text stays.
    Failed,
}

/// The terminal's image support, shared by every reader and worked out
/// the first time a picture is ready to show, so documents without images
/// never wait on the terminal query.
#[derive(Clone)]
pub struct Support(Rc<OnceCell<Picker>>);

impl Support {
    pub fn lazy() -> Self {
        Self(Rc::new(OnceCell::new()))
    }

    /// Support that's already known, e.g. half-blocks in tests.
    pub fn fixed(picker: Picker) -> Self {
        Self(Rc::new(OnceCell::from(picker)))
    }

    fn picker(&self) -> &Picker {
        self.0.get_or_init(detect)
    }
}

/// Images of the open document, keyed by URL as written in it.
pub struct Images {
    /// `None` turns images off.
    support: Option<Support>,
    entries: HashMap<String, Entry>,
    tx: Sender<(String, Result<DynamicImage>)>,
    rx: Receiver<(String, Result<DynamicImage>)>,
}

impl Default for Images {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Images {
    pub fn new(support: Option<Support>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            support,
            entries: HashMap::new(),
            tx,
            rx,
        }
    }

    pub fn enabled(&self) -> bool {
        self.support.is_some()
    }

    /// Start loading `key` from `source`, unless it's already known.
    pub fn request(&mut self, key: &str, source: Source) {
        if !self.enabled() || self.entries.contains_key(key) {
            return;
        }
        self.entries.insert(key.to_string(), Entry::Loading);
        let tx = self.tx.clone();
        let key = key.to_string();
        std::thread::spawn(move || {
            let _ = tx.send((key, load(&source)));
        });
    }

    /// Take in images that finished loading. True when any did.
    pub fn poll(&mut self) -> bool {
        let Some(support) = &self.support else {
            return false;
        };
        let mut changed = false;
        while let Ok((key, result)) = self.rx.try_recv() {
            let entry = match result {
                Ok(img) => {
                    let picker = support.picker();
                    let font = picker.font_size();
                    let cells = (
                        img.width().div_ceil(u32::from(font.width.max(1))) as u16,
                        img.height().div_ceil(u32::from(font.height.max(1))) as u16,
                    );
                    Entry::Ready {
                        protocol: Box::new(picker.new_resize_protocol(img)),
                        cells,
                    }
                }
                Err(_) => Entry::Failed,
            };
            self.entries.insert(key, entry);
            changed = true;
        }
        changed
    }

    /// Cell sizes of the images ready to draw.
    pub fn sizes(&self) -> ImageSizes {
        self.entries
            .iter()
            .filter_map(|(k, e)| match e {
                Entry::Ready { cells, .. } => Some((k.clone(), *cells)),
                _ => None,
            })
            .collect()
    }

    pub fn protocol(&mut self, key: &str) -> Option<&mut StatefulProtocol> {
        match self.entries.get_mut(key) {
            Some(Entry::Ready { protocol, .. }) => Some(protocol),
            _ => None,
        }
    }
}

fn load(source: &Source) -> Result<DynamicImage> {
    let bytes = match source {
        Source::File(path) => {
            let len = std::fs::metadata(path)
                .with_context(|| format!("reading {}", path.display()))?
                .len();
            anyhow::ensure!(len <= MAX_BYTES, "{} is too large", path.display());
            std::fs::read(path).with_context(|| format!("reading {}", path.display()))?
        }
        Source::Url(url) => crate::remote::fetch_bytes(url, MAX_BYTES)?,
    };
    Ok(image::load_from_memory(&bytes)?)
}

/// Ask the terminal which image protocol and font size it has. Falls back
/// to half-block characters when it doesn't answer in time (or stdin isn't
/// the terminal, as with `bastian -`).
fn detect() -> Picker {
    use std::io::IsTerminal;
    let options = QueryStdioOptions {
        timeout: std::time::Duration::from_millis(300),
        ..QueryStdioOptions::default()
    };
    if std::io::stdin().is_terminal()
        && let Ok(picker) = Picker::from_query_stdio_with_options(options)
    {
        return picker;
    }
    Picker::halfblocks()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_pictures_in_the_background() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("dot.png");
        image::RgbImage::from_pixel(40, 30, image::Rgb([200, 30, 30]))
            .save(&png)
            .unwrap();
        std::fs::write(dir.path().join("logo.svg"), "<svg/>").unwrap();

        let mut images = Images::new(Some(Support::fixed(Picker::halfblocks())));
        images.request("dot.png", Source::File(png));
        images.request("logo.svg", Source::File(dir.path().join("logo.svg")));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while images.sizes().is_empty()
            || images.entries.values().any(|e| matches!(e, Entry::Loading))
        {
            images.poll();
            assert!(std::time::Instant::now() < deadline, "images did not load");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let sizes = images.sizes();
        assert!(sizes.contains_key("dot.png"));
        assert!(!sizes.contains_key("logo.svg"));
        assert!(images.protocol("dot.png").is_some());

        // Off: nothing is loaded.
        let mut off = Images::default();
        off.request("dot.png", Source::File(dir.path().join("dot.png")));
        assert!(off.entries.is_empty());
    }
}
