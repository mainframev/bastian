use ratatui::Frame;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super::status;
use crate::picker::ThemePicker;
use crate::render::theme::{Appearance, Theme};

/// The theme list, docked to the right over the current screen, which
/// shows the highlighted theme as a live preview.
pub fn draw(frame: &mut Frame, t: &Theme, p: &ThemePicker) {
    // Leave room for the border and the status bar.
    let rows = frame.area().height.saturating_sub(4).max(1) as usize;
    let start = status::window(p.selected, p.entries.len(), rows, true);
    let name_w = p.entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
    let lines: Vec<Line> = p
        .entries
        .iter()
        .enumerate()
        .skip(start)
        .take(rows)
        .map(|(i, e)| {
            let selected = i == p.selected;
            let mut spans = vec![
                status::select_marker(t, selected),
                Span::styled(
                    format!("{:<name_w$}  ", e.name),
                    status::row_style(t, selected),
                ),
            ];
            match &e.theme {
                Ok(theme) => {
                    let kind = match theme.appearance {
                        Appearance::Dark => "dark ",
                        Appearance::Light => "light",
                    };
                    spans.push(Span::styled(kind, Style::default().fg(t.muted)));
                    spans.push(Span::raw("  "));
                    spans.extend(swatch(theme));
                }
                Err(_) => spans.push(Span::styled("broken", Style::default().fg(t.err))),
            }
            if p.current == Some(i) {
                spans.push(Span::styled("  current", Style::default().fg(t.subtle)));
            }
            Line::from(spans)
        })
        .collect();
    let title = format!(
        " themes {}/{} · enter use · esc cancel ",
        p.selected + 1,
        p.entries.len()
    );
    status::side_panel(frame, t, &title, lines);
    if let Err(e) = &p.entries[p.selected].theme {
        let area = frame.area();
        let y = area.bottom().saturating_sub(2);
        frame.render_widget(
            Line::styled(format!(" {e}"), Style::default().fg(t.err)),
            ratatui::layout::Rect::new(area.x, y, area.width, 1),
        );
    }
}

/// A few of the theme's colors side by side.
fn swatch(theme: &Theme) -> Vec<Span<'static>> {
    [
        theme.accent,
        theme.heading,
        theme.link,
        theme.code,
        theme.ok,
        theme.warn,
        theme.err,
    ]
    .into_iter()
    .map(|c: Color| Span::styled("██", Style::default().fg(c)))
    .collect()
}
