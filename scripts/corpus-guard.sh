# Sourced by the corpus scripts (format-verify.sh, lint-baseline.sh,
# format-compare.sh) right after they set ROOT. Not meant to run on its own.
#
# A corpus run is the heaviest thing this repo does: tuxedo formats the
# whole project on every core, then rubocop forks one worker per core over
# the result (about 900 MB across eight workers on discourse). One run
# fits; two at once, or one beside a release build and an editor, has
# frozen a 7 GB machine. So:
#
#   - one corpus run at a time, failing fast rather than queueing, so a
#     second run started by mistake says so instead of silently stacking;
#   - low CPU and IO priority, so the desktop stays usable while it runs;
#   - half the cores for tuxedo's threads and rubocop's workers. Set
#     CORPUS_JOBS to change it (CORPUS_JOBS="$(nproc)" for a full-speed
#     run on an otherwise idle machine).

mkdir -p "$ROOT/test-projects"
CORPUS_LOCK="$ROOT/test-projects/.corpus-run.lock"
if command -v flock >/dev/null 2>&1; then
  # fd 9 stays open for the rest of the script and in every child it
  # starts, so the lock lasts exactly as long as the run.
  exec 9>>"$CORPUS_LOCK"
  if ! flock -n 9; then
    echo "error: another corpus run holds $CORPUS_LOCK:" >&2
    sed 's/^/  /' "$CORPUS_LOCK.owner" >&2 2>/dev/null || true
    echo "Run corpus scripts one after another; two at once can exhaust memory." >&2
    exit 3
  fi
  echo "pid $$, $(date '+%F %T'): $(basename "$0") $*" >"$CORPUS_LOCK.owner"
fi

renice -n 10 -p $$ >/dev/null 2>&1 || true
ionice -c 3 -p $$ >/dev/null 2>&1 || true

CORPUS_JOBS="${CORPUS_JOBS:-$((($(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2) + 1) / 2))}"
# rubocop --parallel sizes its worker pool from this (the parallel gem).
export PARALLEL_PROCESSOR_COUNT="$CORPUS_JOBS"
