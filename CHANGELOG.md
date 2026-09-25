# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.2] - 2026-09-24

### Changed

- An empty `class`, `module` or `def` is written on one line while it
  fits (`class NotFound < Error; end`, `def reset; end`), however it was
  written. One with a comment inside keeps its two lines.
- A `when` or `in` branch written `when x then y` stays on its line while
  it fits; one that outgrows the line is written out on its own. `if a then
  b end` is still written out.
- A run of guard clauses (`return x unless y`) is no longer split by blank
  lines; the blank line goes after the last guard.
- A blank line between the elements of an expanded array or hash is kept,
  as it is between statements. In a call's arguments, only one between two
  keyword arguments is kept, which is what RuboCop accepts.
- `x << call(...)` is laid out the way `x = call(...)` is: `x << call(`
  stays on the line and the arguments break inside the parentheses.
- An index read in a method chain (`doc.xpath(q)[i].text`) is a link of
  the chain and breaks the way `.fetch(i)` does.
- A method chain whose receiver is an array literal or a call without a
  receiver breaks before every `.` when the receiver has to expand, instead
  of keeping the chain on the closing bracket.

### Fixed

- An overlong chain starting with a `::` call (`Nokogiri::XML(doc).at(x)`)
  was left unformatted: breaking before `::XML` would have changed the
  program.
- A comment written right under a method's `end` (`# rubocop:enable`) was
  separated from it by a blank line when another statement followed.
- A comment after a one-line `when x then y` branch moved to the `when`
  line when the branch was written out.
- The README said Tuxedo never changes one construct into another; it
  now lists the constructs it rewrites.

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
- Distributed as the `tuxedo-r` gem and as a standalone archive on GitHub
  Releases, with a precompiled binary for x86_64 Linux (glibc) only. On any
  other platform the gem runs a `tuxedo` you build yourself, through
  `TUXEDO_INSTALL_DIR`.
