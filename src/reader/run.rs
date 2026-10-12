use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::doc::{self, Block, CodeBlock};
use crate::exec::{BlockRun, RunEvent, RunSpec, Status};
use crate::render;

use super::Reader;

impl Reader {
    pub fn code(&self, id: usize) -> Option<&CodeBlock> {
        self.codes.iter().find(|c| c.id == id)
    }

    /// Position of the focused block among all code blocks, and their count.
    pub fn focus_position(&self) -> Option<(usize, usize)> {
        let id = self.focus?;
        let i = self.rendered.code_blocks.iter().position(|c| c.id == id)?;
        Some((i + 1, self.rendered.code_blocks.len()))
    }

    pub(super) fn move_focus(&mut self, forward: bool) {
        let ranges = &self.rendered.code_blocks;
        if ranges.is_empty() {
            self.message = Some("no code blocks".into());
            return;
        }
        let current = self
            .focus
            .and_then(|id| ranges.iter().position(|c| c.id == id));
        let next = match (current, forward) {
            (Some(i), true) => (i + 1).min(ranges.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
            (None, true) => ranges
                .iter()
                .position(|c| c.lines.start >= self.scroll)
                .unwrap_or(ranges.len() - 1),
            (None, false) => ranges
                .iter()
                .rposition(|c| c.lines.start < self.scroll + self.height)
                .unwrap_or(0),
        };
        let range = ranges[next].lines.clone();
        self.focus = Some(ranges[next].id);
        // Show the whole block when it fits, otherwise its top.
        if range.start < self.scroll || range.end > self.scroll + self.height {
            self.scroll = if range.len() <= self.height {
                range.start.saturating_sub(2)
            } else {
                range.start
            }
            .min(self.max_scroll());
        }
    }

    /// Directory block `id` runs in: the document's, joined with the
    /// block's `cwd` attribute if it has one.
    pub fn cwd(&self, id: usize) -> PathBuf {
        match self.code(id).and_then(|c| c.attrs.cwd.as_ref()) {
            Some(dir) => self.base_dir.join(dir),
            None => self.base_dir.clone(),
        }
    }

    /// How to run block `id`, or why it can't run.
    pub(super) fn spec(&self, id: usize) -> Result<RunSpec, String> {
        let c = self.code(id).ok_or("no such block")?;
        if c.attrs.norun {
            return Err("block is marked norun".into());
        }
        let lang = c.lang.as_deref().ok_or("block has no language")?;
        let mut cmd = self
            .interpreters
            .get(lang)
            .ok_or_else(|| format!("no interpreter for `{lang}`"))?;
        let program = cmd.remove(0);
        let mut args = cmd;
        args.push(c.src.clone());
        Ok(RunSpec {
            program,
            args,
            cwd: self.cwd(id),
            env: c.attrs.env.clone(),
        })
    }

    /// Ask to run `ids` in order, confirming first unless that's switched off.
    pub(super) fn request_run(&mut self, ids: Vec<usize>) {
        if ids.is_empty() {
            return;
        }
        if let Some(&id) = ids.iter().find(|&&id| self.runner.is_running(id)) {
            self.message = Some(format!("{} is already running", self.label(id)));
            return;
        }
        let must_ask = ids
            .iter()
            .any(|&id| self.code(id).is_some_and(|c| c.attrs.confirm));
        if self.assume_yes && !must_ask {
            self.start_queue(ids);
        } else {
            self.confirm = Some(ids);
        }
    }

    pub(super) fn start_queue(&mut self, ids: Vec<usize>) {
        self.follow = ids.first().copied();
        self.queue = ids.into();
        self.start_next();
    }

    pub(super) fn start_next(&mut self) {
        let Some(id) = self.queue.pop_front() else {
            return;
        };
        if self.follow.is_some() {
            self.follow = Some(id);
        }
        let mut run = BlockRun::started();
        match self.spec(id).and_then(|spec| {
            self.runner
                .start(id, &spec)
                .map_err(|e| format!("{}: {e}", spec.program))
        }) {
            Ok(()) => {}
            Err(e) => {
                run.status = Status::Error(e);
                self.queue.clear();
            }
        }
        self.runs.insert(id, run);
        self.dirty = true;
    }

    /// Display name of a block: its `name` attribute, or its position.
    pub fn label(&self, id: usize) -> String {
        match self.code(id).and_then(|c| c.attrs.name.clone()) {
            Some(name) => format!("`{name}`"),
            None => format!("block {}", id + 1),
        }
    }

    pub fn is_running(&self) -> bool {
        self.runs.values().any(BlockRun::is_running)
    }

    /// Redraw the elapsed time on running blocks' status lines in place.
    /// Only valid while `rendered` is current (not `dirty`).
    pub(super) fn refresh_timers(&mut self) {
        for range in &self.rendered.code_blocks {
            let Some(run) = self.runs.get(&range.id).filter(|r| r.is_running()) else {
                continue;
            };
            // The status line opens the run panel right under the block,
            // after any prefix spans from enclosing lists or quotes.
            if range.output_end <= range.lines.end {
                continue;
            }
            let status = render::run_status(run, &self.theme);
            let line = &mut self.rendered.lines[range.lines.end];
            line.spans
                .truncate(line.spans.len().saturating_sub(status.spans.len()));
            line.spans.extend(status.spans);
        }
    }

    pub(super) fn on_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                if let Some(ids) = self.confirm.take() {
                    self.start_queue(ids);
                }
            }
            KeyCode::Char('n' | 'N' | 'q') | KeyCode::Esc => self.confirm = None,
            _ => {}
        }
    }

    pub(super) fn run_focused(&mut self, all_from_here: bool) {
        if self.focus.is_none() {
            self.move_focus(true);
        }
        let Some(focus) = self.focus else { return };
        if !all_from_here {
            match self.spec(focus) {
                Ok(_) => self.request_run(vec![focus]),
                Err(e) => self.message = Some(e),
            }
            return;
        }
        let ids: Vec<usize> = self
            .rendered
            .code_blocks
            .iter()
            .map(|c| c.id)
            .skip_while(|&id| id != focus)
            .filter(|&id| self.spec(id).is_ok())
            .collect();
        if ids.is_empty() {
            self.message = Some("no runnable blocks from here".into());
        } else {
            self.request_run(ids);
        }
    }

    pub(super) fn cancel(&mut self) {
        self.queue.clear();
        let ids: Vec<usize> = match self.focus {
            Some(id) if self.runner.is_running(id) => vec![id],
            _ => self
                .runs
                .iter()
                .filter(|(_, r)| r.is_running())
                .map(|(&id, _)| id)
                .collect(),
        };
        for id in ids {
            self.runner.cancel(id);
        }
    }

    /// Record a running block's output or exit; an exit starts the next
    /// queued block, or ends the queue when the block failed.
    pub(super) fn on_run_event(&mut self, event: RunEvent) {
        match event {
            RunEvent::Output { id, text, stderr } => {
                if let Some(run) = self.runs.get_mut(&id) {
                    run.push(text, stderr);
                }
            }
            RunEvent::Exit {
                id,
                code,
                cancelled,
                took,
            } => {
                let status = if self.timed_out.remove(&id) {
                    Status::TimedOut(took)
                } else if cancelled {
                    Status::Cancelled(took)
                } else if code == Some(0) {
                    Status::Ok(took)
                } else {
                    Status::Failed { code, took }
                };
                let ok = status == Status::Ok(took);
                if let Some(run) = self.runs.get_mut(&id) {
                    run.status = status;
                }
                if ok {
                    self.start_next();
                } else {
                    self.queue.clear();
                }
            }
        }
    }

    /// Stop running blocks that are past their `timeout`.
    pub(super) fn cancel_expired(&mut self) {
        let expired: Vec<usize> = self
            .runs
            .iter()
            .filter_map(|(&id, run)| {
                let Status::Running(since) = run.status else {
                    return None;
                };
                let limit = self.code(id)?.attrs.timeout?;
                (since.elapsed() > limit && !self.timed_out.contains(&id)).then_some(id)
            })
            .collect();
        for id in expired {
            self.timed_out.insert(id);
            self.runner.cancel(id);
        }
    }
}

pub(super) fn collect_code(blocks: &[Block]) -> Vec<CodeBlock> {
    let mut out = Vec::new();
    doc::visit(blocks, &mut |b| {
        if let Block::Code(c) = b {
            out.push(c.clone());
        }
    });
    out
}
