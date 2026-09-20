#!/usr/bin/env bash
# Verifies Tuxedo's output against the three properties it must hold, rather
# than diffing it byte-for-byte against rubocop the way
# scripts/format-compare.sh does.
#
# Usage:
#   scripts/format-verify.sh <project>
#
# Report saved to scripts/results/<project>-format-verify.txt (committed -
# survives across sessions; commit the update alongside whatever prompted
# the run).
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
# rubocop. This script measures all five:
#
#   0. MEANING   The output is the same program. Tuxedo parses its own
#                output and compares it to the source's AST (see
#                src/format/verify.rs); on a mismatch it refuses to write
#                the file and reports it. Counted here as the number of
#                files refused. `ruby -c` cannot see this: `defined?(Foo)`
#                coming out as `Foo` is valid Ruby and a different program.
#                This is the number that matters most.
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
#   3. COMMENTS `scripts/comment-anchors.rb`: how many comments ended up next
#                to different code than in the source. The meaning check
#                already guarantees none is lost; a comment that drifts away
#                from what it describes is the remaining way to ruin one.
#
#   4. LAYOUT    `rubocop --only Layout` reports no offenses on the output,
#                against scripts/format-verify.rubocop.yml — which encodes
#                the layout Tuxedo is aiming for (all-or-nothing expansion,
#                block indentation), not rubocop's stock defaults. See the
#                comments in that file.
#
# All five are ratchets, not pass/fail bars: Tuxedo is early, and the
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
RESULTS_DIR="$ROOT/scripts/results"
REPORT="$RESULTS_DIR/$PROJECT-format-verify.txt"

if [ ! -d "$SRC" ]; then
  echo "error: $SRC does not exist." >&2
  echo "Clone the project's source into test-projects/$PROJECT/source first." >&2
  exit 1
fi

if ! command -v ruby >/dev/null 2>&1; then
  echo "error: ruby not found on PATH (needed for the parse check)." >&2
  exit 1
fi

mkdir -p "$RESULTS_DIR"
# Mirrors stdout (not stderr - that's just progress noise) to the report
# file for the rest of the script, without disturbing this script's exit
# code the way `... | tee` would.
exec > >(tee "$REPORT")

# The files tuxedo formats, which is what has to be verified. Mirrors the
# matcher in src/walk.rs: a check that looked only at *.rb would leave
# every Rakefile, Gemfile, .rake task, .gemspec and config.ru tuxedo
# rewrites unmeasured, and those are the least test-covered files in a
# project.
ruby_files() {
  find "$1" \( -name '*.rb' -o -name '*.rake' -o -name '*.gemspec' \
    -o -name '*.ru' -o -name 'Rakefile' -o -name 'Gemfile' \) \
    -print0 | sort -z
}

echo "Building tuxedo (release)..."
(cd "$ROOT" && cargo build --release --quiet)
TUXEDO_BIN="$ROOT/target/release/tuxedo"

echo "Preparing working copy..."
rm -rf "$OUT"
cp -r "$SRC" "$OUT"

echo "Running tuxedo format..."
format_err="$(mktemp)"
# Exits 2 when any file was refused; that is what the next section counts.
"$TUXEDO_BIN" format --quiet "$OUT" >/dev/null 2>"$format_err" || true

# --- 0. Meaning (AST equivalence) -------------------------------------
echo
echo "[0/4] Meaning check (source AST == output AST)..."
# Syntax errors are not counted: those files never parsed (see the parse
# check's "skipped"), and tuxedo leaves them alone by design.
meaning_changed="$(grep -c 'formatting would change what this code does' "$format_err" || true)"
if [ "$meaning_changed" -gt 0 ]; then
  # The error line names the file; the two lines after it show where the
  # ASTs first diverge.
  grep -A2 'formatting would change what this code does' "$format_err" |
    sed -e "s#$OUT/##" -e 's/:formatting would change.*$//' -e 's/^error: /  CHANGED: /' |
    cut -c1-200 | head -60 || true
fi
rm -f "$format_err"

# --- 1. Parse check ---------------------------------------------------
echo
echo "[1/4] Parse check (ruby -c)..."
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
done < <(ruby_files "$SRC")

# --- 2. Idempotence ---------------------------------------------------
echo
echo "[2/4] Idempotence check (format twice)..."
TWICE="$BASE/verify-twice"
rm -rf "$TWICE"
cp -r "$OUT" "$TWICE"
# Exit status and stderr were already accounted for in the first pass.
"$TUXEDO_BIN" format --quiet "$TWICE" >/dev/null 2>&1 || true
unstable=0
while IFS= read -r -d '' file; do
  rel="${file#"$OUT"/}"
  # Same exclusion as the parse check: a file that was never Ruby (an ERB
  # template under a .rb name) is not something we can be stable on, and
  # holding the formatter to it just adds permanent noise.
  if ! ruby -c "$SRC/$rel" >/dev/null 2>&1; then
    continue
  fi
  if ! diff -q "$OUT/$rel" "$TWICE/$rel" >/dev/null 2>&1; then
    echo "  UNSTABLE: $rel"
    # Show what actually changed on the second pass, not just that it did -
    # this is the difference between "go look at this file" and "go look at
    # this file, on this line, for this reason."
    diff -u "$OUT/$rel" "$TWICE/$rel" 2>&1 | tail -n +3 | head -8 | sed 's/^/          /' || true
    unstable=$((unstable + 1))
  fi
done < <(ruby_files "$OUT")
rm -rf "$TWICE"

# --- 3. Comment placement ---------------------------------------------
echo
echo "[3/4] Comment placement (scripts/comment-anchors.rb)..."
comments_out="$(mktemp)"
ruby "$ROOT/scripts/comment-anchors.rb" "$SRC" "$OUT" --list 25 >"$comments_out"
grep DISPLACED "$comments_out" | cut -c1-160 || true
comments_summary="$(tail -1 "$comments_out")"
comments_summary="${comments_summary#comments: }"
rm -f "$comments_out"

# --- 4. Layout conformance --------------------------------------------
echo
echo "[4/4] Layout conformance (rubocop --only Layout)..."
layout_offenses="n/a"
unparsed_by_rubocop=0
if command -v rubocop >/dev/null 2>&1; then
  # Syntax errors would be reported as Lint/Syntax and drown out the Layout
  # signal, so only lint the files that actually parse.
  parsing_files="$(mktemp)"
  while IFS= read -r -d '' file; do
    rel="${file#"$OUT"/}"
    if ruby -c "$OUT/$rel" >/dev/null 2>&1; then
      printf '%s\n' "$OUT/$rel" >>"$parsing_files"
    fi
  done < <(ruby_files "$OUT")

  rubocop_out="$(mktemp)"
  # The same run twice over, in two shapes: `offenses` for the per-cop
  # counts below, `emacs` for the one file:line per offense that the
  # LineLength split needs. Two formatters beat a second rubocop pass over
  # the same three and a half thousand files.
  emacs_out="$(mktemp)"
  if [ -s "$parsing_files" ]; then
    # shellcheck disable=SC2046
    BUNDLE_GEMFILE=/dev/null rubocop --only Layout,Lint/Syntax \
      -c "$ROOT/scripts/format-verify.rubocop.yml" \
      --no-color --format offenses \
      --format emacs --out "$emacs_out" \
      $(cat "$parsing_files") >"$rubocop_out" 2>&1 || true
    echo
    grep -E '^ *[0-9]+ +Layout/' "$rubocop_out" | head -25 | sed 's/^/  /' || true
    layout_offenses="$(grep -cE '^ *[0-9]+ +Layout/' "$rubocop_out" || true)"
    layout_total="$(awk '/^ *[0-9]+ +Layout\//{ s += $1 } END { print s + 0 }' "$rubocop_out")"
    # A file rubocop cannot parse contributes no Layout offenses at all,
    # which reads as a clean file. Count them so it cannot.
    unparsed_by_rubocop="$(awk '/^ *[0-9]+ +Lint\/Syntax/{ s += $1 } END { print s + 0 }' "$rubocop_out")"
    if [ "$unparsed_by_rubocop" -gt 0 ]; then
      echo
      echo "  WARNING: rubocop could not parse $unparsed_by_rubocop of the files above; they count as zero offenses."
      grep -E ': (E|F): Lint/Syntax' "$emacs_out" | sed "s#^$OUT/##" | head -10 | sed 's/^/    /' || true
    fi

    # Most of LineLength is a literal nobody can place under the limit, so
    # the raw count says little about how much work is left. Hand the exact
    # offense list to the classifier rather than letting it find the lines
    # itself, so the split adds up to the number printed above.
    # Match the message, not `: <cop>:`: rubocop prefixes an offense it can
    # autocorrect with `[Correctable]`, which sits between the two colons.
    linelength_sites="$(mktemp)"
    grep -F 'Layout/LineLength: Line is too long' "$emacs_out" |
      cut -d: -f1,2 | sed "s#^$OUT/##" >"$linelength_sites" || true
    if [ -s "$linelength_sites" ]; then
      linelength_summary="$(ruby "$ROOT/scripts/classify-linelength.rb" "$OUT" \
        --sites "$linelength_sites")"
    fi
    rm -f "$linelength_sites"
  fi
  rm -f "$parsing_files" "$rubocop_out" "$emacs_out"
else
  echo "  skipped: rubocop not found on PATH (gem install rubocop)"
  layout_total="n/a"
fi

# --- Summary -----------------------------------------------------------
echo
echo "======================================================================"
echo "  $PROJECT"
echo "----------------------------------------------------------------------"
printf "  meaning     %s files refused: formatting would have changed the AST\n" \
  "$meaning_changed"
printf "  parse       %s ok, %s BROKEN by tuxedo (%s skipped, never parsed)\n" \
  "$parse_ok" "$parse_regressed" "$parse_skipped"
printf "  idempotent  %s files change on a second format\n" "$unstable"
printf "  comments    %s\n" "$comments_summary"
printf "  layout      %s offenses across %s distinct cops\n" \
  "${layout_total:-n/a}" "${layout_offenses:-n/a}"
if [ "$unparsed_by_rubocop" -gt 0 ]; then
  printf "  layout      NOT MEASURED for %s files rubocop cannot parse (Lint/Syntax)\n" \
    "$unparsed_by_rubocop"
fi
if [ -n "${linelength_summary:-}" ]; then
  printf "  %s\n" "$linelength_summary"
fi
echo "======================================================================"
echo
echo "Output kept at test-projects/$PROJECT/verify for inspection."
echo "Report saved to scripts/results/$PROJECT-format-verify.txt"

if [ "$meaning_changed" -gt 0 ]; then
  echo
  echo "Meaning changes are the highest-priority failures: tuxedo's output was a"
  echo "different program. The files were left untouched, but each one is a bug."
  exit 1
fi

if [ "$parse_regressed" -gt 0 ]; then
  echo
  echo "Parse regressions are the highest-priority failures: they mean tuxedo"
  echo "silently corrupted working Ruby. Fix these before chasing layout."
  exit 1
fi
