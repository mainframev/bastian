use ratatui::crossterm::event::KeyEvent;

use super::{Overlay, Reader};
use crate::prompt::{Prompt, PromptEvent};

impl Reader {
    /// Open the `Line:` prompt.
    pub(super) fn open_goto(&mut self) {
        self.overlay = Some(Overlay::GoTo(Prompt::new("")));
    }

    pub(super) fn on_goto_key(&mut self, key: KeyEvent) {
        let Some(Overlay::GoTo(prompt)) = &mut self.overlay else {
            return;
        };
        match prompt.on_key(key) {
            PromptEvent::Submit => {
                let text = prompt.text().trim().to_string();
                self.overlay = None;
                match text.parse::<usize>() {
                    Ok(n) => self.go_to_line(n),
                    Err(_) if text.is_empty() => {}
                    Err(_) => self.message = Some(format!("not a line number: {text}")),
                }
            }
            PromptEvent::Cancel => self.overlay = None,
            PromptEvent::Changed | PromptEvent::None => {}
        }
    }

    /// Scroll line `n` (1-based, as the line numbers show it) to the top:
    /// a source line in code files, a rendered line otherwise. Past the
    /// end goes to the end.
    pub(super) fn go_to_line(&mut self, n: usize) {
        let want = n.saturating_sub(1);
        let lines = &self.rendered.src_line;
        let line = if lines.is_empty() {
            want
        } else {
            lines
                .iter()
                .position(|l| l.is_some_and(|s| s >= want))
                .unwrap_or(usize::MAX)
        };
        self.jump_to(line);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::doc::{parse, parse_for};
    use crate::reader::{Options, Reader, Source};

    fn press(r: &mut Reader, code: KeyCode) {
        r.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn type_line(r: &mut Reader, text: &str) {
        press(r, KeyCode::Char(':'));
        for c in text.chars() {
            press(r, KeyCode::Char(c));
        }
        press(r, KeyCode::Enter);
    }

    #[test]
    fn colon_goes_to_a_rendered_line() {
        let src: String = (0..50).map(|i| format!("para {i}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 10);
        type_line(&mut r, "21");
        assert_eq!(r.scroll, 20);
        assert!(r.goto().is_none());
        type_line(&mut r, "9999");
        assert_eq!(r.scroll, r.max_scroll());
    }

    #[test]
    fn code_files_go_to_the_source_line() {
        // Long lines wrap, so source and rendered lines differ.
        let long = "x".repeat(200);
        let src: String = (0..40)
            .map(|i| format!("let a{i} = \"{long}\";\n"))
            .collect();
        let doc = parse_for(&src, Some(Path::new("t.rs")));
        let mut r = Reader::new(Source::in_memory("t", doc), &Options::default());
        r.layout(60, 10);
        type_line(&mut r, "5");
        assert_eq!(r.rendered.src_line[r.scroll], Some(4));
        assert_eq!(r.source_line(), Some(5));
    }

    #[test]
    fn bad_input_says_so_and_esc_cancels() {
        let src: String = (0..50).map(|i| format!("para {i}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 10);
        type_line(&mut r, "abc");
        assert_eq!(r.message(), Some("not a line number: abc"));
        assert_eq!(r.scroll, 0);
        press(&mut r, KeyCode::Char(':'));
        press(&mut r, KeyCode::Char('9'));
        press(&mut r, KeyCode::Esc);
        assert!(r.goto().is_none());
        assert_eq!(r.scroll, 0);
    }
}
