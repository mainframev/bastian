use std::time::Duration;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::theme::Theme;
use super::wrap::hard_wrap;
use super::{QUOTE_BAR, Renderer};
use crate::exec::{BlockRun, Status};

impl Renderer<'_> {
    /// Status line and output shown under a code block that has run.
    pub(super) fn run_panel(&self, run: &BlockRun, width: usize) -> Vec<Line<'static>> {
        let t = self.theme;
        let mut lines = vec![run_status(run, t)];
        if !run.expanded {
            return lines;
        }

        let bar = Span::styled(QUOTE_BAR, Style::default().fg(t.muted));
        if run.dropped > 0 {
            lines.push(Line::from(vec![
                bar.clone(),
                Span::styled(
                    format!("… {} earlier lines", run.dropped),
                    Style::default().fg(t.muted),
                ),
            ]));
        }
        for out in &run.output {
            let style = Style::default().fg(if out.stderr { t.err } else { t.subtle });
            for row in hard_wrap(
                vec![Span::styled(out.text.clone(), style)],
                width.saturating_sub(2),
            ) {
                let mut spans = vec![bar.clone()];
                spans.extend(row);
                lines.push(Line::from(spans));
            }
        }
        lines
    }
}

/// First line of a run panel: status icon, text and elapsed time, plus a
/// hidden-line count when the output is collapsed.
pub fn run_status(run: &BlockRun, t: &Theme) -> Line<'static> {
    let (icon, text, color) = match &run.status {
        Status::Running(since) => ("●", format!("running {}", secs(since.elapsed())), t.warn),
        Status::Ok(took) => ("✓", format!("done in {}", secs(*took)), t.ok),
        Status::Failed {
            code: Some(c),
            took,
        } => ("✗", format!("exit {c} after {}", secs(*took)), t.err),
        Status::Failed { code: None, took } => (
            "✗",
            format!("killed by signal after {}", secs(*took)),
            t.err,
        ),
        Status::Cancelled(took) => ("■", format!("cancelled after {}", secs(*took)), t.warn),
        Status::TimedOut(took) => ("✗", format!("timed out after {}", secs(*took)), t.err),
        Status::Error(e) => ("✗", format!("could not start: {e}"), t.err),
    };
    let mut status = vec![Span::styled(
        format!("{icon} {text}"),
        Style::default().fg(color).bold(),
    )];
    if !run.expanded && !run.output.is_empty() {
        let n = run.output.len() + run.dropped;
        status.push(Span::styled(
            format!("  {n} line{} hidden", if n == 1 { "" } else { "s" }),
            Style::default().fg(t.muted),
        ));
    }
    Line::from(status)
}

pub(super) fn secs(d: Duration) -> String {
    format!("{:.1}s", d.as_secs_f64())
}
