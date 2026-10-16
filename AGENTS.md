# bastian

Terminal Markdown reader (Rust, edition 2024, ratatui + crossterm). `CONTEXT.md` is the glossary: use its terms (Document, Source, Reader, Overlay, Focus, ToC, Run, Position, Entry) in code, docs and discussion, and add a term there when a new concept appears.

## Commands

`just` lists the recipes; each is a thin wrapper over cargo, so run the cargo lines from `justfile` when `just` isn't installed.

- `just check`: lint then test, what CI runs. Run it before pushing: CI doesn't run on pull requests.
- `just lint`: `cargo fmt --check`, then `cargo clippy --all-targets -- -D warnings` with and without `--features mermaid`.
- `just test`: `cargo test` with and without `--features mermaid`.
- One test: `cargo test --lib <name>` for unit tests, `cargo test --test reader <name>` for an interface test file.
- `just fmt`: format. `just bench`: divan render benches (not in CI). `just size`: release build against `.github/size-budget`.
- `cargo run -- <file.md>` reads a file; `cargo run -- -` reads stdin.
- `just install-hooks` enables `.githooks/pre-commit` (fmt and clippy).

## Layout

- `src/main.rs`: entry point; `src/cli.rs`: clap arguments.
- `src/doc/`: parsing into `Document` (`parse.rs` for `.md`, `mdx.rs` for `.mdx`, shared post-processing in `mod.rs`).
- `src/render/`: `Document` to styled lines for a width: wrapping, tables, code, images, diagrams, ANSI and plain output.
- `src/reader/`: the `Reader` state and its keys, one file per concern (`toc.rs`, `search.rs`, `goto.rs`, `history.rs`, ...).
- `src/browser/`: the file list (`Entry`, scanning). `src/app/`: the event loop tying `Reader` and `Browser` to the terminal.
- `src/ui/`: drawing only, with the key help tables (`HELP` in `ui/reader/mod.rs` and `ui/browser/mod.rs`).
- `src/config/`, `src/themes/`: settings file and themes. `src/exec/`: running code blocks.
- `docs/*.md`: user docs, linked from `README.md`.

## Shape

- Binary crate. The `lib` target exists so `tests/` can reach the modules; nothing outside this repo depends on it. New items are `pub(crate)` unless an integration test needs them.
- Two Markdown front ends stay: pulldown-cmark for `.md`, markdown-rs for `.mdx`. markdown-rs measured 15x slower and super-linear on deep nesting, so it never parses `.md`. Both must build the same `Block` tree for the same text: a syntax feature lands in both, sharing what it can from `doc/mod.rs` (`Ids`, `Footnotes`, `wiki_target`), with a test in `doc/mdx/tests.rs` that compares the MDX result against the Markdown front end's.
- One thing owns the keyboard at a time: an open Overlay. Esc returns to plain reading in one step.
- `mermaid` is an opt-in feature (merman adds about 8 MB). Code behind it must build and pass clippy both with and without it.

## Changing behavior

- A new or changed key goes in three places: the handler in `src/reader/keys.rs` (or `browser/keys.rs`), the `HELP` table in `src/ui/`, and the key table in `docs/reader.md` (or `docs/file-list.md`).
- A change users see updates the matching page in `docs/`.

## Tests

- Tests live beside the code. When a module's tests outgrow the code they test, they move to a sibling `tests.rs` (`config/tests.rs` is the model).
- `tests/` holds the interface tests: they drive `Reader`, `Browser` and the binary the way callers do and must survive internal refactors.
- Real stand-ins over mocks: real `sh`, a local `TcpListener`, tempdirs. The clock is the one injected dependency, so no test sleeps to wait for a timer.
- A bug fix comes with a regression test that fails without the fix.
- `tests/render.rs` snapshots `tests/fixtures/*.md` at several widths with insta. After an intended rendering change, accept new snapshots with `INSTA_UPDATE=always cargo test --test render` (or `cargo insta review`), and read the `.snap` diff before committing it.

## Checks

`just check` is what CI runs. CI (`.github/workflows/ci.yml`) doesn't run on pull requests or pushes, to save Actions minutes: Release runs it before building, and it can be started by hand (Actions, CI, Run workflow). So `just check` before pushing is the gate for a PR. Local Rust may lag CI's stable; a lint CI reports that `just check` did not is still a failure to fix.

CI also runs, with `RUSTFLAGS=-D warnings`:

- Conventional Commits: cocogitto checks every commit message on the branch.
- Binary size: the Linux release binary must fit `.github/size-budget`.

Clippy runs `pedantic` with the exceptions listed in `Cargo.toml`; fix a lint rather than allowing it locally, unless the reason is stated beside the `allow`.

## Commits and pull requests

- Conventional Commits with a scope naming the module: `feat(reader): ...`, `fix(render): ...`, `refactor(nav): ...`, `docs: ...`. The subject is lower case and imperative; the body says why. git-cliff builds the changelog from these, so the subject is what users read.
- One logical change per commit and one concern per PR.
- Every review thread is resolved before merging.

## Release

Run by workflows, not by hand: Prepare Release opens a version-bump PR with the changelog, merging it creates the tag, and Release builds the archives, publishes to crates.io and creates the GitHub release. Don't edit `CHANGELOG.md` or the version in `Cargo.toml` in feature PRs.
