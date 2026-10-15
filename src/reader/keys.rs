use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{Action, LinkPicker, Overlay, Reader};
use crate::links;
use crate::nav;
use crate::prompt::{Prompt, PromptEvent};

impl Reader {
    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        self.message = None;
        self.goto_mark = None;
        // Only the key right after a jump by number goes into its section,
        // whether or not the keys between could move the view.
        let by_number = std::mem::take(&mut self.toc_by_number);
        match &mut self.overlay {
            Some(Overlay::Help(help)) => {
                help.on_key(&key);
                if !help.open {
                    self.overlay = None;
                }
                return Action::None;
            }
            Some(Overlay::Links(_)) => return self.on_link_key(key),
            Some(Overlay::Location(_)) => return self.on_location_key(key),
            Some(Overlay::RunConfirm(_)) => {
                self.on_confirm_key(key);
                return Action::None;
            }
            Some(Overlay::OpenConfirm(_)) => return self.on_open_confirm_key(key),
            Some(Overlay::GoTo(_)) => {
                self.on_goto_key(key);
                return Action::None;
            }
            Some(Overlay::History(_)) => return self.on_history_key(key),
            Some(Overlay::Prompt(prompt)) => {
                let event = prompt.on_key(key);
                let text = prompt.text().to_string();
                match event {
                    PromptEvent::Changed => self.set_query(&text),
                    PromptEvent::Submit => self.overlay = None,
                    PromptEvent::Cancel => {
                        self.overlay = None;
                        self.search = None;
                    }
                    PromptEvent::None => {}
                }
                return Action::None;
            }
            None => {}
        }

        let page = self.height.max(1) as isize;
        let ctrl = nav::ctrl(&key);
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            // Esc returns to plain reading in one step; a second one goes back.
            KeyCode::Esc if self.reading_plainly() => return Action::Back,
            KeyCode::Esc => {
                self.link_focus = None;
                self.details_focus = None;
                self.search = None;
                self.focus = None;
            }
            KeyCode::Char('h') | KeyCode::Left => return Action::Back,
            KeyCode::Char('d') if ctrl => self.scroll_by(page / 2),
            KeyCode::Char('u') if ctrl => self.scroll_by(-page / 2),
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::Char(' ' | 'f') | KeyCode::PageDown => self.scroll_by(page),
            KeyCode::Char('b') | KeyCode::PageUp => self.scroll_by(-page),
            KeyCode::Char('g') | KeyCode::Home => self.jump_to(0),
            KeyCode::Char('G') | KeyCode::End => self.jump_to(usize::MAX),
            KeyCode::Char('/') => self.overlay = Some(Overlay::Prompt(Prompt::new(""))),
            KeyCode::Char(':') => self.open_goto(),
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
            KeyCode::Char('v') => self.toggle_diagram(),
            KeyCode::Char('L') => {
                let links = links::collect(&self.doc);
                if links.is_empty() {
                    self.message = Some("no links in this document".into());
                } else {
                    self.overlay = Some(Overlay::Links(LinkPicker::new(links)));
                }
            }
            KeyCode::Char('p') => self.open_location(),
            KeyCode::Char('H') => return Action::ShowHistory,
            KeyCode::Char('?') => {
                let mut help = nav::Help::default();
                help.show();
                self.overlay = Some(Overlay::Help(help));
            }
            KeyCode::Char('T') => return Action::PickTheme,
            KeyCode::Char('i') => return Action::ToggleImages,
            KeyCode::Char('t') => self.toggle_toc(),
            KeyCode::Char('<') if self.toc => self.resize_toc(false),
            KeyCode::Char('>') if self.toc => self.resize_toc(true),
            KeyCode::Char(c @ '0'..='9') if self.toc => {
                // `0` is the tenth, as on a keyboard's number row.
                let n = c.to_digit(10).unwrap_or(0) as usize;
                self.jump_to_entry(if n == 0 { 10 } else { n }, by_number);
            }
            KeyCode::Char('J') => self.step_heading(true),
            KeyCode::Char('K') => self.step_heading(false),
            KeyCode::Char('#') => self.line_numbers = !self.line_numbers,
            KeyCode::Char('e') if ctrl => match &self.source {
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

    /// Nothing focused and no search: Esc has nothing left to dismiss.
    fn reading_plainly(&self) -> bool {
        self.link_focus.is_none()
            && self.details_focus.is_none()
            && self.search.is_none()
            && self.focus.is_none()
    }
}
