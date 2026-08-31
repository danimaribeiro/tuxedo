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
# rubocop runs with --force-default-config, ignoring the project's own
# .rubocop.yml. tuxedo has no per-project config for individual style
# choices yet, so the only fair baseline is rubocop's own defaults — a
# project's local overrides (e.g. Thor's AllCops: DisabledByDefault: true,
# which silently turns off most Layout cops rather than customizing them)
# would otherwise produce a misleading reference full of un-corrected,
# never-checked code.
#
# Follow up on a mismatch with:
#   scripts/format-diff.sh <project> <relative/path.rb>

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

if [ ! -d "$SRC" ]; then
  echo "error: $SRC does not exist." >&2
  echo "Clone the project's source into test-projects/$PROJECT/source first." >&2
  exit 1
fi

if ! command -v rubocop >/dev/null 2>&1; then
  echo "error: rubocop not found on PATH (gem install rubocop)." >&2
  exit 1
fi

echo "Building tuxedo (release)..."
(cd "$ROOT" && cargo build --release --quiet)
TUXEDO_BIN="$ROOT/target/release/tuxedo"

echo "Preparing working copies..."
rm -rf "$BASE/tuxedo" "$BASE/rubocop"
cp -r "$SRC" "$BASE/tuxedo"
cp -r "$SRC" "$BASE/rubocop"

echo "Running tuxedo format..."
"$TUXEDO_BIN" format --quiet "$BASE/tuxedo"

echo "Running rubocop -A (Layout cops only, default config)..."
(cd "$BASE/rubocop" && rubocop --only Layout --force-default-config -A --no-color . >/dev/null 2>&1) || true

echo
echo "Comparing output..."
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
if [ "$mismatches" -gt 0 ]; then
  echo
  echo "To see a diff for one file:"
  echo "  scripts/format-diff.sh $PROJECT <relative/path.rb>"
fi
