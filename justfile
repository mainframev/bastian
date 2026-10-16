# Development tasks. `just` lists them; `just check` runs what CI runs.

# Show the available recipes.
default:
    @just --list

# Format the code.
fmt:
    cargo fmt

# Check formatting and clippy lints (warnings fail).
lint:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo clippy --all-targets --features mermaid -- -D warnings

# Run the test suite, with and without the mermaid feature.
test:
    cargo test
    cargo test --features mermaid

# Time rendering of generated documents (not run in CI).
bench:
    cargo bench

# Check the release binary against the size budget. CI checks x86_64
# Linux; binaries for other platforms differ in size.
size:
    cargo build --release --locked
    .github/scripts/size-check.sh target/release/bastian .github/size-budget

# Lint and test, as CI does.
check: lint test

# Run `lint` before every commit (opt-in, per clone).
install-hooks:
    git config core.hooksPath .githooks
