# Tuxedo

A fast Ruby code formatter, written in Rust. A linter is planned; the
long-term aim is a drop-in replacement for
[RuboCop](https://github.com/rubocop/rubocop), the way
[`ruff`](https://github.com/astral-sh/ruff) is for Python.

**Documentation: <https://danimar.dev/tuxedo/>**

RuboCop is thorough but, being written in Ruby, can be slow on large
codebases. Tuxedo gives the same clean, opinionated formatting at a fraction
of the runtime, by leveraging Rust.

On the Rails repository (3,520 Ruby files, commit `6b52f38`), `tuxedo format
--check` takes **2.8 s** on 8 threads and 10.9 s on one. That is tuxedo 0.0.1
on a laptop (i7-1165G7, WSL2), with a warm file cache and no writes, best of
five runs. No ratio against RuboCop is claimed: RuboCop's own time on the same
tree varied fourfold across saved runs, so a ratio would measure the machine.

## Status

The formatter is usable; there is no linter yet. Every file Tuxedo formats is
checked to still mean the same code (see [Safety](#safety)). Across five real
projects (Rails, Thor, ruby/spec, Discourse and Homebrew, 22,779 files) no file
was refused for a changed meaning, none was broken, and none changed on a
second pass.

Tuxedo is at version 0.0.1: expect the layout, and the options that tune it,
to keep moving.

## Installation

As a Ruby gem, which wraps a precompiled native binary (no Rust toolchain
needed):

```ruby
group :development, :test do
  gem "tuxedo-r", require: false
end
```

```bash
bundle install
bundle exec tuxedo format --check .
```

Without Bundler, `gem install tuxedo-r` puts `tuxedo` on your `PATH`. See
[`ruby/README.md`](ruby/README.md) for supported platforms and requirements.

Or download a prebuilt archive for your platform from the
[GitHub Releases](https://github.com/danimaribeiro/tuxedo/releases) page.

It is not on crates.io. The name `tuxedo` there belongs to an unrelated crate,
so `cargo install tuxedo` would install someone else's project. To build this
one, see [Development](#development).

## Quickstart

```bash
tuxedo format .                 # rewrite files in place
tuxedo format --check .         # list files that would change; exit 1 if any
tuxedo format --diff app/       # print a unified diff instead of writing
cat a.rb | tuxedo format        # format stdin to stdout
```

## Command line

`tuxedo format [OPTIONS] [PATHS]...` is the whole interface. With no paths it
reads one buffer from stdin and writes the result to stdout.

| Option | Effect |
| --- | --- |
| `--check` | Report files that would change (`would reformat: PATH`) and write nothing |
| `--diff` | Print a unified diff of what would change and write nothing |
| `--stdin-filename PATH` | Name the stdin buffer `PATH` in messages, so an editor's diagnostics point at a real file |
| `--config PATH` | Use this config file instead of searching for `.tuxedo.toml` |
| `-j`, `--jobs N` | Number of threads (default: automatic) |
| `-q`, `--quiet` | Omit the per-file lines and the summary of a write run. Errors, and the summary of `--check` or `--diff`, still print |
| `--no-ignore` | Do not honour `.gitignore` |
| `--ast` | Dump the Prism syntax tree of stdin, for debugging |

| Exit code | Meaning |
| --- | --- |
| 0 | Nothing to do |
| 1 | `--check` or `--diff` found a file that would change |
| 2 | A file could not be formatted: a syntax error, a changed meaning, an unreadable path or a failed write. A usage error also exits 2 |

A file that fails is left byte-identical.

### What gets formatted

Given a directory, Tuxedo formats files ending in `.rb`, `.rake`, `.gemspec`
or `.ru`, plus files named `Rakefile` or `Gemfile`. It skips hidden files,
honours `.gitignore` (turn that off with `--no-ignore`), and skips whatever the
config's `exclude` matches. A path named on the command line is formatted even
if `exclude` matches it.

## The goal: formatting optimized for diff quality

Tuxedo exists to end formatting discussion in code review. "Move this space"
is not feedback worth a round-trip, and the way to make it stop is a
formatter whose output is not up for debate — so Tuxedo is opinionated by
design. The layout below is the default and the recommended one; the few
options that exist (see [Configuration](#configuration)) are for a team that
has already chosen otherwise.

Where more than one layout is defensible, the tie-breaker is **which one
produces the better diff later**:

- **All-or-nothing expansion.** A collection either fits on one line or every
  element gets its own line. Tuxedo never bin-packs, because packing makes
  adding or removing one element reflow an arbitrary number of unrelated
  lines.
- **Block indentation.** When a collection expands, the bracket sits alone on
  its own line and elements sit at a fixed indent. Aligning under the opening
  bracket instead would let the *name* of a thing control the indentation of
  every element, so renaming a variable would reindent the whole literal.
- **No trailing comma** on the last element.

```ruby
ROLES = [
  "administrator",
  "moderator",
  "contributor",
  "subscriber"
]
```

Tuxedo does not change one construct into another. An endless method
(`def name = expr`) stays endless while it fits on the line and expands to
the block form when it does not; a block `def` is never made endless.

## Configuration

`.tuxedo.toml`, found by walking up from the current directory (or named with
`--config`). Every key is optional and an unknown one is an error. These are
all the keys the formatter and the file walk read:

```toml
exclude = ["vendor/", "/db/schema.rb"]  # skipped by format, on top of .gitignore

[format]
line_length = 120                  # column a line is broken to fit
indent_size = 2                    # spaces per indent level
hash_brace_space = "space"         # "space" { a: 1 } or "no_space" {a: 1}
access_modifier_style = "indent"   # "indent" or "outdent" for private/protected/public
quote_style = "double"             # "double", "single" or "preserve"
```

`exclude` takes gitignore-style globs, matched against the path from the
directory the config file sits in (`vendor/` at any depth, `/db/schema.rb`
only at the top). `--no-ignore` turns `.gitignore` off but not `exclude`, and
a file named on the command line is formatted even if `exclude` matches it.

`quote_style` rewrites a plain string, a quoted symbol and a quoted hash key
(`'a': 1`) to the chosen quote, and only when that loses nothing: a literal
that the other quote would read as an escape or interpolation, or that holds
that quote, keeps what was written. `%q`, `%w`, `?a`, heredocs, and anything
inside a `#{}` are never touched, and neither is an implicit concatenation
whose parts would not all end up with the same quote. `preserve` rewrites
nothing.

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `exclude` | list of globs | `[]` | Paths a walk skips, on top of `.gitignore` |
| `format.line_length` | integer | `120` | Column a line is broken to fit |
| `format.indent_size` | integer | `2` | Spaces per indent level |
| `format.hash_brace_space` | `"space"`, `"no_space"` | `"space"` | `{ a: 1 }` or `{a: 1}` |
| `format.access_modifier_style` | `"indent"`, `"outdent"` | `"indent"` | Where `private`, `protected` and `public` sit in a class body |
| `format.quote_style` | `"double"`, `"single"`, `"preserve"` | `"double"` | The quote plain strings and quoted symbols are written with |

The config is found by walking up from the **current working directory**, not
from the file being formatted. In a monorepo, run Tuxedo from the directory
that holds the `.tuxedo.toml` you mean, or pass `--config`.

## Turning the formatter off

`# tuxedo:off` and `# tuxedo:on`, each on a line of its own, copy the
statements between them through exactly as written. It is for what a
formatter cannot improve, such as a hand-aligned table:

```ruby
# tuxedo:off
MATRIX = [
  [1,   0,   0],
  [0,   1,   0],
  [0,   0,   1]
]
# tuxedo:on
```

A region with no `on` runs to the end of the body it opened in, and an `off`
before the first statement holds the whole file. The markers go between
statements. An `on` with no `off` before it, an `off` inside a region that is
already off, and one in the middle of a literal (a hash, an argument list)
are errors, reported like a syntax error, and the file is left untouched.

## Continuous integration

With `tuxedo-r` in your `Gemfile`, a GitHub Actions job that fails when a file
is not formatted:

```yaml
jobs:
  tuxedo:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: ruby/setup-ruby@v1
        with:
          ruby-version: "3.3"
          bundler-cache: true
      - run: bundle exec tuxedo format --check .
```

## Editor integration

An editor that can pipe a buffer through a command can format with Tuxedo:
`tuxedo format --stdin-filename PATH` reads the buffer on stdin and writes the
formatted buffer to stdout. For Neovim with
[conform.nvim](https://github.com/stevearc/conform.nvim):

```lua
require("conform").setup({
  formatters_by_ft = { ruby = { "tuxedo" } },
  formatters = {
    tuxedo = {
      command = "tuxedo",
      args = { "format", "--stdin-filename", "$FILENAME" },
      stdin = true,
    },
  },
})
```

Emacs, with [reformatter](https://github.com/purcell/emacs-reformatter):

```elisp
(reformatter-define tuxedo-format
  :program "tuxedo"
  :args (list "format" "--stdin-filename" (or (buffer-file-name) "buffer.rb")))
```

VS Code has no built-in hook for an external formatter; an extension such as
Custom Local Formatters can run the same command.

## How it differs from RuboCop

RuboCop is a reference, not the target. RuboCop is a linter, so it only ever
*adds* line breaks — it has no mandate to overrule an author who broke a line
by hand. Tuxedo is a formatter and normalizes in both directions: given
`x = [1,\n     2]`, RuboCop block-expands it while Tuxedo collapses it to
`x = [1, 2]`. Both are right by their own rules, so matching RuboCop
byte-for-byte is not a goal.

Tuxedo only formats: it has no linter, and RuboCop's non-layout cops are still
yours to run. To run both, format first, then lint. A few RuboCop layout cops
default to a style that Tuxedo's block indentation deliberately does not
follow (for example `Layout/ArgumentAlignment`), so set those to their
fixed-indentation styles. [`scripts/format-verify.rubocop.yml`](scripts/format-verify.rubocop.yml)
is the RuboCop layout configuration that Tuxedo's output satisfies.

## Safety

Tuxedo never writes a file it cannot format faithfully. After formatting, it
parses the output again and compares the syntax tree with the original's
(`src/format/verify.rs`). If they differ, the file is refused, left exactly as
it was, and the run exits 2.

The same check runs over real projects: on Rails, Thor, ruby/spec, Discourse
and Homebrew (22,779 files) no file was refused, no file that parsed before
failed to parse after, and formatting a second time changed nothing.
`scripts/format-verify.sh <project>` reproduces the numbers, and
`scripts/results/` holds the latest run for each project.

## Ruby syntax

Tuxedo parses every file as the latest Ruby syntax Prism supports, whatever
Ruby runs it. Code that relies on syntax removed in Ruby 3.4, such as keyword
or block arguments in an index assignment (`a[k: 1] = v`), is rejected as a
syntax error.

## Development

This repository contains two things:

- The Rust crate at the root (`Cargo.toml`, `src/`) — the `tuxedo` binary
  itself.
- The `ruby/` directory — the `tuxedo-r` gem that wraps and distributes
  that binary (the "R" is both Ruby and Rust). See [`ruby/README.md`](ruby/README.md)
  for details on how packaging works.

### Building the binary

```bash
cargo build --release
./target/release/tuxedo
```

### Running tests

CI runs these four; run them before sending a change:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cd ruby && bundle exec rake test
```

## License

MIT
