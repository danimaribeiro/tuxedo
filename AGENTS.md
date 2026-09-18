# CLAUDE.md

Guidance for Claude Code in this repository. It changes only when a **command, a
rule, or a pointer** changes: status lives in the GitHub issues and milestones,
reasoning in the roadmap, how the code works in the module docs (see "Map"). A PR
that fixes a bug or lands a feature does not touch this file.

## What this is

Tuxedo is a fast Ruby formatter and linter written in Rust, aiming to be a drop-in
replacement for RuboCop the way `ruff` is for Python. It is early-stage: the
formatter is being finished first; the linter (`src/lint.rs`) is a stub.

- **Rust crate at the root** (`Cargo.toml`, `src/`): the `tuxedo` binary.
- **`ruby/`**: the `tuxedo-r` gem that wraps and distributes precompiled binaries
  per platform (the "R" is both Ruby and Rust). See `ruby/README.md`.

## The goal: formatting optimized for diff quality

Tuxedo exists to end formatting discussion in code review. It does that by being
in CI with a layout the team chose once, so "move this space" never needs a
round-trip. The layout below is the **default and the recommended one**; it is not
a prohibition on configuration (`quote_style` and an aligned indent style are
planned; see the roadmap). Where more than one layout is defensible, the
tie-breaker is **which one produces the better diff later**:

1. **All-or-nothing expansion.** A collection either fits on one line or every
   element gets its own line. Never bin-pack: packing makes adding or removing one
   element reflow unrelated lines. A Wadler-style `group` gives this for free.
2. **Block indentation.** When something expands, its continuation sits at a fixed
   indent from the start of the statement, never aligned under the opening
   bracket, receiver or keyword. Alignment lets the *name* of a thing control the
   indentation of everything under it, so renaming reindents the whole literal.
3. **No trailing comma** on the last element (matches RuboCop's default and Ruby
   convention, at a small cost on append).

```ruby
ROLES = [                              # not: ROLES = ["administrator", "moderator",
  "administrator",                     #             "contributor", "subscriber"]
  "moderator",
  "contributor",
  "subscriber"
]
```

RuboCop is a **reference, not the target.** It only ever *adds* line breaks;
Tuxedo normalizes in both directions, so byte-equality with it is not chased.

## Commands

```bash
cargo build --release                       # -> target/release/tuxedo
cargo test                                  # all tests, including insta snapshots
cargo test <name>                           # one test
cargo insta review                          # accept changed snapshots (cargo install cargo-insta)
cargo fmt --check                           # CI-enforced
cargo clippy --all-targets -- -D warnings   # CI-enforced
cargo doc --no-deps --document-private-items --open   # the architecture docs

cd ruby && bundle install && bundle exec rake test    # gem tests (Minitest); CI-enforced
```

CI runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, and `bundle exec rake test` from `ruby/`. Run them before calling a
change done.

## Map

Read the module docs, not this file, for how things work. `src/format/mod.rs`
holds the pipeline (parse, one `Doc` for the whole file, render, splice heredocs,
verify) and the formatter's invariants. `src/format/dispatch.rs` is the exhaustive
node-type match; `src/format/doc.rs` is the layout IR and the only file that names
the printer crate; `src/format/verify.rs` is the AST-equivalence check. `src/main.rs`
is the CLI (`format`, `check`, exit codes), `src/walk.rs` the file walk,
`src/write.rs` the atomic write, `src/config.rs` the `.tuxedo.toml` loader.
`ruby/README.md` explains gem packaging. To see the tree a construct parses to:
`echo '<ruby>' | cargo run --release -- format --ast`.

## Measuring a change to the formatter

Fixtures only pin down what someone thought to write. Real projects under
`test-projects/` (gitignored; `scripts/README.md` says how to add one) are the
measure, through `scripts/format-verify.sh <project>` and
`scripts/lint-baseline.sh <project>`; that README explains every number.

The rule: **meaning, parse and idempotence may never regress.** Comment placement
and layout conformance are ratchets that should only go down, and may move a
little either way on a layout change.

The recipe for any layout change: build the binary from `main` and from your
branch, format `test-projects/rails` and `test-projects/thor` with each, `diff -rq`
the two trees and read every hunk. Show before and after in the PR, run
`format-verify.sh` on both, and commit the updated `scripts/results/*.txt` with it.

## Where to pick up work next

`gh issue list`. Issue numbers follow the order of work, so the next thing is
normally the lowest open number; the GitHub milestones say which body of work is
active. Read the issue before planning, and record what you learn back in it.
The roadmap holds the reasoning behind each milestone:
<https://claude.ai/code/artifact/a4eeb67b-2e0c-422a-af19-730c276f1055>.
`scripts/results/` holds the latest saved bug lists (regenerate before trusting).

## Code comments

A comment earns its place by explaining what the code cannot say: why a surprising
choice is right, what breaks if it is undone, which trap the next reader is about
to walk into. The ones in `src/format/` are the house style.

Two things do *not* belong in a comment, because a better place already holds
them:

- **Issue numbers, PR numbers and changelog** (`(issue #123)`, "this used to
  write strings"). Git history records when and why every line arrived and stays
  accurate when a ticket is closed; a comment does not.
- **Notes to whoever picks up the next ticket** ("this goes away once …"). Write
  those in the ticket, where the next person is already looking and where they
  close out with the work. In the code they rot silently.

Describe what is true now. If a workaround exists, say what it works around in the
present tense and leave its removal plan to the issue that owns it.
