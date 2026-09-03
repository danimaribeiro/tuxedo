#!/usr/bin/env bash
# Verifies Tuxedo's output against the three properties it must hold, rather
# than diffing it byte-for-byte against rubocop the way
# scripts/format-compare.sh does.
#
# Usage:
#   scripts/format-verify.sh <project>
#
# Why a second script: rubocop is a linter, so it only ever *adds* line
# breaks — it has no mandate to overrule an author who broke a line by hand.
# Tuxedo is a formatter and normalizes in both directions. Given
# `x = [1,\n     2]`, rubocop block-expands it and tuxedo collapses it to
# `x = [1, 2]`; both are right by their own rules. So byte-equality with
# rubocop is not reachable, and chasing the last few percent of it would
# mean making Tuxedo worse. format-compare.sh's mismatch count is a
# similarity gauge, not a correctness oracle.
#
# What is reachable is a subset relation — everything Tuxedo emits is
# something rubocop accepts — plus two invariants that hold regardless of
# rubocop. This script measures all three:
#
#   1. PARSE     Output is still valid Ruby. Measured as a regression count
#                against the source: a file that parsed before formatting
#                and does not parse after is a bug, full stop. (Files that
#                never parsed — ERB templates and the like living under a
#                .rb name — are excluded, not counted as failures.)
#
#   2. IDEMPOTENT  format(format(x)) == format(x). A formatter that keeps
#                changing its mind produces churn in every diff.
#
#   3. LAYOUT    `rubocop --only Layout` reports no offenses on the output,
#                against scripts/format-verify.rubocop.yml — which encodes
#                the layout Tuxedo is aiming for (all-or-nothing expansion,
#                block indentation), not rubocop's stock defaults. See the
#                comments in that file.
#
# All three are ratchets, not pass/fail bars: Tuxedo is early, and the
# numbers are expected to start high and only ever go down. Record them
# before a change and compare after.

set -euo pipefail

PROJECT="${1:-}"
if [ -z "$PROJECT" ]; then
  echo "Usage: scripts/format-verify.sh <project>" >&2
  echo "  (expects test-projects/<project>/source to exist)" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE="$ROOT/test-projects/$PROJECT"
SRC="$BASE/source"
OUT="$BASE/verify"

if [ ! -d "$SRC" ]; then
  echo "error: $SRC does not exist." >&2
  echo "Clone the project's source into test-projects/$PROJECT/source first." >&2
  exit 1
fi

if ! command -v ruby >/dev/null 2>&1; then
  echo "error: ruby not found on PATH (needed for the parse check)." >&2
  exit 1
fi

echo "Building tuxedo (release)..."
(cd "$ROOT" && cargo build --release --quiet)
TUXEDO_BIN="$ROOT/target/release/tuxedo"

echo "Preparing working copy..."
rm -rf "$OUT"
cp -r "$SRC" "$OUT"

echo "Running tuxedo format..."
"$TUXEDO_BIN" format --quiet "$OUT" >/dev/null

# --- 1. Parse check ---------------------------------------------------
echo
echo "[1/3] Parse check (ruby -c)..."
parse_ok=0
parse_regressed=0
parse_skipped=0
regressions=()
while IFS= read -r -d '' file; do
  rel="${file#"$SRC"/}"
  if ! ruby -c "$SRC/$rel" >/dev/null 2>&1; then
    # Never parsed to begin with (ERB template, fixture of deliberately
    # broken code, ...). Not ours to fix.
    parse_skipped=$((parse_skipped + 1))
    continue
  fi
  if ruby -c "$OUT/$rel" >/dev/null 2>&1; then
    parse_ok=$((parse_ok + 1))
  else
    parse_regressed=$((parse_regressed + 1))
    regressions+=("$rel")
    echo "  BROKEN: $rel"
    # `|| true`: head closing the pipe early trips pipefail otherwise.
    ruby -c "$OUT/$rel" 2>&1 | head -2 | sed 's/^/          /' || true
  fi
done < <(find "$SRC" -name '*.rb' -print0 | sort -z)

# --- 2. Idempotence ---------------------------------------------------
echo
echo "[2/3] Idempotence check (format twice)..."
TWICE="$BASE/verify-twice"
rm -rf "$TWICE"
cp -r "$OUT" "$TWICE"
"$TUXEDO_BIN" format --quiet "$TWICE" >/dev/null
unstable=0
while IFS= read -r -d '' file; do
  rel="${file#"$OUT"/}"
  if ! diff -q "$OUT/$rel" "$TWICE/$rel" >/dev/null 2>&1; then
    echo "  UNSTABLE: $rel"
    unstable=$((unstable + 1))
  fi
done < <(find "$OUT" -name '*.rb' -print0 | sort -z)
rm -rf "$TWICE"

# --- 3. Layout conformance --------------------------------------------
echo
echo "[3/3] Layout conformance (rubocop --only Layout)..."
layout_offenses="n/a"
if command -v rubocop >/dev/null 2>&1; then
  # Syntax errors would be reported as Lint/Syntax and drown out the Layout
  # signal, so only lint the files that actually parse.
  parsing_files="$(mktemp)"
  while IFS= read -r -d '' file; do
    rel="${file#"$OUT"/}"
    if ruby -c "$OUT/$rel" >/dev/null 2>&1; then
      printf '%s\n' "$OUT/$rel" >>"$parsing_files"
    fi
  done < <(find "$OUT" -name '*.rb' -print0 | sort -z)

  rubocop_out="$(mktemp)"
  if [ -s "$parsing_files" ]; then
    # shellcheck disable=SC2046
    BUNDLE_GEMFILE=/dev/null rubocop --only Layout \
      -c "$ROOT/scripts/format-verify.rubocop.yml" \
      --no-color --format offenses \
      $(cat "$parsing_files") >"$rubocop_out" 2>&1 || true
    echo
    grep -E '^ *[0-9]+ +Layout/' "$rubocop_out" | head -25 | sed 's/^/  /' || true
    layout_offenses="$(grep -cE '^ *[0-9]+ +Layout/' "$rubocop_out" || true)"
    layout_total="$(awk '/^ *[0-9]+ +Layout\//{ s += $1 } END { print s + 0 }' "$rubocop_out")"
  fi
  rm -f "$parsing_files" "$rubocop_out"
else
  echo "  skipped: rubocop not found on PATH (gem install rubocop)"
  layout_total="n/a"
fi

# --- Summary -----------------------------------------------------------
echo
echo "======================================================================"
echo "  $PROJECT"
echo "----------------------------------------------------------------------"
printf "  parse       %s ok, %s BROKEN by tuxedo (%s skipped, never parsed)\n" \
  "$parse_ok" "$parse_regressed" "$parse_skipped"
printf "  idempotent  %s files change on a second format\n" "$unstable"
printf "  layout      %s offenses across %s distinct cops\n" \
  "${layout_total:-n/a}" "${layout_offenses:-n/a}"
echo "======================================================================"
echo
echo "Output kept at test-projects/$PROJECT/verify for inspection."

if [ "$parse_regressed" -gt 0 ]; then
  echo
  echo "Parse regressions are the highest-priority failures: they mean tuxedo"
  echo "silently corrupted working Ruby. Fix these before chasing layout."
  exit 1
fi
