# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.1] - 2026-09-23

### Added

- Ruby formatter core: all-or-nothing collection expansion, block
  indentation, no trailing comma on the last element.
- `tuxedo format` command, with `--check` and `--diff` to report without
  writing, stdin support, `.gitignore` and `--no-ignore` handling, and an
  `exclude` list.
- `.tuxedo.toml` configuration: `line_length`, `indent_size`,
  `hash_brace_space`, `access_modifier_style`, `quote_style`.
- `# tuxedo:off` / `# tuxedo:on` region markers for hand-formatted code.
- AST-equivalence verification: every formatted file is re-parsed and
  compared against the original before it is written.
- Distributed as the `tuxedo-r` gem, with precompiled binaries for macOS
  (arm64, x86_64), Linux (x86_64, aarch64; glibc and musl) and Windows
  (x64).
