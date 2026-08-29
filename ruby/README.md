# tuxedo-r

A self-contained [`tuxedo`](../) executable — a Ruby formatter/linter written in
Rust — wrapped up in a Ruby gem, distributed as a set of platform-specific gems, each containing a
precompiled native binary, plus a tiny Ruby script that locates and execs it.

The name is deliberately ambiguous: the "R" stands for both **Ruby** (the
language it formats, and the language of this wrapper) and **Rust** (the
language the `tuxedo` binary itself is written in).

No Rust toolchain is required to install or run `tuxedo` this way.

## Installation

```ruby
group :development, :test do
  gem "tuxedo-r", require: false
end
```

```bash
bundle install
bundle exec tuxedo
```

If you're not using Bundler:

```bash
gem install tuxedo-r
tuxedo
```

RubyGems automatically resolves and installs the platform-specific gem that
matches your machine (e.g. `tuxedo-r-arm64-darwin`).

### Unsupported platforms

If a precompiled binary isn't available for your platform yet, set
`TUXEDO_INSTALL_DIR` to a directory containing a `tuxedo` executable (for
example, one you built yourself from the [Rust crate](../)):

```bash
export TUXEDO_INSTALL_DIR=/path/to/tuxedo/target/release
bundle exec tuxedo
```

## How it works

- The `tuxedo-r` gem itself ships no binary — it's the platform-independent
  "meta" gem.
- `tuxedo-r-<platform>` gems (built by `rake package`, see
  `rakelib/package.rake`) each bundle a `tuxedo` binary compiled for that
  target triple, under `exe/<platform>/tuxedo`.
- The `exe/tuxedo` script installed onto your `PATH` is plain Ruby (RubyGems
  requires gem executables to be Ruby scripts) — it just finds the right
  binary for the current platform and `exec`s it, forwarding all arguments and
  the exit status.

## Development

```bash
cd ruby
bundle install
bundle exec rake test

# Build the tuxedo binary for every supported platform (requires `cross`
# for cross-compiled targets: cargo install cross)
bundle exec rake build:all

# Package all platform gems into pkg/
bundle exec rake package
```
