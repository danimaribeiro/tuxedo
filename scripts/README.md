# scripts/

Tooling for checking tuxedo's formatter against real-world Ruby projects. The
four project scripts here expect a project's pristine source at
`test-projects/<project>/source`. To add one, clone it there:

```bash
git clone --depth 1 https://github.com/rails/rails test-projects/rails/source
```

`test-projects/` is gitignored; nothing under it is part of this repo.

The scripts need `ruby` and `rubocop` on `PATH`. If the shell has neither,
they are under mise: `mise exec ruby -- scripts/format-verify.sh rails`, or
put `~/.local/share/mise/installs/ruby/<version>/bin` first on `PATH`.

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
Checks five properties that must hold regardless of RuboCop, as ratchets
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
3. **Comment placement** — `scripts/comment-anchors.rb <source> <formatted>` counts
   comments that ended up next to different code than in the source (a trailing
   comment is anchored to the token before it, an own-line comment to the token
   after it). Can be run on its own; `--list N` prints the first N.
4. **Layout conformance** — `rubocop --only Layout` reports no offenses on
   tuxedo's own output, against `scripts/format-verify.rubocop.yml` (which
   encodes tuxedo's own target layout, not RuboCop's stock defaults).

## lint-baseline.sh \<project\>

Formats the project with tuxedo, then runs full RuboCop (not just Layout)
three ways and reports a per-cop table with the delta:

- **baseline** — RuboCop's shipped defaults (`scripts/lint-baseline.rubocop.yml`, which only pins the target Ruby) on the *original*,
  unformatted source.
- **tuxedo** — the same on the tuxedo-formatted copy. A file tuxedo refuses is
  counted and left unformatted; the report header says how many, so the table
  is not read as covering the whole project.
- **project-cfg** — plain `rubocop` (auto-discovering the project's own
  copied `.rubocop.yml`, if any) on the tuxedo-formatted copy. Reported as
  `n/a` if the project has no `.rubocop.yml`, or if it names plugin gems
  that aren't installed here.

**Why shipped defaults for the first two columns, deliberately
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

**Per-project options**: if `scripts/configs/<project>.tuxedo.toml` exists,
the working copy is formatted with it, so the project-cfg column measures
what the formatter can do with the options it has. Thor's picks
`hash_brace_space = "no_space"` and `access_modifier_style = "outdent"`, which
is what brings its project-cfg column to 0. The baseline and tuxedo columns
run RuboCop's stock defaults, so those two cops rise there under that config;
read them from the project-cfg column.

**Deliberate divergences**: a row marked `*` is a cop tuxedo departs from on
purpose, with its reason in the legend under the table. The list is
`scripts/lint-baseline.deliberate.tsv` (`<cop><TAB><reason>`); it is also the
index for the Layout decisions that `format-verify.rubocop.yml` carries, so
there is one list to read. A mark says the position has been decided, not
that the row may be ignored: the row and its numbers stay, so a marked cop
that starts climbing is still visible. The last line of the report counts
cops tuxedo made worse (delta above 0), split into those nobody has decided
on and the marked ones.

Output: `scripts/results/<project>-lint-baseline.txt` (committed — see
above).

## classify-linelength.rb \<dir\> [--limit N] [--list N] [--sites FILE]

Takes a formatted tree rather than a project name, the way
`comment-anchors.rb` does, and splits its over-long lines into the ones a
formatter could still fix and the ones it could not:

```
linelength: 22 over 120, 18 unsplittable, 4 breakable
```

`Layout/LineLength` dominates every layout report tuxedo produces — 2121 of
rails' 2272 offenses, 22 of thor's 51 — and most of it is a line holding one
string, regex or comment that is wider than the limit by itself. No
placement fixes those, because the only move left is splitting a literal,
which tuxedo does not do. Reporting them next to the lines tuxedo genuinely
fails to break hides the second number behind the first; this separates
them.

A line is **unsplittable** when its widest atom — a string, regex, symbol or
backtick literal, or a comment — is wide enough that the line stays over the
limit with that atom alone at the statement's own indent. A formatter can
nest deeper but never dedent, so nothing is left to try. A call written
without parentheses counts as one atom together with its single argument,
because there is no comma to break at and tuxedo does not add parentheses
the author did not write; with several arguments it is breakable, since Ruby
takes a line break after a comma without them. Everything else is
**breakable**: a construct tuxedo does not break yet. That split is triage,
not a verdict — a breakable line can still turn out to need a layout
decision rather than a fix.

Scan mode finds the over-long lines itself and needs no rubocop, at the cost
of reimplementing `Layout/LineLength`'s `AllowURI`, `AllowHeredoc` and
`IgnoreCopDirectives` defaults — close to rubocop's count but not identical.
`--sites` takes rubocop's own offence list (`path:line` per line, relative
to `<dir>`) and classifies exactly those, so the two numbers add up to the
one in the layout report.
