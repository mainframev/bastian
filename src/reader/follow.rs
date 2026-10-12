use std::ops::Range;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Action, Reader};
use crate::doc::{self, Block};
use crate::links::{self, Target};
use crate::nav::{self, Motion};
use crate::render;

impl Reader {
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
            return crate::remote::resolve_link(base, url).map(|u| u.to_string());
        }
        let (path, fragment) = url
            .split_once('#')
            .map_or((url, None), |(p, f)| (p, Some(f)));
        let abs = std::path::absolute(self.base_dir.join(path)).ok()?;
        let mut file = url::Url::from_file_path(abs).ok()?;
        file.set_fragment(fragment);
        Some(file.to_string())
    }

    pub(super) fn on_link_key(&mut self, key: KeyEvent) -> Action {
        let Some(picker) = &mut self.link_picker else {
            return Action::None;
        };
        let motion = match key.code {
            KeyCode::Tab => Some(Motion::Down),
            KeyCode::BackTab => Some(Motion::Up),
            _ => nav::list_motion(&key),
        };
        if let Some(motion) = motion {
            picker.selected = nav::step(picker.selected, picker.links.len(), motion);
            return Action::None;
        }
        match key.code {
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

    pub(super) fn follow_link(&mut self, url: &str) -> Action {
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
                    Some(line) => self.jump_to(line),
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
    pub(super) fn heading_line(&self, anchor: &str) -> Option<usize> {
        // Top-level headings know where they start, so a TOC line or a
        // paragraph with the same words can't be mistaken for them.
        let top = self
            .doc
            .blocks
            .iter()
            .enumerate()
            .find_map(|(i, b)| match b {
                Block::Heading { inlines, .. }
                    if links::slug(&doc::anchor_text(inlines)) == anchor =>
                {
                    Some(i)
                }
                Block::Footnote { label, .. } if doc::footnote_anchor(label) == anchor => Some(i),
                _ => None,
            });
        if let Some(i) = top {
            return self.rendered.block_starts.get(i).copied();
        }
        // Headings nested in quotes, lists or components: match by text.
        let (_, text) = heading_texts(&self.doc.blocks)
            .into_iter()
            .find(|(a, _)| links::slug(a) == anchor)?;
        self.rendered.lines.iter().position(|line| {
            let plain = render::plain(line);
            let plain = plain.trim().trim_start_matches('#').trim();
            !plain.is_empty() && text.starts_with(plain)
        })
    }

    /// The links focus steps through: ranges of `rendered.links`, one per
    /// link, joining the pieces of a link that wraps onto further lines.
    pub fn link_stops(&self) -> Vec<Range<usize>> {
        let links = &self.rendered.links;
        let mut stops: Vec<Range<usize>> = Vec::new();
        for (i, l) in links.iter().enumerate() {
            match stops.last_mut() {
                Some(s) if links[i - 1].id == l.id => s.end = i + 1,
                _ => stops.push(i..i + 1),
            }
        }
        stops
    }

    /// `(n, total)` of the focused link, for the status bar.
    pub fn link_position(&self) -> Option<(usize, usize)> {
        let i = self.link_focus?;
        Some((i + 1, self.link_stops().len()))
    }

    /// Focus the next or previous link; from nothing, the first one in
    /// view (or the last, going back).
    pub(super) fn move_link_focus(&mut self, forward: bool) {
        let stops = self.link_stops();
        if stops.is_empty() {
            self.message = Some("no links in this document".into());
            return;
        }
        let line = |s: &Range<usize>| self.rendered.links[s.start].line;
        let next = match (self.link_focus, forward) {
            (Some(i), true) => (i + 1).min(stops.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
            (None, true) => stops
                .iter()
                .position(|s| line(s) >= self.scroll)
                .unwrap_or(stops.len() - 1),
            (None, false) => stops
                .iter()
                .rposition(|s| line(s) < self.scroll + self.height)
                .unwrap_or(0),
        };
        self.focus = None;
        self.link_focus = Some(next);
        let first = line(&stops[next]);
        let last = self.rendered.links[stops[next].end - 1].line;
        if first < self.scroll || last >= self.scroll + self.height {
            self.follow = None;
            self.scroll = first.saturating_sub(self.height / 3).min(self.max_scroll());
        }
        let url = self.rendered.links[stops[next].start].url.clone();
        self.link_preview = self.describe_link(&url);
    }

    /// Follow the focused link, letting go of it.
    pub(super) fn follow_focused_link(&mut self) -> Action {
        let url = self
            .link_focus
            .and_then(|i| self.link_stops().get(i).cloned())
            .map(|s| self.rendered.links[s.start].url.clone());
        self.link_focus = None;
        match url {
            Some(url) => self.follow_link(&url),
            None => Action::None,
        }
    }

    /// Where following `url` would go, in a few words.
    pub fn describe_link(&self, url: &str) -> String {
        if let Some(base) = &self.remote_base
            && !url.starts_with('#')
        {
            return match crate::remote::resolve_link(base, url) {
                Some(target) => format!("→ {target}"),
                None => format!("can't resolve {url}"),
            };
        }
        match links::resolve(url, &self.base_dir) {
            Target::Anchor(anchor) => self.describe_anchor(&anchor),
            Target::Document(path) => {
                let shown = url.split('#').next().unwrap_or(url);
                match document_title(&path) {
                    Some(title) => format!("→ {shown} · {title}"),
                    None => format!("→ {shown}"),
                }
            }
            Target::External(target) => format!("→ {target}"),
            Target::Missing(path) => format!("not found: {}", path.display()),
        }
    }

    fn describe_anchor(&self, anchor: &str) -> String {
        let mut found = None;
        doc::visit(&self.doc.blocks, &mut |b| {
            if found.is_some() {
                return;
            }
            match b {
                Block::Heading { inlines, .. } => {
                    let text = doc::anchor_text(inlines);
                    if links::slug(&text) == anchor {
                        found = Some(format!("→ § {text}"));
                    }
                }
                Block::Footnote {
                    label,
                    number,
                    blocks,
                } if doc::footnote_anchor(label) == anchor => {
                    found = Some(format!(
                        "[{number}] {}",
                        crate::ui::text::shorten_right(&blocks_text(blocks), 60)
                    ));
                }
                _ => {}
            }
        });
        found.unwrap_or_else(|| format!("no heading #{anchor}"))
    }
}

/// The title of the Markdown file at `path`, from its first few KiB, so
/// previewing a link doesn't read a whole large file.
fn document_title(path: &std::path::Path) -> Option<String> {
    use std::io::Read;
    let mut head = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(8 * 1024)
        .read_to_end(&mut head)
        .ok()?;
    doc::parse(&String::from_utf8_lossy(&head)).title()
}

/// The words of `blocks`, on one line.
fn blocks_text(blocks: &[Block]) -> String {
    let mut parts = Vec::new();
    doc::visit(blocks, &mut |b| match b {
        Block::Paragraph(inlines) | Block::Heading { inlines, .. } => {
            parts.push(doc::plain_text(inlines));
        }
        Block::Code(c) => parts.push(c.src.clone()),
        _ => {}
    });
    parts
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `(anchor text, shown text)` of every heading.
pub(super) fn heading_texts(blocks: &[Block]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    doc::visit(blocks, &mut |b| {
        if let Block::Heading { inlines, .. } = b {
            out.push((doc::anchor_text(inlines), doc::plain_text(inlines)));
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::doc::parse;
    use crate::reader::Reader;
    use crate::render::theme::Theme;

    fn press(r: &mut Reader, code: KeyCode, mods: KeyModifiers) {
        r.on_key(KeyEvent::new(code, mods));
    }

    #[test]
    fn link_picker_takes_the_list_keys() {
        let src = "[a](https://a.dev) [b](https://b.dev) [c](https://c.dev)\n";
        let mut r = Reader::new("t".into(), parse(src), Theme::dark(), 80);
        r.layout(80, 20);
        press(&mut r, KeyCode::Char('L'), KeyModifiers::NONE);
        let selected = |r: &Reader| r.link_picker.as_ref().unwrap().selected;
        press(&mut r, KeyCode::Char('G'), KeyModifiers::NONE);
        assert_eq!(selected(&r), 2);
        press(&mut r, KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(selected(&r), 1);
        press(&mut r, KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(selected(&r), 0);
        press(&mut r, KeyCode::Char('n'), KeyModifiers::CONTROL);
        press(&mut r, KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(selected(&r), 2);
        press(&mut r, KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(selected(&r), 1);
    }
}
