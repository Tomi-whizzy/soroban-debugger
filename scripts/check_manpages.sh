#!/usr/bin/env bash
# Regenerates man pages into a temp directory and diffs against committed versions.
# Exits non-zero with a clear diff output if drift is detected.
#
# Usage: bash scripts/check_manpages.sh
#   or:  make check-man
#
# Portability notes:
#   - Uses `mktemp -d` with explicit template for BSD/macOS compatibility.
#   - Honours TMPDIR so CI and sandbox environments can control the temp root.
#   - Falls back gracefully when TMPDIR is unset.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMMITTED_DIR="$REPO_ROOT/man/man1"

# Use TMPDIR if set, otherwise /tmp. This allows CI/sandbox systems to
# control the temp root without relying on platform-specific mktemp defaults.
TMPDIR="${TMPDIR:-/tmp}"
export TMPDIR

# Create temp dir with explicit template for portability (BSD mktemp requires
# the template to contain at least 3 trailing Xs).
TEMP_DIR=$(mktemp -d "${TMPDIR}/check_manpages.XXXXXXXX")

# Always clean up temp dir, even on failure or Ctrl-C
trap 'rm -rf "$TEMP_DIR"' EXIT

TEMP_MAN_DIR="$TEMP_DIR/man/man1"

echo "Generating fresh man pages into $TEMP_DIR..."

# MAN_OUT_DIR is read by build.rs to redirect man page output to a temp directory.
# This avoids touching the committed man/man1/ during the diff check.
# The build script (which generates man pages) runs before the main crate is compiled,
# so we tolerate main-crate compilation failures with || true — man pages are still produced.
MAN_OUT_DIR="$TEMP_MAN_DIR" cargo build --quiet 2>/dev/null || true

if [ ! -d "$TEMP_MAN_DIR" ]; then
    echo "ERROR: Man page generation failed: $TEMP_MAN_DIR was not created."
    exit 2
fi

if [ ! -d "$COMMITTED_DIR" ]; then
    echo "ERROR: Committed man page directory not found: $COMMITTED_DIR"
    echo "   Run 'make regen-man' to generate and commit man pages."
    exit 1
fi

echo "Diffing against committed man pages in $COMMITTED_DIR..."

diff_output=$(diff -r "$COMMITTED_DIR" "$TEMP_MAN_DIR" 2>&1) || diff_status=$?
diff_status=${diff_status:-0}

if [ "$diff_status" -eq 0 ]; then
    echo "OK: Man pages are in sync."
    exit 0
elif [ "$diff_status" -eq 1 ]; then
    echo ""
    echo "ERROR: Drift detected between committed man pages and current CLI source."
    echo "   Run 'make regen-man' and commit the updated .1 files."
    echo ""
    echo "--- diff output ---"
    echo "$diff_output"
    exit 1
else
    echo "ERROR: diff exited with error (status $diff_status)."
    echo "$diff_output"
    exit 2
fi
