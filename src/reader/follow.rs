use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::doc::{self, Block};
use crate::links::{self, Target};
use crate::nav::{self, Motion};
use crate::render;

use super::{Action, Reader};

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
            return base.join(url).ok().map(|u| u.to_string());
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
        let len = picker.links.len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                picker.selected = nav::step(picker.selected, len, Motion::Down);
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                picker.selected = nav::step(picker.selected, len, Motion::Up);
            }
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
                    if links::slug(&doc::plain_text(inlines)) == anchor =>
                {
                    Some(i)
                }
                _ => None,
            });
        if let Some(i) = top {
            return self.rendered.block_starts.get(i).copied();
        }
        // Headings nested in quotes, lists or components: match by text.
        let text = heading_texts(&self.doc.blocks)
            .into_iter()
            .find(|h| links::slug(h) == anchor)?;
        self.rendered.lines.iter().position(|line| {
            let plain = render::plain(line);
            let plain = plain.trim().trim_start_matches('#').trim();
            !plain.is_empty() && text.starts_with(plain)
        })
    }
}

pub(super) fn heading_texts(blocks: &[Block]) -> Vec<String> {
    let mut out = Vec::new();
    doc::visit(blocks, &mut |b| {
        if let Block::Heading { inlines, .. } = b {
            out.push(doc::plain_text(inlines));
        }
    });
    out
}
