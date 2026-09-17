# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Tuxedo is a fast Ruby code formatter and linter written in Rust, aiming to be a drop-in
replacement for RuboCop (the way `ruff` is for Python). It's early-stage and not yet
production-ready. Repo has two parts:

- **Rust crate at the root** (`Cargo.toml`, `src/`) — the `tuxedo` binary itself.
- **`ruby/` directory** — the `tuxedo-r` gem that wraps and distributes precompiled
  `tuxedo` binaries for each platform (the "R" stands for both Ruby and Rust).

## The goal: formatting optimized for diff quality

Tuxedo exists to end formatting discussion in code review. "Move this space" is not
feedback worth a round-trip, and the way to make it stop is a formatter whose output is
not up for debate. So Tuxedo is opinionated by design and takes no per-project style
config for the choices below.

Where more than one layout is defensible, the tie-breaker is **which one produces the
better diff later**. Three properties follow from that, and they should be treated as
the spec:

1. **All-or-nothing expansion.** A collection either fits on one line or every element
   gets its own line. Never bin-pack (fill as many elements per line as the column limit
   allows) — packing makes adding or removing a single element reflow an arbitrary number
   of unrelated lines. This is what a Wadler-style `group` gives you for free, which is
   why the formatter is migrating to the `Doc` IR in `src/format/doc.rs`.

2. **Block indentation.** When a collection expands, the bracket sits alone on its own
   line and elements sit at a fixed indent from the start of the statement — not aligned
   under the opening bracket. Bracket alignment lets the *name* of a thing control the
   indentation of every element, so renaming a variable reindents the whole literal.

3. **No trailing comma** on the last element. (A deliberate choice — it costs a little
   diff quality on append, but matches RuboCop's `Style/TrailingCommaIn*` default of
   `no_comma` and Ruby convention.)

```ruby
# what tuxedo aims for                 # not this (bin-packed, bracket-aligned)
ROLES = [                              ROLES = ["administrator", "moderator",
  "administrator",                              "contributor", "subscriber"]
  "moderator",
  "contributor",
  "subscriber"
]
```

RuboCop is a **reference, not the target.** RuboCop is a linter: it only ever *adds* line
breaks, because it has no mandate to overrule an author who broke a line by hand. Tuxedo
is a formatter and normalizes in both directions. Given `x = [1,\n     2]`, RuboCop
block-expands it and Tuxedo collapses it to `x = [1, 2]` — both correct by their own
rules. Byte-equality with RuboCop is therefore not reachable, and chasing the last few
percent of it would mean making Tuxedo worse. See "Testing against real-world Ruby
projects" below for the two scripts and what each one actually measures.

## Commands

### Rust crate (root)

```bash
cargo build --release          # build the binary -> target/release/tuxedo
cargo test                     # run all tests (includes insta snapshot tests)
cargo test <name>              # run a single test by name
cargo fmt --check              # formatting check (CI-enforced)
cargo clippy --all-targets -- -D warnings   # lint (CI-enforced, warnings fail)
```

Snapshot tests use `insta`; review/accept changed snapshots with `cargo insta review`
(requires `cargo install cargo-insta`).

### Ruby gem (`ruby/`)

```bash
cd ruby
bundle install
bundle exec rake test          # run gem tests (Minitest)

bundle exec rake build:all     # compile the tuxedo binary for every supported platform
                                # (cross-compiled targets require `cargo install cross`)
bundle exec rake package       # package all platform gems into ruby/pkg/
```

CI runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
for the Rust side, and `bundle exec rake test` (from `ruby/`) for the gem side — run these
before considering a change done.

## Testing against real-world Ruby projects

`test-projects/` (gitignored, not committed) holds real Ruby codebases used to stress-test
the formatter beyond the fixtures in `tests/fixtures/`. To add one, clone its source into
`test-projects/<name>/source`. See `scripts/README.md` for the full script list
(`format-compare.sh`, `format-diff.sh`, `format-verify.sh`, `lint-baseline.sh`) and what
each one measures.

### `format-verify.sh` — the metric that actually matters

```bash
scripts/format-verify.sh <name>
```

Byte-diffing against RuboCop's autocorrect (`format-compare.sh`, see `scripts/README.md`)
measures *similarity*, which is capped below 100% for the reason given under "The goal"
above. `format-verify.sh` measures the properties Tuxedo must actually hold instead, and
each of them is reachable at 100%:

0. **Meaning.** The output is the same program as the source. `tuxedo::format` parses its
   own output and compares it to the source's AST, ignoring layout (`src/format/verify.rs`);
   on a mismatch it returns `FormatError::AstMismatch` and the CLI leaves the file
   untouched. The script counts those refusals and prints where the two ASTs first
   diverge. **This is the highest-priority number**, above parse: `defined?(Foo)` coming
   out as `Foo` is valid Ruby and a different program, so `ruby -c` cannot see it. A
   non-zero count is always a formatter bug, and the printed AST window usually names the
   node type at fault.

1. **Parse.** Output is still valid Ruby, counted as regressions against the source: a
   file that parsed before formatting and does not parse after is a bug, full stop.
   Files that never parsed (ERB templates and the like under a `.rb` name) are excluded
   rather than counted as failures. A parse regression means Tuxedo silently corrupted
   working code, and the script exits non-zero on any. Byte-diffing against RuboCop cannot
   see this: corrupted output just shows up as one more "file that differs."

2. **Idempotence.** `format(format(x)) == format(x)`. A formatter that keeps changing its
   mind produces churn in every diff.

3. **Comment placement.** `scripts/comment-anchors.rb` counts comments that ended up
   next to different code than in the source. The meaning check already guarantees no
   comment is lost; this measures whether each one still describes the same thing. A
   trailing comment is anchored to the token before it (ignoring a comma), an own-line
   comment to the first token after it. Reformatting the code around a comment never
   changes its anchor, so only a comment that moved is counted. A few are intentional:
   `def x; end # :nodoc:` expands to `def x # :nodoc:`.

4. **Layout conformance.** `rubocop --only Layout` reports no offenses on Tuxedo's
   output — a *subset* relation ("everything Tuxedo emits, RuboCop accepts"), not
   equality. It runs against `scripts/format-verify.rubocop.yml`, which encodes the three
   properties above rather than RuboCop's stock defaults; the six `*LineBreak`/
   `*LineBreaks` cops that forbid bin-packing all ship disabled by default, so they have
   to be turned on explicitly. Files that fail the parse check are excluded from this
   pass, since `Lint/Syntax` would otherwise drown out the Layout signal.

All five are **ratchets, not pass/fail bars.** Tuxedo is early; the numbers start high
and should only ever go down. Record them before a change and compare after.

There's also `scripts/lint-baseline.sh <project>` — runs full RuboCop (not just Layout)
three ways (RuboCop's real defaults on the original source, on tuxedo's output, and the
project's own `.rubocop.yml` on tuxedo's output) and reports a per-cop delta table.
`scripts/results/` holds the latest saved run against `rails` and `thor` — regenerate
with `format-verify.sh`/`lint-baseline.sh` before trusting stale numbers.

## Where to pick up work next

There are two backlogs, and they answer different questions.

**The plan** — GitHub issues, grouped into five milestones, with a roadmap doc at
<https://claude.ai/code/artifact/a4eeb67b-2e0c-422a-af19-730c276f1055> holding the
reasoning behind each one. Issue numbers follow the order of work, so the next thing
to pick up is normally the lowest open number: `gh issue list`. The issue bodies carry
the detail — which node types, which functions, which traps — so read the issue before
planning the work, and record what you learn back in it.

| Milestone | Outcome | State |
| --- | --- | --- |
| 1. Safety net | An unfinished node passes through unchanged and never disappears | Done, PR #33 |
| 2. Comment attachment | Comments stay beside the code they describe | Done, PR #34 |
| 3. Finish the Doc migration | The capture and snapshot workarounds in `mod.rs` go away | In progress, PR #35 merged |
| 4. Formatter cleanups | Config, block indentation and CLI gaps are closed | #26 done, PR #12; #25, #27 open |
| 5. Linter | `tuxedo check` reports real diagnostics with RuboCop cop names | Not started |

Milestone 3 is where the work is. #22 is a tracking issue; the actual work is #36 to
#43, one per file, and they must be done roughly in that order — #40 (statements and
comments) is the keystone, because every construct with a body contains a
`StatementsNode`, and #41, #42 and #43 all depend on it. #24 (whether to replace
`pretty`) gets settled inside #40, not afterwards: `line_suffix` for trailing comments
is the only capability still missing, so whichever way #40 goes decides it.

Milestone 4 is small and independent of that, so it's the thing to reach for if the Doc
migration is blocked. Note #25 is half done: PR #33 wired `.tuxedo.toml` into `format`,
but `run_check` still takes `_config` and ignores it.

**Getting onto the `Doc` IR is the goal; byte-identical output is not.** Byte-identity
was the rule while the migration started, and it is no longer — a step that makes the
output better is welcome, and a number that moves the wrong way by a little is not a
blocker. What a step must not do is make the formatting *worse*. The test suite cannot
tell you either way — both bugs found in PR #35 were invisible to it — so measure:
format all of `test-projects/rails` and `test-projects/thor` with the binary from `main`
and with your branch, `diff -rq` the two trees, and read what actually changed. Show the
before and after in the PR, and run `format-verify.sh` on both projects so the meaning,
parse, idempotence, comment and Layout numbers back up the judgement. Meaning, parse and
idempotence are the ones that may never regress.

**The bugs** — `scripts/results/`, committed, holding the latest saved run against `rails`
and `thor`. Don't keep a separate hand-written list of known issues here; it will drift the
moment one gets fixed and this file doesn't get edited to match.

`format-verify.sh`'s output is the primary guide: its meaning section names an exact
file plus the point where the source and output ASTs diverge, its idempotence section
names an exact file plus an 8-line diff of what changed on a second `format` pass, and
its parse section names an exact file plus the parse error — each points straight at a
bug, no further digging needed. `lint-baseline.sh`'s per-cop delta table is the secondary guide, for
"which RuboCop-ecosystem thing changed and by how much." Pick whichever entry looks worth
tackling and go straight to it.

If the saved reports look stale (or you've just fixed something and want to confirm),
regenerate before trusting them: `scripts/format-verify.sh rails`, `scripts/format-verify.sh
thor`, `scripts/lint-baseline.sh rails`, `scripts/lint-baseline.sh thor` — commit the
updated `scripts/results/*.txt` alongside the fix.

## Architecture

- `src/main.rs` — CLI entry point (clap-based). Two subcommands: `format` and `check`.
  Both accept paths (or read from stdin when no paths given), walk the filesystem via
  `walk_and_process`, and either rewrite files in place or just report (`--check` /
  without `--fix`).
- `src/walk.rs` — shared file-walking/parallel-processing logic (`walk_and_process`) used
  by both the `format` and `check` commands; respects `.gitignore` unless `--no-ignore`.
- `src/format/` — the formatter, built on `ruby-prism` (Ruby's own parser, via the
  `ruby-prism` crate) to parse source into an AST and re-emit formatted code. When adding
  or debugging a `visit_*_node` method, dump the actual Prism AST first instead of guessing
  field names/types: `echo '<ruby snippet>' | cargo run --release -- format --ast` (stdin
  only). It prints node names, field order, and which fields are optional/repeated —
  matches `ruby_prism`'s generated Rust accessors 1:1.

  **Useful Debugging Commands for Formatter Development:**
  ```bash
  # 1. Dump AST for a specific ruby construct
  echo 'config = args.last.is_a?(Hash) ? args.pop : {}' | cargo run --release -- format --ast
  
  # 2. Test how the formatter handles a specific ruby snippet
  echo 'yield unless pretend?' | cargo run --release -- format

  # 3. Explore the ruby-prism Rust API documentation locally
  cargo doc --package ruby-prism --open
  
  # 4. View the raw generated Rust node structures directly (useful for finding field accessors)
  find target -name bindings.rs | grep ruby-prism | xargs grep -A 10 "pub struct IfNode"
  ```

  **Node dispatch is an exhaustive `match`, not `impl Visit`** (`src/format/dispatch.rs`).
  `Visit`'s default for an un-overridden node walks the children and emits nothing for the
  node itself, which silently deletes syntax. Every Prism node type must be listed in
  `dispatch.rs`, under one of three groups: `leaf` (its `fmt_*` returns a `Doc`),
  `formatted` (its `fmt_*` writes to `self.output`) or `verbatim` (source text copied
  through unchanged). To add a formatter for a node, move it out of `verbatim`. A node type
  added by a Prism upgrade is a compile error until it is listed.

  `leaf` is where every node type is headed. Use `self.node_doc(&child)` to get a child as
  a `Doc`: native for a `leaf` node, and for the rest still the node's string output
  wrapped as one opaque `Doc::text`, which is what makes nested collections render more
  than once and what `forces_list_break` exists to work around. Moving a node type from
  `formatted` to `leaf` is allowed to change output, but not to make it worse — verify by
  formatting `test-projects/rails` and `test-projects/thor` before and after, diffing the
  trees, and reading every hunk.

  **Layout goes through the `Doc` IR** (`src/format/doc.rs`), the only module that names
  `pretty`. Build a `Doc` out of `text`/`line`/`softline`/`hardline`/`if_break`, mark a
  unit with `group` and a nested level with `indent`, and `render` it against a width;
  the group decides flat-or-broken, which is where all-or-nothing expansion comes from.
  Never reach for `pretty` directly — the wrapper is what keeps a printer swap
  (issue #24) confined to one file. The formatter is mid-migration: everything not yet
  ported still writes strings into `self.output` and gets spliced in as an opaque
  `Doc::text` leaf by `capture_as_doc`, which is why `forces_list_break` and the
  `contains('\n')` checks exist. Both go away as constructs learn to emit `hardline`
  (issues #22, #23).

  **Comments are placed by position, not attached to nodes.** `comment_cursor` walks the
  file's comments in order and whoever owns a source range consumes the ones inside it:
  `fmt_statements_node` takes own-line comments above each statement,
  `emit_trailing_comment_if_same_line` takes one at the end of a header line
  (`class Foo # :nodoc:`), `flush_comments_before` takes those at the bottom of a body
  before `end`/`else`/`when`/`rescue`, and `write_commented_bracketed_items` handles a
  list with comments between its brackets (which forces it to expand). A new construct
  with a body needs the header and bottom-of-body calls, or every comment nested inside it
  is displaced: the cursor is strictly ordered, so one unconsumed comment blocks the rest.
  A comment between operator operands or chain links has no place in the string-based
  emitter, so `write_verbatim_if_commented` copies that one expression through unchanged.

  **`format` returns `Result<String, FormatError>`.** `Syntax` means the source has parse
  errors (never format a tree with holes in it); `AstMismatch` means the output failed
  the AST-equivalence check above. In both cases the caller must leave the source as it
  was; the CLI exits 2.

- `src/lint.rs` — lint rule checking (`check`) and auto-fixing (`fix`), producing
  `Diagnostic`s (rule name, message, line/col).
- `src/config.rs` — config loading. Looks for `.tuxedo.toml`, walking up from the current
  directory if no explicit `--config` path is given; falls back to `Config::default()`.
  Config controls `line_length`, `indent_size`, `quote_style` (format) and
  `enabled`/`disabled` lint rules — deserialized with `deny_unknown_fields`, so an unknown
  config key is a hard error.

### Ruby gem packaging (`ruby/`)

- `tuxedo-r` (the base gem) ships no binary — it's the platform-independent "meta" gem.
- `ruby/rakelib/package.rake` defines, per target platform (see
  `Tuxedo::R::NativePlatforms::TARGETS`), a `gem:<platform>` and `build:<platform>` task
  that compiles the Rust binary (via `cargo build` or, for cross-compiled targets,
  `cross build`) and packages it into a platform-specific gem
  (e.g. `tuxedo-r-arm64-darwin`) under `exe/<platform>/tuxedo`.
- `ruby/exe/tuxedo` — the plain-Ruby script actually installed onto `PATH` (RubyGems
  requires gem executables to be Ruby). It locates the right platform binary next to
  itself and `exec`s it, forwarding args and exit status. If none is bundled, it falls
  back to `TUXEDO_INSTALL_DIR` env var for a self-built binary.
