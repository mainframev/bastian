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

# Lint and test, as CI does.
check: lint test

# Run `lint` before every commit (opt-in, per clone).
install-hooks:
    git config core.hooksPath .githooks

# Re-record the README's GIFs with this checkout's build (needs vhs).
demo:
    cargo build --release
    PATH="$PWD/target/release:$PATH" vhs demo/themes.tape
    PATH="$PWD/target/release:$PATH" vhs demo/reader-themes.tape
