#!/bin/sh
# Fail when the local rustc is older than the latest stable release, which
# CI (dtolnay/rust-toolchain@stable) uses: an older clippy misses lints that
# CI then reports, as it did for v0.4.1. Offline, it warns and passes.
# Set SKIP_TOOLCHAIN_CHECK=1 to skip it.
set -eu

[ "${SKIP_TOOLCHAIN_CHECK:-}" = 1 ] && exit 0

LOCAL=$(rustc -V | cut -d' ' -f2)
STABLE=$(curl -fsS --max-time 5 https://api.github.com/repos/rust-lang/rust/releases/latest 2>/dev/null |
    sed -n 's/.*"tag_name": *"\([0-9.]*\)".*/\1/p' || true)
if [ -z "$STABLE" ]; then
    echo "toolchain-check: couldn't fetch the latest stable Rust; rustc $LOCAL may lag CI." >&2
    exit 0
fi

# sort -V puts the older version first.
OLDEST=$(printf '%s\n%s\n' "$LOCAL" "$STABLE" | sort -V | head -n1)
if [ "$LOCAL" != "$STABLE" ] && [ "$OLDEST" = "$LOCAL" ]; then
    echo "toolchain-check: rustc $LOCAL is older than stable $STABLE, which CI uses;" >&2
    echo "its clippy can miss lints CI fails on. Update (rustup update, or brew upgrade rust)." >&2
    exit 1
fi
