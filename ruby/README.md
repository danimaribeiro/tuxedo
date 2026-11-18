# tuxedo-r

[Tuxedo](https://github.com/danimaribeiro/tuxedo) is a fast Ruby code
formatter written in Rust. `tuxedo-r` is the Ruby gem that installs it: each
platform-specific gem carries a precompiled native binary, so no Rust
toolchain is needed. The name is deliberately ambiguous: the "R" stands for
both **Ruby** (the language it formats, and the language of this wrapper) and
**Rust** (the language the `tuxedo` binary itself is written in).

## Installation

```ruby
group :development, :test do
  gem "tuxedo-r", require: false
end
```

```bash
bundle install
bundle exec tuxedo format --check .
```

Without Bundler:

```bash
gem install tuxedo-r
tuxedo format .
```

RubyGems automatically resolves and installs the platform-specific gem that
matches your machine (for example `tuxedo-r-x86_64-linux`). From 0.0.3
there is one for each of these platforms:

| Gem platform | Machine |
| --- | --- |
| `arm64-darwin` | macOS, Apple Silicon |
| `x86_64-darwin` | macOS, Intel |
| `x86_64-linux` | Linux x86_64, glibc 2.28+ (Debian 10, Ubuntu 20.04, RHEL 8) |
| `aarch64-linux` | Linux arm64, glibc 2.28+ |
| `x86_64-linux-musl` | Linux x86_64, musl (Alpine) |
| `aarch64-linux-musl` | Linux arm64, musl (Alpine) |
| `x64-mingw-ucrt` | Windows x64 (RubyInstaller 3.1+) |

0.0.2 and earlier ship a binary for x86_64 Linux (glibc) only. On any
other platform, see [Unsupported platforms](#unsupported-platforms). The
[documentation](https://danimar.dev/tuxedo/docs.html) covers what
the formatter does, its command line and its configuration.

### Requirements

Ruby 3.1 or newer and RubyGems 3.3.22 or newer (`gem update --system`).
`required_ruby_version` is the Ruby that runs the `tuxedo` shim, not the
Ruby syntax `tuxedo` accepts. On Alpine and other musl hosts, RubyGems
older than 3.3.22 resolves the glibc gem, whose binary will not run.

### Unsupported platforms

If a precompiled binary isn't available for your platform yet, set
`TUXEDO_INSTALL_DIR` to a directory containing a `tuxedo` executable
(`tuxedo.exe` on Windows; for example, one you built yourself from the
[Rust crate](https://github.com/danimaribeiro/tuxedo)):

```bash
export TUXEDO_INSTALL_DIR=/path/to/tuxedo/target/release
bundle exec tuxedo format --check .
```

## How it works

- The `tuxedo-r` gem itself ships no binary — it's the platform-independent
  "meta" gem.
- `tuxedo-r-<platform>` gems (built by `rake package`, see
  [`rakelib/package.rake`](https://github.com/danimaribeiro/tuxedo/blob/main/ruby/rakelib/package.rake))
  each bundle a `tuxedo` binary compiled for that target triple, under
  `exe/<platform>/tuxedo` (`tuxedo.exe` on Windows).
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
