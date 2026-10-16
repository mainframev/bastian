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

    /// Scroll line `n` (1-based, as the line numbers show it) to the top
    /// and mark it: a source line in code files, with every row it wraps
    /// onto, a rendered line otherwise. Past the end goes to the last.
    pub(super) fn go_to_line(&mut self, n: usize) {
        let want = n.saturating_sub(1);
        let lines = &self.rendered.src_line;
        let last = self.rendered.lines.len().saturating_sub(1);
        let rows = if lines.is_empty() {
            let line = want.min(last);
            line..line + 1
        } else {
            let start = lines
                .iter()
                .position(|l| l.is_some_and(|s| s >= want))
                .or_else(|| lines.iter().rposition(Option::is_some))
                .unwrap_or(last);
            // Rows it wraps onto carry no line number of their own; a run
            // panel's rows after the code don't either.
            let code_end = self
                .rendered
                .code_blocks
                .first()
                .map_or(lines.len(), |c| c.lines.end);
            let more = lines[start + 1..code_end.max(start + 1)]
                .iter()
                .take_while(|l| l.is_none())
                .count();
            start..start + 1 + more
        };
        self.scroll_to(rows.start);
        if !self.rendered.lines.is_empty() {
            self.goto_mark = Some(rows);
        }
    }

    /// Rendered rows of the line last gone to, while it's marked.
    pub fn goto_mark(&self) -> Option<std::ops::Range<usize>> {
        self.goto_mark.clone()
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
    fn g_and_shift_g_go_to_the_ends_and_ctrl_g_asks_for_a_line() {
        let src: String = (0..50).map(|i| format!("para {i}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 10);
        press(&mut r, KeyCode::Char('G'));
        assert_eq!(r.scroll, r.max_scroll());
        assert!(r.goto().is_none());
        press(&mut r, KeyCode::Char('g'));
        assert_eq!(r.scroll, 0);
        assert!(r.goto().is_none());
        r.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        assert!(r.goto().is_some());
        for c in "21".chars() {
            press(&mut r, KeyCode::Char(c));
        }
        press(&mut r, KeyCode::Enter);
        assert_eq!(r.scroll, 20);
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

    #[test]
    fn the_line_gone_to_is_marked_until_the_next_key() {
        let src: String = (0..50).map(|i| format!("para {i}\n\n")).collect();
        let mut r = Reader::new(Source::in_memory("t", parse(&src)), &Options::default());
        r.layout(80, 10);
        // Near the end, where the line can't reach the top.
        let last = r.rendered.lines.len();
        type_line(&mut r, &(last - 2).to_string());
        assert_eq!(r.scroll, r.max_scroll());
        assert_eq!(r.goto_mark(), Some(last - 3..last - 2));
        press(&mut r, KeyCode::Char('k'));
        assert_eq!(r.goto_mark(), None);
        // Past the end marks the last line.
        type_line(&mut r, "9999");
        assert_eq!(r.goto_mark(), Some(last - 1..last));
        // Any key clears it, even one that doesn't move the view.
        press(&mut r, KeyCode::Char('j'));
        assert_eq!(r.goto_mark(), None);
    }

    #[test]
    fn a_wrapped_source_line_is_marked_on_every_row() {
        let long = "x".repeat(200);
        let src: String = (0..40)
            .map(|i| format!("let a{i} = \"{long}\";\n"))
            .collect();
        let doc = parse_for(&src, Some(Path::new("t.rs")));
        let mut r = Reader::new(Source::in_memory("t", doc), &Options::default());
        r.layout(60, 10);
        type_line(&mut r, "5");
        let rows = r.goto_mark().expect("marked");
        assert!(rows.len() > 1, "{rows:?}");
        assert_eq!(r.rendered.src_line[rows.start], Some(4));
        assert!(
            rows.clone()
                .skip(1)
                .all(|l| r.rendered.src_line[l].is_none())
        );
        assert_eq!(r.rendered.src_line[rows.end], Some(5));
    }
}
