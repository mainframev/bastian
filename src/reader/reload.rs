use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

use super::{Overlay, Reader, collect_code};
use crate::doc::{CodeBlock, Document};

impl Reader {
    /// Re-read the source file if its modification time moved.
    pub(super) fn check_reload(&mut self) -> bool {
        if self.last_check.elapsed() < RELOAD_CHECK {
            return false;
        }
        self.last_check = Instant::now();
        let Some(path) = &self.source else {
            return false;
        };
        let modified = mtime(path);
        if modified == self.modified {
            return false;
        }
        self.modified = modified;
        match crate::open::open(path) {
            Ok(opened) => {
                self.replace_doc(opened.doc);
                self.text = opened.text;
                self.message = Some("reloaded".into());
            }
            Err(e) => self.message = Some(format!("reload failed: {e}")),
        }
        true
    }

    /// Swap in a new version of the document, keeping scroll and any run
    /// state whose block still exists.
    pub fn replace_doc(&mut self, doc: Document) {
        let codes = collect_code(&doc.blocks);
        let moved = match_blocks(&self.codes, &codes);
        // A pending prompt or queue names blocks by index. Once the blocks
        // change, those indexes point at code the user never agreed to run.
        let same = codes.len() == self.codes.len()
            && moved.iter().enumerate().all(|(i, &to)| to == Some(i));
        if !same {
            self.queue.clear();
        }
        // The link may be gone or point elsewhere now, and a changed block
        // list makes a pending run confirmation name the wrong blocks.
        let stale = match &self.overlay {
            Some(Overlay::OpenConfirm(_)) => true,
            Some(Overlay::RunConfirm(_)) => !same,
            _ => false,
        };
        if stale {
            self.overlay = None;
        }
        // Run state belongs to code, not to a position: follow each block to
        // its new place, and drop what ran code that's gone or changed.
        let mut runs = HashMap::new();
        for (id, run) in std::mem::take(&mut self.runs) {
            let to = moved.get(id).copied().flatten();
            if run.is_running() && to != Some(id) {
                // The process reports under its old id, which may now be
                // another block's: stop it and ignore what it still sends.
                self.runner.cancel(id);
                self.detached.insert(id);
            } else if let Some(to) = to {
                runs.insert(to, run);
            }
        }
        self.runs = runs;
        self.follow = self.follow.and_then(|id| moved.get(id).copied().flatten());
        // The focused link may now be a different one; Enter mustn't follow
        // a link the preview didn't show.
        self.link_focus = None;
        // Details are numbered by position, so after an edit the numbers may
        // name other blocks: forget which were opened and which was focused.
        self.details_focus = None;
        self.toggled_details.clear();
        // Diagrams switched to their source follow their block.
        self.as_source = std::mem::take(&mut self.as_source)
            .into_iter()
            .filter_map(|id| moved.get(id).copied().flatten())
            .collect();
        self.focus = self
            .focus
            .and_then(|id| moved.get(id).copied().flatten().or(Some(id)))
            .filter(|&id| id < codes.len());
        self.codes = codes;
        self.doc = doc;
        self.highlights.clear();
        self.images_requested = false;
        self.dirty = true;
    }
}

/// Where each of the `old` blocks went in `new`: the index of a block with
/// the same code, preferring the same position, or `None` when it's gone.
fn match_blocks(old: &[CodeBlock], new: &[CodeBlock]) -> Vec<Option<usize>> {
    let same =
        |a: &CodeBlock, b: &CodeBlock| a.lang == b.lang && a.attrs == b.attrs && a.src == b.src;
    let mut taken = vec![false; new.len()];
    let mut out = vec![None; old.len()];
    // Blocks that stayed put first, so a moved copy can't take their place.
    for (i, a) in old.iter().enumerate() {
        if new.get(i).is_some_and(|b| same(a, b)) {
            taken[i] = true;
            out[i] = Some(i);
        }
    }
    for (i, a) in old.iter().enumerate() {
        if out[i].is_none()
            && let Some(j) = (0..new.len()).find(|&j| !taken[j] && same(a, &new[j]))
        {
            taken[j] = true;
            out[i] = Some(j);
        }
    }
    out
}

/// How often to check the source file for changes.
const RELOAD_CHECK: Duration = Duration::from_millis(500);

pub(super) fn mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::doc::parse;
    use crate::exec::Status;
    use crate::reader::{Options, Reader, Source};

    fn reader(src: &str) -> Reader {
        let opts = Options {
            assume_yes: true,
            ..Options::default()
        };
        let mut r = Reader::new(Source::in_memory("t", parse(src)), &opts);
        r.layout(80, 40);
        r
    }

    #[test]
    fn diagrams_shown_as_source_follow_their_block() {
        let diagram = "```mermaid\nflowchart TD\n  A --> B\n```\n";
        let mut r = reader(&format!("{diagram}\n```sh\necho a\n```\n"));
        r.as_source.insert(0);
        r.replace_doc(parse(&format!("```sh\necho new\n```\n\n{diagram}")));
        assert_eq!(r.as_source.iter().collect::<Vec<_>>(), [&1]);
        r.replace_doc(parse("```sh\necho new\n```\n"));
        assert!(r.as_source.is_empty(), "the diagram is gone");
    }

    fn settle(r: &mut Reader) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while r.is_running() || r.runner.is_running(0) || r.runner.is_running(1) {
            assert!(Instant::now() < deadline, "blocks did not finish");
            r.tick();
            std::thread::sleep(Duration::from_millis(10));
        }
        r.tick();
    }

    fn output(r: &Reader, id: usize) -> Vec<&str> {
        r.runs[&id].output.iter().map(|l| l.text.as_str()).collect()
    }

    #[test]
    fn output_follows_its_block_when_blocks_move() {
        let mut r = reader("```sh\necho one\n```\n");
        r.request_run(vec![0]);
        settle(&mut r);
        r.replace_doc(parse("```sh\necho new\n```\n\n```sh\necho one\n```\n"));
        assert!(!r.runs.contains_key(&0), "new code shows old output");
        assert_eq!(output(&r, 1), ["one"]);
    }

    #[test]
    fn changed_code_drops_its_output() {
        let mut r = reader("```sh\necho one\n```\n");
        r.request_run(vec![0]);
        settle(&mut r);
        r.replace_doc(parse("```sh\necho two\n```\n"));
        assert!(r.runs.is_empty());
    }

    #[test]
    fn a_moved_running_block_stops_without_touching_other_output() {
        let mut r = reader("```sh\necho a\n```\n\n```sh\nsleep 5; echo late\n```\n");
        r.request_run(vec![0]);
        settle(&mut r);
        r.request_run(vec![1]);
        r.tick();
        assert!(r.runner.is_running(1));
        // Swap the blocks: `echo a` is now block 1, where the old process
        // still reports from.
        r.replace_doc(parse(
            "```sh\nsleep 5; echo late\n```\n\n```sh\necho a\n```\n",
        ));
        settle(&mut r);
        assert_eq!(output(&r, 1), ["a"]);
        assert!(matches!(r.runs[&1].status, Status::Ok(_)));
        assert!(!r.runs.contains_key(&0));
    }
}
