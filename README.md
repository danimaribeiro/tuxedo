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
