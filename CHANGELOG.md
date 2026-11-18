# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- A multiple assignment whose targets do not fit on the line with their `=`
  puts each target on its own line, one indent in, with the value after the
  last one. The targets used to stay on one line past the limit.
- An RSpec compound matcher (`.to change { A }.by(1).and change { B }.by(2)`)
  breaks as one chain, every link one indent in; each matcher used to nest a
  level deeper than the one before it.
- A method chain that is an operand of an `if`, `unless`, `while` or `until`
  condition continues two levels in from its receiver, as an operator spine
  there does.
- A comparison on a method chain that breaks stays on the chain's last line
  (`.count > max`), instead of putting the right side on a line of its own.
- In a command call without parentheses, a `{ }` hash value of the first
  argument, with more arguments after it (`render json: {...}, status:
  :ok`), moves under its key when it does not fit, at the indent of the
  arguments after it. It used to expand on the command's line.
- A command call in `( )` (`return (render json: x, status: :ok)`) breaks
  between its arguments the way it does outside the parentheses. It was
  held to one line, so only its last argument's own brackets could break.
- An arithmetic or comparison spine whose last operand is a call with a
  `do` block (`a + b + list.map do ... end`) puts each operand on its own
  line when the head does not fit, the block body one level in from its
  operand, as `&&`/`||` spines already did.

### Fixed

- A blank line between a file's last statement and the comments after it
  (a `# == Schema Information` block under a model) is kept; it was removed.
- A comment right after `(` (`return(` + newline + `# why`) no longer joins
  the `(` line: the body goes on lines of its own, with `)` below it.
- A comment between a key and its value (`logs:` + newline + `# rubocop:disable`
  + newline + the value) keeps the value on its own line below it; the value
  joined the key and left the comment underneath, uncovered.
- A modifier `if`/`unless` written out as a block keeps the comment that
  ended its line on the body's line, not after `end`.
- A `do ... end` block written on one line and expanded keeps the comment
  after it at the end of the statement; it moved to the `do` line.

## [0.0.3] - 2026-11-17

### Added

- Precompiled `tuxedo-r` gems for macOS (`arm64-darwin`, `x86_64-darwin`),
  arm64 Linux (`aarch64-linux`), musl Linux (`x86_64-linux-musl`,
  `aarch64-linux-musl`) and Windows (`x64-mingw-ucrt`), next to x86_64
  Linux. The GitHub Releases archive is still x86_64 Linux only.

### Changed

- The Linux (glibc) binaries need glibc 2.28 or newer (Debian 10, Ubuntu
  20.04, RHEL 8), down from 2.34.
- A chain on an array or hash literal stays on the closing bracket's line
  when it fits there (`].map { ... }.join(" ")`), instead of breaking
  before every `.`. A literal that fits by itself now expands when the
  whole chain does not fit on one line.
- A wrapped `return` value and an operand of an `&&`/`||` condition
  continue two levels in, as RuboCop's indentation cops expect.
- A `do` block at the end of an `&&`/`||` chain moves to its own line when
  the line before it does not fit; in a block condition, a multi-line
  operand puts every operand on its own line.
- A long `rescue A, B, C => e` list breaks one class per line, like `when`
  values.
- A Sorbet `sig { ... }` stays directly above its `def`, with no blank line
  between them.
- `redo` and `retry` no longer count as guard clauses, matching RuboCop's
  `Layout/EmptyLineAfterGuardClause`: no blank line is added after them,
  and one after a `break if` before them is kept.

### Fixed

- A `( )` body or a multi-line `#{}` holding a `begin`/`rescue`, an
  `if`/`else` or a `do` block keeps its lines; it was joined with `;`.
- A trailing comment in a command call without parentheses stays with the
  argument it follows; it moved to the first argument on that line.
- Comments stay where they were written among `when` values, between `=`
  and its value, and after a one-line endless `def` (which no longer
  expands because of the comment). Trailing whitespace is trimmed from
  comment lines.
- Magic comments only count above the first line of code, so a comment
  that looks like one inside a doc comment no longer gets a blank line
  forced into it. `typed` and `rbs_inline` now count;
  `shareable_constant_value` and `warn_indent` still count anywhere.
- A value after `%` on a backslash-continued string continues from that
  line's indent.
- On Windows, `TUXEDO_INSTALL_DIR` finds `tuxedo.exe`; it looked for
  `tuxedo`, which Windows does not run.

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
