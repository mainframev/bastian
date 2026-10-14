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

# Run the test suite.
test:
    cargo test

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

# Re-record the README's GIFs with this checkout's build (needs vhs).
demo:
    cargo build --release
    PATH="$PWD/target/release:$PATH" vhs demo/usage.tape
    PATH="$PWD/target/release:$PATH" vhs demo/themes.tape
    PATH="$PWD/target/release:$PATH" vhs demo/reader-themes.tape
