use std::collections::HashMap;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Action, Overlay, Reader};
use crate::doc::{self, Block};
use crate::links::{self, Target};
use crate::nav::{self, Motion};
use crate::prompt::{Prompt, PromptEvent};
use crate::render;

impl Reader {
    /// Absolute target for a link written as `url`, for terminal hyperlinks.
    /// A click there opens without asking, so only links bastian would open
    /// without asking get one: web and mail URLs, and existing local
    /// Markdown files as `file://` URLs. In a remote document, links resolve
    /// against its URL and only web targets qualify. In-document `#anchors`,
    /// other schemes and other local files have none; the link picker still
    /// follows them, behind a prompt.
    pub(super) fn hyperlink_target(&self, url: &str) -> Option<String> {
        if url.starts_with('#') {
            return None;
        }
        if let Some(base) = &self.remote_base {
            return crate::remote::resolve_link(base, url)
                .map(|u| u.to_string())
                .filter(|u| links::is_web(u));
        }
        if let Ok(abs) = url::Url::parse(url) {
            if links::is_web(abs.as_str()) {
                return Some(abs.to_string());
            }
            // `to_file_path` doesn't check the scheme itself.
            if abs.scheme() != "file" {
                return None;
            }
            let path = abs.to_file_path().ok()?;
            return crate::open::is_markdown_file(&path).then(|| abs.to_string());
        }
        let (path, fragment) = url
            .split_once('#')
            .map_or((url, None), |(p, f)| (p, Some(f)));
        let abs = std::path::absolute(self.base_dir.join(path)).ok()?;
        if !crate::open::is_markdown_file(&abs) {
            return None;
        }
        let mut file = url::Url::from_file_path(abs).ok()?;
        file.set_fragment(fragment);
        Some(file.to_string())
    }

    pub(super) fn on_link_key(&mut self, key: KeyEvent) -> Action {
        // As many rows as the popup shows: those it had room for, or
        // the document's before it's drawn, and no more than match.
        let fallback = self.height.saturating_sub(3);
        let Some(Overlay::Links(picker)) = &mut self.overlay else {
            return Action::None;
        };
        let len = picker.shown().len();
        let room = match picker.rows.get() {
            0 => fallback,
            rows => rows,
        };
        let page = room.min(len).max(1);
        let ctrl = nav::ctrl(&key);
        let typing = picker.prompt.is_some();
        // Only keys that don't type text move the list while filtering.
        let motion = match key.code {
            KeyCode::Tab | KeyCode::Down => Some(Motion::Down),
            KeyCode::BackTab | KeyCode::Up => Some(Motion::Up),
            KeyCode::Char('n') if ctrl => Some(Motion::Down),
            KeyCode::Char('p') if ctrl => Some(Motion::Up),
            _ if typing => None,
            _ => nav::list_motion(&key),
        };
        let jump = match key.code {
            KeyCode::PageDown => Some(page as isize),
            KeyCode::PageUp => Some(-(page as isize)),
            KeyCode::Char('d') if ctrl => Some((page / 2).max(1) as isize),
            KeyCode::Char('u') if ctrl => Some(-((page / 2).max(1) as isize)),
            _ => None,
        };
        if let Some(motion) = motion {
            picker.selected = nav::step(picker.selected, len, motion);
            return Action::None;
        }
        if let Some(by) = jump {
            let last = len.saturating_sub(1);
            picker.selected = picker.selected.saturating_add_signed(by).min(last);
            return Action::None;
        }
        match key.code {
            KeyCode::Enter => {
                let Some(url) = picker.current().map(|l| l.url.clone()) else {
                    return Action::None;
                };
                self.overlay = None;
                return self.follow_link(&url);
            }
            KeyCode::Esc if typing => {
                picker.prompt = None;
                picker.set_filter(String::new());
            }
            _ if typing => {
                let prompt = picker.prompt.as_mut().expect("typing");
                if prompt.on_key(key) == PromptEvent::Changed {
                    let text = prompt.text().to_string();
                    picker.set_filter(text);
                }
            }
            KeyCode::Char('/') => picker.prompt = Some(Prompt::new(&picker.filter)),
            KeyCode::Esc if !picker.filter.is_empty() => picker.set_filter(String::new()),
            KeyCode::Esc | KeyCode::Char('q' | 'L') => self.overlay = None,
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
                Some(target) => self.open_external(target.to_string()),
                None => {
                    self.message = Some(format!("can't resolve link {url}"));
                    Action::None
                }
            };
        }
        match links::resolve(url, &self.base_dir) {
            Target::Document(path) => Action::OpenDocument {
                path,
                anchor: url
                    .split_once('#')
                    .map(|(_, a)| a.to_string())
                    .filter(|a| !a.is_empty()),
            },
            Target::External(target) => self.open_external(target),
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

    /// Web and mail links open right away; anything else could launch an
    /// app or run a script, so it waits for a yes/no.
    fn open_external(&mut self, target: String) -> Action {
        if links::is_web(&target) {
            Action::OpenExternal(target)
        } else {
            self.overlay = Some(Overlay::OpenConfirm(target));
            Action::None
        }
    }

    pub(super) fn on_open_confirm_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                if let Some(Overlay::OpenConfirm(target)) = self.overlay.take() {
                    return Action::OpenExternal(target);
                }
            }
            KeyCode::Char('n' | 'N' | 'q') | KeyCode::Esc => self.overlay = None,
            _ => {}
        }
        Action::None
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
            let plain = plain.trim();
            !plain.is_empty() && text.starts_with(plain)
        })
    }

    /// The links focus steps through, in screen order: for each link, the
    /// indexes of its pieces in `rendered.links` (one per line it's on; in
    /// a table, other cells' links can come between them).
    pub fn link_stops(&self) -> Vec<Vec<usize>> {
        let mut stops: Vec<Vec<usize>> = Vec::new();
        let mut stop_of: HashMap<usize, usize> = HashMap::new();
        for (i, l) in self.rendered.links.iter().enumerate() {
            if let Some(&s) = stop_of.get(&l.id) {
                stops[s].push(i);
            } else {
                stop_of.insert(l.id, stops.len());
                stops.push(vec![i]);
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
        let links = &self.rendered.links;
        let first_line = |s: &[usize]| links[s[0]].line;
        let last_line = |s: &[usize]| links[s[s.len() - 1]].line;
        let next = match (self.link_focus, forward) {
            (Some(i), true) => (i + 1).min(stops.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
            // The first link with any part in view (or below it).
            (None, true) => stops
                .iter()
                .position(|s| last_line(s) >= self.scroll)
                .unwrap_or(stops.len() - 1),
            (None, false) => stops
                .iter()
                .rposition(|s| first_line(s) < self.scroll + self.height)
                .unwrap_or(0),
        };
        let first = first_line(&stops[next]);
        let last = last_line(&stops[next]);
        let url = links[stops[next][0]].url.clone();
        self.focus = None;
        self.link_focus = Some(next);
        if first < self.scroll || last >= self.scroll + self.height {
            self.follow = None;
            self.scroll = first.saturating_sub(self.height / 3).min(self.max_scroll());
        }
        self.link_preview = self.describe_link(&url);
    }

    /// Follow the focused link, letting go of it.
    pub(super) fn follow_focused_link(&mut self) -> Action {
        let url = self
            .link_focus
            .and_then(|i| self.link_stops().get(i).cloned())
            .map(|s| self.rendered.links[s[0]].url.clone());
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
                match crate::open::peek_title(&path) {
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
                        crate::text::shorten_right(&blocks_text(blocks), 60)
                    ));
                }
                _ => {}
            }
        });
        found.unwrap_or_else(|| format!("no heading #{anchor}"))
    }
}

/// The words of `blocks`, on one line.
fn blocks_text(blocks: &[Block]) -> String {
    let mut parts = Vec::new();
    doc::visit(blocks, &mut |b| match b {
        Block::Paragraph(inlines) | Block::Heading { inlines, .. } => {
            parts.push(doc::plain_text(inlines));
        }
        Block::Code(c) => parts.push(c.src.clone()),
        Block::Callout { kind, .. } => parts.push(format!("{}:", kind.title())),
        Block::Table { header, rows, .. } => {
            for cell in header.iter().chain(rows.iter().flatten()) {
                parts.push(doc::plain_text(cell));
            }
        }
        Block::Image { alt, .. } => parts.push(alt.clone()),
        Block::Details { summary, .. } => parts.push(doc::plain_text(summary)),
        Block::Label(text) => parts.push(text.clone()),
        Block::Quote(_) | Block::List { .. } | Block::Footnote { .. } | Block::Rule => {}
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
    use crate::reader::{LinkPicker, Options, Overlay, Reader, Source};

    fn press(r: &mut Reader, code: KeyCode, mods: KeyModifiers) {
        r.on_key(KeyEvent::new(code, mods));
    }

    fn picker(r: &Reader) -> &LinkPicker {
        match &r.overlay {
            Some(Overlay::Links(p)) => p,
            _ => panic!("no link picker"),
        }
    }

    fn picker_open(r: &Reader) -> bool {
        matches!(r.overlay, Some(Overlay::Links(_)))
    }

    #[test]
    fn link_picker_takes_the_list_keys() {
        let src = "[a](https://a.dev) [b](https://b.dev) [c](https://c.dev)\n";
        let mut r = Reader::new(Source::in_memory("t", parse(src)), &Options::default());
        r.layout(80, 20);
        press(&mut r, KeyCode::Char('L'), KeyModifiers::NONE);
        let selected = |r: &Reader| match &r.overlay {
            Some(Overlay::Links(p)) => p.selected,
            _ => panic!("no link picker"),
        };
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

    #[test]
    fn link_picker_pages_through_long_lists() {
        let src: String = (0..50)
            .map(|i| format!("[l{i}](https://{i}.dev)\n\n"))
            .collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        // 20 rows of document: 17 in the list.
        r.layout(80, 20);
        press(&mut r, KeyCode::Char('L'), KeyModifiers::NONE);
        let selected = |r: &Reader| picker(r).selected;
        press(&mut r, KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(selected(&r), 17);
        press(&mut r, KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(selected(&r), 25);
        press(&mut r, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(selected(&r), 17);
        press(&mut r, KeyCode::PageUp, KeyModifiers::NONE);
        assert_eq!(selected(&r), 0);
        for _ in 0..4 {
            press(&mut r, KeyCode::PageDown, KeyModifiers::NONE);
        }
        assert_eq!(selected(&r), 49);
    }

    #[test]
    fn link_picker_pages_by_the_rows_it_shows() {
        let src: String = (0..50)
            .map(|i| format!("[l{i}](https://{i}.dev)\n\n"))
            .collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 20);
        press(&mut r, KeyCode::Char('L'), KeyModifiers::NONE);
        // As drawn at 80×24, whatever the document's height.
        picker(&r).rows.set(18);
        press(&mut r, KeyCode::PageDown, KeyModifiers::NONE);
        assert_eq!(picker(&r).selected, 18);
        press(&mut r, KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(picker(&r).selected, 27);
        // Fewer matches than rows: the page is the matches.
        press(&mut r, KeyCode::Char('/'), KeyModifiers::NONE);
        press(&mut r, KeyCode::Char('4'), KeyModifiers::NONE);
        let shown = picker(&r).shown().len();
        assert!(shown > 4, "{shown}");
        press(&mut r, KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(picker(&r).selected, shown / 2);
    }

    #[test]
    fn link_picker_filters_by_text_or_url() {
        let src = "[Alma](https://a.dev) [beta](https://b.dev/guide) [Gamma](https://c.dev)\n";
        let mut r = Reader::new(Source::in_memory("t", parse(src)), &Options::default());
        r.layout(80, 20);
        press(&mut r, KeyCode::Char('L'), KeyModifiers::NONE);
        press(&mut r, KeyCode::Char('/'), KeyModifiers::NONE);
        // Letters type into the filter, not move the list.
        for c in "GU".chars() {
            press(&mut r, KeyCode::Char(c), KeyModifiers::NONE);
        }
        assert_eq!(picker(&r).filter, "GU");
        let texts = |r: &Reader| {
            let p = picker(r);
            p.shown()
                .iter()
                .map(|&i| p.links[i].text.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(&r), ["beta"]);
        // Esc while typing drops the filter; again closes the list.
        press(&mut r, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(texts(&r).len(), 3);
        assert!(picker(&r).prompt.is_none());
        press(&mut r, KeyCode::Char('/'), KeyModifiers::NONE);
        press(&mut r, KeyCode::Char('m'), KeyModifiers::NONE);
        press(&mut r, KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(texts(&r), ["Alma", "Gamma"]);
        press(&mut r, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(picker(&r).current().unwrap().text, "Gamma");
        // Enter follows the selected match.
        let action = r.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            matches!(&action, crate::reader::Action::OpenExternal(url) if url == "https://c.dev")
        );
        assert!(!picker_open(&r));
    }

    #[test]
    fn link_picker_ignores_enter_with_no_matches() {
        let src = "[a](https://a.dev)\n";
        let mut r = Reader::new(Source::in_memory("t", parse(src)), &Options::default());
        r.layout(80, 20);
        press(&mut r, KeyCode::Char('L'), KeyModifiers::NONE);
        press(&mut r, KeyCode::Char('/'), KeyModifiers::NONE);
        press(&mut r, KeyCode::Char('z'), KeyModifiers::NONE);
        press(&mut r, KeyCode::Enter, KeyModifiers::NONE);
        assert!(picker_open(&r));
        // Closing the prompt keeps the filter; Esc clears it, then closes.
        press(&mut r, KeyCode::Esc, KeyModifiers::NONE);
        press(&mut r, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!picker_open(&r));
    }

    #[test]
    fn wiki_links_open_the_page_next_to_the_document() {
        use crate::reader::Action;

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Other Page.md"),
            "# Other\n\nintro\n\n## Getting Started\n\nstep one\n\nstep two\n\nthe end\n",
        )
        .unwrap();
        let source = Source {
            base_dir: dir.path().to_path_buf(),
            ..Source::in_memory("t", parse("See [[Other Page#Getting Started|it]].\n"))
        };
        let mut r = Reader::new(source, &Options::default());
        r.layout(80, 20);
        press(&mut r, KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(r.link_preview, "→ Other Page.md · Other");
        let Action::OpenDocument { path, anchor } =
            r.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        else {
            panic!("expected OpenDocument");
        };
        assert!(path.ends_with("Other Page.md"));
        assert_eq!(anchor.as_deref(), Some("getting-started"));

        // The page opens at that heading, not at its top.
        let text = std::fs::read_to_string(&path).unwrap();
        let mut page = Reader::new(
            Source::in_memory("Other Page.md", parse(&text)),
            &Options::default(),
        );
        page.go_to_anchor(anchor.as_deref().unwrap());
        page.layout(80, 2);
        assert_eq!(Some(page.scroll), page.heading_line("getting-started"));
        assert!(page.scroll > 0);
    }
}
