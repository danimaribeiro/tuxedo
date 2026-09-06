#!/usr/bin/env bash
# Compares tuxedo's formatter output against rubocop's Layout-cop autocorrect
# output for every .rb file in a test project, and reports which files differ.
#
# Usage:
#   scripts/format-compare.sh <project>
#
# Expects the project's pristine source at test-projects/<project>/source
# (see README in test-projects/ for how to add one). Writes two working
# copies alongside it:
#   test-projects/<project>/tuxedo   - source formatted by our formatter
#   test-projects/<project>/rubocop  - source autocorrected by rubocop (Layout only)
#
# rubocop runs with `-c scripts/format-compare.rubocop.yml`, ignoring the
# project's own .rubocop.yml. tuxedo has no per-project config for
# individual style choices yet, so the only fair baseline is rubocop's own
# defaults — a project's local overrides (e.g. Thor's
# AllCops: DisabledByDefault: true, which silently turns off most Layout
# cops rather than customizing them) would otherwise produce a misleading
# reference full of un-corrected, never-checked code. That comparison
# config also disables AllowForAlignment on the few cops that default it to
# true — see the comment in that file for why.
#
# Follow up on a mismatch with:
#   scripts/format-diff.sh <project> <relative/path.rb>
#
# Report saved to scripts/results/<project>-format-compare.txt (committed -
# survives across sessions instead of needing a rerun to know where things
# stand; commit the update alongside whatever prompted the run).

set -euo pipefail

PROJECT="${1:-}"
if [ -z "$PROJECT" ]; then
  echo "Usage: scripts/format-compare.sh <project>" >&2
  echo "  (expects test-projects/<project>/source to exist)" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE="$ROOT/test-projects/$PROJECT"
SRC="$BASE/source"
RESULTS_DIR="$ROOT/scripts/results"
REPORT="$RESULTS_DIR/$PROJECT-format-compare.txt"

if [ ! -d "$SRC" ]; then
  echo "error: $SRC does not exist." >&2
  echo "Clone the project's source into test-projects/$PROJECT/source first." >&2
  exit 1
fi

if ! command -v rubocop >/dev/null 2>&1; then
  echo "error: rubocop not found on PATH (gem install rubocop)." >&2
  exit 1
fi

mkdir -p "$RESULTS_DIR"

echo "Building tuxedo (release)..."
(cd "$ROOT" && cargo build --release --quiet)
TUXEDO_BIN="$ROOT/target/release/tuxedo"

echo "Preparing working copies..."
rm -rf "$BASE/tuxedo" "$BASE/rubocop"
cp -r "$SRC" "$BASE/tuxedo"
cp -r "$SRC" "$BASE/rubocop"

TIMEFORMAT='%R'

echo "Running tuxedo format..."
tuxedo_time=$( { time "$TUXEDO_BIN" format --quiet "$BASE/tuxedo" >/dev/null; } 2>&1 )

echo "Running rubocop -A (Layout cops only, default config)..."
rubocop_log="$(mktemp)"
rubocop_time=$( { time (cd "$BASE/rubocop" && BUNDLE_GEMFILE=/dev/null rubocop --only Layout -c "$ROOT/scripts/format-compare.rubocop.yml" -A --no-color . >"$rubocop_log" 2>&1 || true); } 2>&1 )
if grep -q "Could not find proper version of rubocop" "$rubocop_log"; then
  echo "warning: rubocop refused to run because $PROJECT pins its own rubocop version" \
    "in Gemfile.lock (worked around with BUNDLE_GEMFILE=/dev/null, but double-check" \
    "the output below if this project still looks off)." >&2
  tail -5 "$rubocop_log" >&2
fi
rm -f "$rubocop_log"

echo
echo "Comparing output..."
{
  echo "format-compare: $PROJECT"
  echo "generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo

  mismatches=0
  total=0
  while IFS= read -r -d '' file; do
    rel="${file#"$SRC"/}"
    total=$((total + 1))
    if ! diff -q "$BASE/tuxedo/$rel" "$BASE/rubocop/$rel" >/dev/null 2>&1; then
      echo "MISMATCH: $rel"
      mismatches=$((mismatches + 1))
    fi
  done < <(find "$SRC" -name '*.rb' -print0 | sort -z)

  echo
  echo "$mismatches / $total files differ between tuxedo and rubocop output."
  echo
  echo "tuxedo:  ${tuxedo_time}s"
  echo "rubocop: ${rubocop_time}s"
  speedup=$(awk -v a="$rubocop_time" -v b="$tuxedo_time" 'BEGIN { if (b > 0) printf "%.1f", a / b }')
  if [ -n "$speedup" ]; then
    echo "tuxedo is ${speedup}x faster"
  fi
  if [ "$mismatches" -gt 0 ]; then
    echo
    echo "To see a diff for one file:"
    echo "  scripts/format-diff.sh $PROJECT <relative/path.rb>"
  fi
} | tee "$REPORT"

echo
echo "Report saved to scripts/results/$PROJECT-format-compare.txt"
