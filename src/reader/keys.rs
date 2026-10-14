use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Action, LinkPicker, Reader};
use crate::links;
use crate::nav;
use crate::outline::{self, Outline};
use crate::prompt::{self, Edited, Prompt};

impl Reader {
    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        self.message = None;
        if self.help {
            self.help = false;
            return Action::None;
        }
        if self.link_picker.is_some() {
            return self.on_link_key(key);
        }
        if let Some(outline) = &mut self.outline {
            match outline.on_key(key) {
                outline::Event::None => {}
                outline::Event::Close => self.outline = None,
                outline::Event::Jump(line) => {
                    self.outline = None;
                    self.scroll = line.min(self.max_scroll());
                }
            }
            return Action::None;
        }
        if self.confirm.is_some() {
            self.on_confirm_key(key);
            return Action::None;
        }
        if self.confirm_open.is_some() {
            return self.on_open_confirm_key(key);
        }
        if self.prompt.is_some() {
            match prompt::drive(&mut self.prompt, key) {
                Some(Edited::Text(q)) => self.set_query(&q),
                Some(Edited::Cancelled) => self.search = None,
                None => {}
            }
            return Action::None;
        }

        let page = self.height.max(1) as isize;
        let ctrl = nav::ctrl(&key);
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Esc if self.link_focus.is_some() => self.link_focus = None,
            KeyCode::Esc if self.details_focus.is_some() => self.details_focus = None,
            KeyCode::Esc if self.search.is_some() => self.search = None,
            KeyCode::Esc if self.focus.is_some() => self.focus = None,
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => return Action::Back,
            KeyCode::Char('d') if ctrl => self.scroll_by(page / 2),
            KeyCode::Char('u') if ctrl => self.scroll_by(-page / 2),
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::Char(' ' | 'f') | KeyCode::PageDown => self.scroll_by(page),
            KeyCode::Char('b') | KeyCode::PageUp => self.scroll_by(-page),
            KeyCode::Char('g') | KeyCode::Home => self.jump_to(0),
            KeyCode::Char('G') | KeyCode::End => self.jump_to(usize::MAX),
            KeyCode::Char('/') => self.prompt = Some(Prompt::new("")),
            KeyCode::Char('n') => self.step_match(true),
            KeyCode::Char('N') => self.step_match(false),
            KeyCode::Char('s') => {
                self.details_focus = None;
                self.move_link_focus(true);
            }
            KeyCode::Char('S') => {
                self.details_focus = None;
                self.move_link_focus(false);
            }
            KeyCode::Enter if self.link_focus.is_some() => return self.follow_focused_link(),
            KeyCode::Char('d') => self.move_details_focus(true),
            KeyCode::Char('D') => self.move_details_focus(false),
            KeyCode::Enter if self.details_focus.is_some() => self.toggle_details(),
            KeyCode::Tab => {
                self.link_focus = None;
                self.details_focus = None;
                self.move_focus(true);
            }
            KeyCode::BackTab => {
                self.link_focus = None;
                self.details_focus = None;
                self.move_focus(false);
            }
            KeyCode::Enter | KeyCode::Char('r') => self.run_focused(false),
            KeyCode::Char('R') => self.run_focused(true),
            KeyCode::Char('x') => self.cancel(),
            KeyCode::Char('o') => {
                if let Some(run) = self.focus.and_then(|id| self.runs.get_mut(&id)) {
                    run.expanded = !run.expanded;
                    self.dirty = true;
                }
            }
            KeyCode::Char('y') => match self.focus.and_then(|id| self.code(id)) {
                Some(c) => {
                    let src = c.src.trim_end_matches('\n').to_string();
                    self.message = Some(format!("copied {}", self.label(c.id)));
                    return Action::Copy(src);
                }
                None => self.message = Some("focus a block with Tab first".into()),
            },
            KeyCode::Char('L') => {
                let links = links::collect(&self.doc);
                if links.is_empty() {
                    self.message = Some("no links in this document".into());
                } else {
                    self.link_picker = Some(LinkPicker { links, selected: 0 });
                }
            }
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('T') => return Action::PickTheme,
            KeyCode::Char('i') => return Action::ToggleImages,
            KeyCode::Char('O') => {
                self.outline = Outline::new(&self.doc, &self.rendered, self.scroll);
                if self.outline.is_none() {
                    self.message = Some("no headings in this document".into());
                }
            }
            KeyCode::Char('#') => self.line_numbers = !self.line_numbers,
            KeyCode::Char('e') => match &self.source {
                Some(path) => return Action::Edit(path.clone(), self.source_line()),
                None => self.message = Some("no local file to edit".into()),
            },
            KeyCode::Char('c') if !ctrl => {
                if self.text.is_empty() {
                    self.message = Some("nothing to copy".into());
                } else {
                    let lines = self.text.lines().count();
                    self.message = Some(format!("copied document ({lines} lines)"));
                    return Action::Copy(self.text.clone());
                }
            }
            _ => {}
        }
        Action::None
    }
}
