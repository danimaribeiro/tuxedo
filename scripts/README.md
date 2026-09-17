# scripts/

Tooling for checking tuxedo's formatter against real-world Ruby projects. All
four scripts here expect a project's pristine source at
`test-projects/<project>/source` — see "Testing against real-world Ruby
projects" in the root `CLAUDE.md` for how to add one. `test-projects/` is
gitignored; nothing under it is part of this repo.

Each script writes its own working copy under `test-projects/<project>/` and
regenerates it fresh on every run — those are scratch space, safe to delete.
`scripts/results/` is the opposite: it's committed, so a script's numbers
survive across sessions instead of needing to be regenerated from scratch
every time someone wants to know "where do we stand." When a script writes
there, commit the update alongside whatever formatter/lint change prompted
the run, the same way you'd commit a snapshot test update.

## format-compare.sh \<project\>

Formats the project with tuxedo, autocorrects a copy with `rubocop --only
Layout` (using `scripts/format-compare.rubocop.yml`, not the project's own
config — see the comment in that file for why), and reports which files
differ and the speedup. This is a **similarity gauge, not a correctness
oracle** — tuxedo normalizes bidirectionally (it collapses lines a human
broke by hand) and RuboCop's Layout autocorrect only ever adds breaks, so
100% is not the target. See `format-verify.sh` for what actually needs to be
100%.

## format-diff.sh \<project\> \<relative/path.rb\>

Shows the diff for one file between `format-compare.sh`'s two working copies.
Run `format-compare.sh` first.

## format-verify.sh \<project\>

The metric that matters more than `format-compare.sh`'s similarity score.
Checks four properties that must hold regardless of RuboCop, as ratchets
(numbers should only go down over time, never treated as a pass/fail bar):

0. **Meaning** — the output is the same program as the source. Tuxedo parses
   its own output and compares ASTs (`src/format/verify.rs`); on a mismatch it
   refuses to write the file. This counts the refused files and prints where
   each pair of ASTs first diverges. It is the highest-priority number:
   `ruby -c` cannot see `defined?(Foo)` coming out as `Foo`.
1. **Parse** — output is still valid Ruby (a parse regression is a bug, full
   stop; files that never parsed to begin with, like ERB templates under a
   `.rb` name, are excluded rather than counted as failures).
2. **Idempotence** — `format(format(x)) == format(x)`. For every unstable file, prints
   an 8-line unified diff of what changed on the second pass right under the filename —
   that's usually enough to see the exact pattern without opening either file by hand.
3. **Layout conformance** — `rubocop --only Layout` reports no offenses on
   tuxedo's own output, against `scripts/format-verify.rubocop.yml` (which
   encodes tuxedo's own target layout, not RuboCop's stock defaults).

## lint-baseline.sh \<project\>

Formats the project with tuxedo, then runs full RuboCop (not just Layout)
three ways and reports a per-cop table with the delta:

- **baseline** — `rubocop --force-default-config` on the *original*,
  unformatted source.
- **tuxedo** — `rubocop --force-default-config` on the tuxedo-formatted copy.
- **project-cfg** — plain `rubocop` (auto-discovering the project's own
  copied `.rubocop.yml`, if any) on the tuxedo-formatted copy. Reported as
  `n/a` if the project has no `.rubocop.yml`, or if it names plugin gems
  that aren't installed here.

**Why `--force-default-config` for the first two columns, deliberately
ignoring the project's own config**: a project's `.rubocop.yml` often
*overrides* RuboCop's actual defaults with house-specific conventions rather
than customizing them — Rails is a good example. Its config sets
`Layout/IndentationConsistency: EnforcedStyle: indented_internal_methods`,
`Layout/EmptyLinesAroundAccessModifier: EnforcedStyle: only_before`, and
`Layout/EndAlignment: EnforcedStyleAlignWith: variable` — all three are
non-default values, and RuboCop's own shipped config comments trace the
first two back to Rails' own contributing guide, not a RuboCop
recommendation. Comparing tuxedo against a project's own config measures
"does tuxedo match this one project's house style," which is a different,
narrower question than "does tuxedo match what RuboCop ships with" — the
same trap `format-compare.sh`'s dedicated rubocop config already avoids for
Layout specifically (see the comment there re: Thor's `AllCops:
DisabledByDefault: true`). Running against real defaults, then separately
against the project's own config as a third column, keeps both questions
answerable instead of silently conflating them into one misleading number.

**Reading the delta column**: a cop with 0 baseline and N>0 tuxedo offenses
is something tuxedo's *formatting itself* introduced — worth investigating
regardless of any RuboCop-compatibility goal, independent of the
project-cfg column entirely. A cop with similar baseline and tuxedo counts
was probably already present in the source either way.

Output: `scripts/results/<project>-lint-baseline.txt` (committed — see
above).
