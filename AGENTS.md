# bastian

Terminal Markdown reader. `CONTEXT.md` is the glossary: use its terms (Document, Source, Reader, Overlay, Focus, ToC, Run, Position, Entry) in code, docs and discussion, and add a term there when a new concept appears.

## Shape

- Binary crate. The `lib` target exists so `tests/` can reach the modules; nothing outside this repo depends on it. New items are `pub(crate)` unless an integration test needs them.
- Two Markdown front ends stay: pulldown-cmark for `.md`, markdown-rs for `.mdx`. markdown-rs measured 15x slower and super-linear on deep nesting, so it never parses `.md`. Both must build the same `Block` tree for the same text: a syntax feature lands in both, sharing what it can from `doc/mod.rs` (`Ids`, `Footnotes`, `wiki_target`), with a test in `doc/mdx/tests.rs` that compares the MDX result against the Markdown front end's.
- One thing owns the keyboard at a time: an open Overlay. Esc returns to plain reading in one step.

## Tests

- Tests live beside the code. When a module's tests outgrow the code they test, they move to a sibling `tests.rs` (`config/tests.rs` is the model).
- `tests/` holds the interface tests: they drive `Reader`, `Browser` and the binary the way callers do and must survive internal refactors.
- Real stand-ins over mocks: real `sh`, a local `TcpListener`, tempdirs. The clock is the one injected dependency, so no test sleeps to wait for a timer.

## Checks

`just check` is what CI runs. Local Rust may lag CI's stable; a lint CI reports that `just check` did not is still a failure to fix.
