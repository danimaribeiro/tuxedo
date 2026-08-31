#!/usr/bin/env bash
# Shows the diff between tuxedo's and rubocop's formatted output for one file
# in a test project, after scripts/format-compare.sh has been run.
#
# Usage:
#   scripts/format-diff.sh <project> <relative/path.rb>

set -euo pipefail

PROJECT="${1:-}"
FILE="${2:-}"
if [ -z "$PROJECT" ] || [ -z "$FILE" ]; then
  echo "Usage: scripts/format-diff.sh <project> <relative/path.rb>" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE="$ROOT/test-projects/$PROJECT"

for dir in tuxedo rubocop; do
  if [ ! -d "$BASE/$dir" ]; then
    echo "error: $BASE/$dir does not exist. Run scripts/format-compare.sh $PROJECT first." >&2
    exit 1
  fi
done

diff -u "$BASE/rubocop/$FILE" "$BASE/tuxedo/$FILE"
