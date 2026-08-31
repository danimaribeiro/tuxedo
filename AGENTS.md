# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Tuxedo is a fast Ruby code formatter and linter written in Rust, aiming to be a drop-in
replacement for RuboCop (the way `ruff` is for Python). It's early-stage and not yet
production-ready. Repo has two parts:

- **Rust crate at the root** (`Cargo.toml`, `src/`) — the `tuxedo` binary itself.
- **`ruby/` directory** — the `tuxedo-r` gem that wraps and distributes precompiled
  `tuxedo` binaries for each platform (the "R" stands for both Ruby and Rust).

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

## Architecture

- `src/main.rs` — CLI entry point (clap-based). Two subcommands: `format` and `check`.
  Both accept paths (or read from stdin when no paths given), walk the filesystem via
  `walk_and_process`, and either rewrite files in place or just report (`--check` /
  without `--fix`).
- `src/walk.rs` — shared file-walking/parallel-processing logic (`walk_and_process`) used
  by both the `format` and `check` commands; respects `.gitignore` unless `--no-ignore`.
- `src/format.rs` — the formatter, built on `ruby-prism` (Ruby's own parser, via the
  `ruby-prism` crate) to parse source into an AST and re-emit formatted code.
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
