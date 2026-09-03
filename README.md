# Tuxedo

A fast Ruby code formatter and linter, written in Rust — aiming to be a
drop-in replacement for [RuboCop](https://github.com/rubocop/rubocop), the
way [`ruff`](https://github.com/astral-sh/ruff) is for Python.

## Status

Early days — Tuxedo is a work in progress and not yet ready for production
use.

## Why

RuboCop is thorough but, being written in Ruby, can be slow on large
codebases. Tuxedo aims to provide the same clean, opinionated formatting and
linting experience at a fraction of the runtime cost, by leveraging Rust.

## The goal: formatting optimized for diff quality

Tuxedo exists to end formatting discussion in code review. "Move this space"
is not feedback worth a round-trip, and the way to make it stop is a
formatter whose output is not up for debate — so Tuxedo is opinionated by
design, and takes no per-project style config for these choices.

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

RuboCop is a reference, not the target. RuboCop is a linter, so it only ever
*adds* line breaks — it has no mandate to overrule an author who broke a line
by hand. Tuxedo is a formatter and normalizes in both directions: given
`x = [1,\n     2]`, RuboCop block-expands it while Tuxedo collapses it to
`x = [1, 2]`. Both are right by their own rules, so matching RuboCop
byte-for-byte is not a goal.

## Installation

Tuxedo is distributed as a Ruby gem wrapping a precompiled native binary — no
Rust toolchain required. See [`ruby/README.md`](ruby/README.md) for
installation and usage instructions:

```ruby
group :development, :test do
  gem "tuxedo-r", require: false
end
```

```bash
bundle exec tuxedo
```

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

```bash
cargo test
```

## License

MIT
