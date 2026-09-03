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
   why the formatter is migrating to `pretty::RcDoc`.

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
`test-projects/<name>/source`. Then:

```bash
scripts/format-compare.sh <name>              # format with tuxedo + autocorrect with
                                               # rubocop (Layout cops only), diff the two,
                                               # list files that differ
scripts/format-diff.sh <name> <relative/path.rb>   # show the diff for one mismatching file
```

`format-compare.sh` writes two working copies next to the source
(`test-projects/<name>/tuxedo` and `.../rubocop`, both regenerated on each run) and compares
them file-by-file. Comparison is scoped to RuboCop's `Layout` department (whitespace,
indentation, blank lines) rather than the full `Style`/`Lint` cop set, since tuxedo's
formatter is a formatting tool, not a full linter — `Style` cops rewrite idiom choices
(quote style, `.each` vs `.map`, etc.) that aren't formatting concerns. rubocop also runs
with `-c scripts/format-compare.rubocop.yml`, ignoring the project's own `.rubocop.yml`
(that file starts from rubocop's defaults and only relaxes `AllowForAlignment`) — tuxedo has no
per-project style config yet, so rubocop's own defaults are the only fair baseline (a
project's local overrides, like Thor's `AllCops: DisabledByDefault: true`, can silently
turn off Layout cops rather than customize them, which produced a misleading reference
full of un-corrected code before this was added).

`format-compare.sh` also prints wall-clock timing for both formatters and the resulting
speedup factor.

### `format-verify.sh` — the metric that actually matters

```bash
scripts/format-verify.sh <name>
```

`format-compare.sh` measures *similarity* to RuboCop, which is capped below 100% for the
reason given under "The goal" above. `format-verify.sh` measures the properties Tuxedo
must actually hold, and each of them is reachable at 100%:

1. **Parse.** Output is still valid Ruby, counted as regressions against the source: a
   file that parsed before formatting and does not parse after is a bug, full stop.
   Files that never parsed (ERB templates and the like under a `.rb` name) are excluded
   rather than counted as failures. **This is the highest-priority number** — a parse
   regression means Tuxedo silently corrupted working code, and the script exits non-zero
   on any. Byte-diffing against RuboCop cannot see this: corrupted output just shows up
   as one more "file that differs."

2. **Idempotence.** `format(format(x)) == format(x)`. A formatter that keeps changing its
   mind produces churn in every diff.

3. **Layout conformance.** `rubocop --only Layout` reports no offenses on Tuxedo's
   output — a *subset* relation ("everything Tuxedo emits, RuboCop accepts"), not
   equality. It runs against `scripts/format-verify.rubocop.yml`, which encodes the three
   properties above rather than RuboCop's stock defaults; the six `*LineBreak`/
   `*LineBreaks` cops that forbid bin-packing all ship disabled by default, so they have
   to be turned on explicitly. Files that fail the parse check are excluded from this
   pass, since `Lint/Syntax` would otherwise drown out the Layout signal.

All three are **ratchets, not pass/fail bars.** Tuxedo is early; the numbers start high
and should only ever go down. Record them before a change and compare after.

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
