//! Pictures for image blocks: loaded in the background, then drawn with
//! the best protocol the terminal supports (kitty, iTerm2, sixel, or
//! half-block characters).

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::{Context, Result};
use image::DynamicImage;

use ratatui_image::FontSize;
use ratatui_image::picker::{Picker, ProtocolType};
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
        image: DynamicImage,
        /// Natural size in cells.
        cells: (u16, u16),
        /// The picture scaled to fill a slot exactly, by slot size, so a
        /// slot partly scrolled off can be cropped to its visible rows.
        scaled: HashMap<(u16, u16), Box<StatefulProtocol>>,
    },
    /// Unreadable or unsupported (SVG, for one): the alt text stays.
    Failed,
}

/// The terminal's image support, shared by every reader.
#[derive(Clone)]
pub struct Support(Rc<Picker>);

impl Support {
    /// Ask the terminal (see `term::graphics`). Call before the TUI starts:
    /// the reply has to be read before anything else reads the keyboard.
    pub fn detect() -> Self {
        Self::fixed(picker_for(crate::term::graphics()))
    }

    /// Support that's already known, e.g. half-blocks in tests.
    pub fn fixed(picker: Picker) -> Self {
        Self(Rc::new(picker))
    }

    fn picker(&self) -> &Picker {
        &self.0
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

    /// Turn pictures on with `support`, or off with `None`. Loaded images
    /// stay cached, so turning them back on doesn't load them again.
    pub fn set_support(&mut self, support: Option<Support>) {
        self.support = support;
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
                        image: img,
                        cells,
                        scaled: HashMap::new(),
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
        if !self.enabled() {
            return ImageSizes::new();
        }
        self.entries
            .iter()
            .filter_map(|(k, e)| match e {
                Entry::Ready { cells, .. } => Some((k.clone(), *cells)),
                _ => None,
            })
            .collect()
    }

    /// The picture for `key` sized to a `cols`×`rows` slot. Built on first
    /// use for that size (e.g. after a resize) and reused while scrolling.
    pub fn protocol(&mut self, key: &str, cols: u16, rows: u16) -> Option<&mut StatefulProtocol> {
        let support = self.support.as_ref()?;
        let Some(Entry::Ready { image, scaled, .. }) = self.entries.get_mut(key) else {
            return None;
        };
        let protocol = scaled.entry((cols, rows)).or_insert_with(|| {
            let picker = support.picker();
            let font = picker.font_size();
            let fitted = image.resize_exact(
                u32::from(cols) * u32::from(font.width.max(1)),
                u32::from(rows) * u32::from(font.height.max(1)),
                image::imageops::FilterType::Triangle,
            );
            Box::new(picker.new_resize_protocol(fitted))
        });
        Some(protocol)
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

/// The best protocol the terminal answered for: kitty graphics, then
/// iTerm2's (named by the environment, as it has no query), then sixel,
/// else half-blocks, which are also used inside tmux. The cell size comes from the reply, the window size,
/// or a guess.
fn picker_for(g: crate::term::Graphics) -> Picker {
    let cell = g
        .cell
        .map(|(w, h)| FontSize::new(w, h))
        .or_else(cell_size)
        .unwrap_or(FontSize::new(10, 20));
    // Also detects tmux and iTerm2-compatible terminals from the environment.
    #[allow(deprecated, reason = "the only constructor that doesn't read stdin")]
    let mut picker = Picker::from_fontsize(cell);
    if picker.tmux_detected() {
        // tmux answers for itself (it may claim sixel the outer terminal
        // can't show) and drops kitty graphics without passthrough.
        picker.set_protocol_type(ProtocolType::Halfblocks);
    } else if g.kitty {
        picker.set_protocol_type(ProtocolType::Kitty);
    } else if picker.protocol_type() == ProtocolType::Halfblocks && g.sixel {
        picker.set_protocol_type(ProtocolType::Sixel);
    }
    picker
}

/// Pixel size of one character cell, from the terminal's window size.
fn cell_size() -> Option<FontSize> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let tty = std::fs::File::open("/dev/tty").ok()?;
        // SAFETY: TIOCGWINSZ fills a winsize struct we own, on an open fd.
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(tty.as_raw_fd(), libc::TIOCGWINSZ, &raw mut ws) } != 0 {
            return None;
        }
        (ws.ws_xpixel > 0 && ws.ws_ypixel > 0 && ws.ws_col > 0 && ws.ws_row > 0)
            .then(|| FontSize::new(ws.ws_xpixel / ws.ws_col, ws.ws_ypixel / ws.ws_row))
    }
    #[cfg(not(unix))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_follows_the_terminal_reply() {
        use crate::term::Graphics;
        // Outside tmux (the test may run inside it).
        if Picker::halfblocks().tmux_detected() {
            return;
        }
        let kitty = picker_for(Graphics {
            kitty: true,
            sixel: true,
            cell: Some((9, 18)),
        });
        assert_eq!(kitty.protocol_type(), ProtocolType::Kitty);
        let font = kitty.font_size();
        assert_eq!((font.width, font.height), (9, 18));
        let none = picker_for(Graphics::default());
        assert!(matches!(
            none.protocol_type(),
            ProtocolType::Halfblocks | ProtocolType::Iterm2
        ));
    }

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
        assert!(images.protocol("dot.png", 4, 2).is_some());

        // Off: nothing is loaded.
        let mut off = Images::default();
        off.request("dot.png", Source::File(dir.path().join("dot.png")));
        assert!(off.entries.is_empty());
    }
}
