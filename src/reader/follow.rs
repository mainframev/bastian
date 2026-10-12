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
