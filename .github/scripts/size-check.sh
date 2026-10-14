#!/bin/sh
# Fail when a binary is larger than the size budget.
# Usage: size-check.sh <binary> <budget file>
set -eu

BIN=$1
# Exactly one line besides comments and blanks: the byte count.
ENTRIES=$(grep -v '^[[:space:]]*#' "$2" | grep -v '^[[:space:]]*$' || true)
if [ "$(printf '%s\n' "$ENTRIES" | grep -c .)" -ne 1 ]; then
    echo "$2 must hold exactly one byte count" >&2; exit 1
fi
BUDGET=$(printf '%s' "$ENTRIES" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')
case $BUDGET in
    '' | *[!0-9]*) echo "no byte count in $2" >&2; exit 1 ;;
esac
SIZE=$(wc -c < "$BIN" | tr -d '[:space:]')

mb() { awk -v b="$1" 'BEGIN { printf "%.2f MB", b / 1000000 }'; }
REPORT="$(basename "$BIN") is $(mb "$SIZE") ($SIZE bytes), budget $(mb "$BUDGET") ($BUDGET bytes)"

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    echo "Binary size: $REPORT" >> "$GITHUB_STEP_SUMMARY"
fi
if [ "$SIZE" -gt "$BUDGET" ]; then
    echo "$REPORT: over by $(mb $((SIZE - BUDGET)))." >&2
    echo "Shrink it (cargo bloat --release --crates shows what's big) or raise $2." >&2
    exit 1
fi
echo "$REPORT"
