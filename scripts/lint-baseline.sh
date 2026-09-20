#!/usr/bin/env bash
# Measures how many RuboCop-*default*-config offenses a project has before
# and after tuxedo formats it, broken down by cop, and reports the delta.
#
# Usage:
#   scripts/lint-baseline.sh <project>
#
# Expects the project's pristine source at test-projects/<project>/source
# (see "Testing against real-world Ruby projects" in CLAUDE.md - the
# directory is gitignored, not part of this repo). Writes:
#   test-projects/<project>/tuxedo               - source formatted by tuxedo
#   scripts/results/<project>-lint-baseline.txt  - the report (also printed)
#
# The report goes under scripts/results/ (committed), not test-projects/
# (gitignored) - so a run's numbers survive as a point-in-time record
# instead of needing to be regenerated from scratch every session.
#
# Why the shipped defaults (a -c config that only pins the target Ruby): a project's own .rubocop.yml often overrides
# RuboCop's actual defaults with project-specific conventions rather than
# customizing them - Rails is a good example (DisabledByDefault: true, plus
# EnforcedStyle overrides on Layout/IndentationConsistency,
# Layout/EmptyLinesAroundAccessModifier and Layout/EndAlignment that its own
# config comments trace back to Rails' contributing guide, not RuboCop's
# recommendation). Comparing tuxedo's output against a project's own config
# measures "does tuxedo match this one project's house style," which is a
# different question from "does tuxedo match what RuboCop ships with" - and
# conflating the two produces a misleadingly large mismatch count, the same
# trap format-compare.sh's rubocop config avoids for Layout specifically
# (see the comment there re: Thor's AllCops: DisabledByDefault: true).
# Passing -c sidesteps this: it ignores the project's
# .rubocop.yml (and any `plugins:`/`require:` it declares) entirely and
# runs only RuboCop's own shipped default cop set.
#
# The delta column is the useful number: a cop with 0 baseline and N>0
# tuxedo offenses is something tuxedo's *formatting* introduced - worth
# investigating regardless of any RuboCop-compatibility goal. A cop with
# similar baseline and tuxedo counts was probably already there.

set -euo pipefail

PROJECT="${1:-}"
if [ -z "$PROJECT" ]; then
  echo "Usage: scripts/lint-baseline.sh <project>" >&2
  echo "  (expects test-projects/<project>/source to exist)" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE="$ROOT/test-projects/$PROJECT"
SRC="$BASE/source"
RESULTS_DIR="$ROOT/scripts/results"
REPORT="$RESULTS_DIR/$PROJECT-lint-baseline.txt"

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

echo "Building tuxedo (release)..." >&2
(cd "$ROOT" && cargo build --release --quiet)
TUXEDO_BIN="$ROOT/target/release/tuxedo"
DELIBERATE="$ROOT/scripts/lint-baseline.deliberate.tsv"
# RuboCop's shipped defaults, plus a target version: with none, RuboCop
# analyses as Ruby 2.7 and reports Lint/Syntax for every file using newer
# syntax, in the baseline and tuxedo columns alike.
DEFAULTS_CFG="$ROOT/scripts/lint-baseline.rubocop.yml"

echo "Formatting a working copy with tuxedo..." >&2
rm -rf "$BASE/tuxedo"
cp -r "$SRC" "$BASE/tuxedo"
# Every corpus has a few files that were never Ruby -- an ERB template
# under a .rb name, a fixture of deliberately broken code (thor's
# spec/fixtures/doc has two). tuxedo leaves those exactly as it found
# them and exits non-zero, which under `set -e` took the whole run
# down. Hide them from the walk rather than swallowing the exit
# status, so a file tuxedo refuses for any other reason still stops
# the run: tuxedo's walk honours `.ignore`. They stay in the copy, so
# both rubocop passes below still see the file set the baseline does.
find "$BASE/tuxedo" \( -name '*.rb' -o -name '*.rake' -o -name '*.gemspec' \
  -o -name '*.ru' -o -name 'Rakefile' -o -name 'Gemfile' \) -print0 |
  xargs -0 -r -P "$(nproc)" -I{} \
    sh -c 'ruby -c "$1" >/dev/null 2>&1 || printf "%s\n" "${1#"$2"/}"' _ {} "$BASE/tuxedo" \
    > "$BASE/tuxedo/.ignore"
# A project that picked non-default styles gets them from
# scripts/configs/<project>.tuxedo.toml, so the project-cfg column below
# measures what tuxedo can do with the options it has.
CONFIG_ARGS=()
if [ -f "$ROOT/scripts/configs/$PROJECT.tuxedo.toml" ]; then
  CONFIG_ARGS=(--config "$ROOT/scripts/configs/$PROJECT.tuxedo.toml")
fi
# A file tuxedo refuses (a meaning change, which is a bug in tuxedo) makes
# it exit 2 and is left exactly as the source had it. The run goes on with
# the rest, and the report says how many were left out.
FORMAT_ERR="$(mktemp)"
"$TUXEDO_BIN" format --quiet "${CONFIG_ARGS[@]}" "$BASE/tuxedo" >/dev/null 2>"$FORMAT_ERR" || true
REFUSED="$(grep -c 'formatting would change what this code does' "$FORMAT_ERR" || true)"
if [ "$REFUSED" -gt 0 ]; then
  echo "warning: tuxedo refused $REFUSED file(s); they are unformatted in the tuxedo column:" >&2
  grep 'formatting would change what this code does' "$FORMAT_ERR" |
    sed -e "s#$BASE/tuxedo/##" -e 's/:formatting would change.*$//' -e 's/^error: /  /' >&2
fi
rm -f "$FORMAT_ERR" "$BASE/tuxedo/.ignore"

# Parses `rubocop --format offenses` output into "<count>\t<CopName>" lines,
# ignoring everything else (the pending-new-cops notice, the total line).
count_offenses() {
  BUNDLE_GEMFILE=/dev/null rubocop --no-color --parallel --format offenses "$@" 2>/dev/null \
    | grep -E '^ *[0-9]+ +[A-Za-z]+/[A-Za-z0-9]+' \
    | sed -E 's/^ *([0-9]+) +([A-Za-z]+\/[A-Za-z0-9]+).*/\1\t\2/' || true
}

echo "Running rubocop (defaults, pinned target) on the original source..." >&2
BASELINE_TSV="$(mktemp)"
count_offenses -c "$DEFAULTS_CFG" "$SRC" >"$BASELINE_TSV"

echo "Running rubocop (defaults, pinned target) on the tuxedo-formatted copy..." >&2
TUXEDO_TSV="$(mktemp)"
count_offenses -c "$DEFAULTS_CFG" "$BASE/tuxedo" >"$TUXEDO_TSV"

# Third measurement: the project's *own* .rubocop.yml (if any) against the
# tuxedo-formatted copy - "does this satisfy what the project itself checks
# for," as distinct from "does this satisfy RuboCop's defaults" above. No
# -c flag: rubocop auto-discovers the copied .rubocop.yml the normal way.
# A project's config can name plugins/gems that aren't installed here - in
# that case rubocop hard-fails, and this column is reported as n/a rather
# than a misleading 0.
PROJECT_TSV="$(mktemp)"
PROJECT_CFG_ERR="$(mktemp)"
project_cfg_available=1
if [ -f "$SRC/.rubocop.yml" ]; then
  echo "Running rubocop with the project's own .rubocop.yml on the tuxedo-formatted copy..." >&2
  BUNDLE_GEMFILE=/dev/null rubocop --no-color --parallel --format offenses "$BASE/tuxedo" \
    >"$PROJECT_TSV.raw" 2>"$PROJECT_CFG_ERR" || true
  # The run worked if it listed a cop or, for a project with no offenses at
  # all, printed its `0  Total` line.
  if grep -qE '^ *[0-9]+ +([A-Za-z]+/[A-Za-z0-9]+|Total )' "$PROJECT_TSV.raw"; then
    { grep -E '^ *[0-9]+ +[A-Za-z]+/[A-Za-z0-9]+' "$PROJECT_TSV.raw" || true; } \
      | sed -E 's/^ *([0-9]+) +([A-Za-z]+\/[A-Za-z0-9]+).*/\1\t\2/' >"$PROJECT_TSV"
  elif [ -s "$PROJECT_CFG_ERR" ]; then
    project_cfg_available=0
    echo "warning: project's own .rubocop.yml failed to run (missing plugin gems?) - that column will show n/a" >&2
    sed 's/^/  /' "$PROJECT_CFG_ERR" >&2
  fi
  rm -f "$PROJECT_TSV.raw"
else
  project_cfg_available=0
fi

{
  echo "RuboCop offenses: $PROJECT"
  echo "generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  echo "baseline/tuxedo columns: rubocop with only its shipped defaults (scripts/lint-baseline.rubocop.yml pins the target Ruby)"
  if [ "$REFUSED" -gt 0 ]; then
    echo "REFUSED: $REFUSED file(s) could not be formatted (tuxedo bug, meaning check) and are UNFORMATTED in the tuxedo column."
    echo "         The table below does not cover the whole project."
  else
    echo "refused: 0 files; the tuxedo column covers every file."
  fi
  if [ "$project_cfg_available" -eq 1 ]; then
    echo "project-cfg column: rubocop with $PROJECT's own .rubocop.yml, run on the tuxedo-formatted copy"
  else
    echo "project-cfg column: n/a (no .rubocop.yml, or it needs gems not installed here)"
  fi
  echo
  printf "%-4s %-45s %10s %10s %10s %12s\n" "#" "Cop" "baseline" "tuxedo" "delta" "project-cfg"
  printf "%-4s %-45s %10s %10s %10s %12s\n" "--" "---" "--------" "------" "-----" "-----------"

  awk -F'\t' -v have_proj="$project_cfg_available" '
    FNR == NR { base[$2] = $1; cops[$2] = 1; next }
    FILENAME == ARGV[2] { tux[$2] = $1; cops[$2] = 1; next }
    { proj[$2] = $1; cops[$2] = 1 }
    END {
      for (c in cops) {
        b = (c in base) ? base[c] : 0
        t = (c in tux) ? tux[c] : 0
        p = (c in proj) ? proj[c] : 0
        proj_str = have_proj ? p : "n/a"
        printf "%s\t%d\t%d\t%d\t%s\n", c, b, t, t - b, proj_str
      }
    }
  ' "$BASELINE_TSV" "$TUXEDO_TSV" "$PROJECT_TSV" \
    | sort -t$'\t' -k4,4nr -k3,3nr \
    | awk -F'\t' -v have_proj="$project_cfg_available" -v deliberate="$DELIBERATE" '
        BEGIN {
          while ((getline line < deliberate) > 0) {
            if (line ~ /^#/ || line == "") continue
            split(line, f, "\t")
            reason[f[1]] = f[2]
          }
        }
        {
          name = $1
          if ($1 in reason) {
            name = $1 " *"
            order[++n] = $1
            if ($4 > 0) { marked_up++; marked_delta += $4 }
          } else if ($4 > 0) {
            open_up++; open_delta += $4
          }
          printf "%-4d %-45s %10d %10d %+10d %12s\n", NR, name, $2, $3, $4, $5
          b+=$2; t+=$3; d+=$4; if (have_proj && $5 != "n/a") p+=$5
        }
        END {
          proj_total = have_proj ? p : "n/a"
          printf "\n%-4s %-45s %10d %10d %+10d %12s\n", "", "TOTAL", b, t, d, proj_total
          if (n > 0) {
            print ""
            print "* deliberate: decided, still counted"
            for (i = 1; i <= n; i++) printf "    %s: %s\n", order[i], reason[order[i]]
          }
          print ""
          printf "introduced by tuxedo (delta above 0): %d cops (%+d) not decided, %d cops (%+d) deliberate\n", \
            open_up, open_delta, marked_up, marked_delta
        }
      '
} | tee "$REPORT"

rm -f "$BASELINE_TSV" "$TUXEDO_TSV" "$PROJECT_TSV" "$PROJECT_CFG_ERR"

echo
echo "Report saved to scripts/results/$PROJECT-lint-baseline.txt"
