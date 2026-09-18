# Red/green trees, formatters, linters — notes

Working notes from exploring whether/how a lossless red/green syntax tree could sit
under tuxedo. Nothing here is wired into the real formatter or linter (see
`CLAUDE.md`) — this is the standalone exploration in `playground/`.

Live artifact (interactive, includes a real embedded tree-sitter parse and
step-through pseudocode for every algorithm below):
https://claude.ai/code/artifact/f1a3aa50-d9e5-4501-90b9-62c5715fb774

Code: `examples/cst_playground.rs` (`cargo run --release --example cst_playground`
regenerates `playground/output.json`, which the artifact embeds).
Fixtures: `playground/conditionals.rb`, `formatter_before.rb`, `alignment.rb`.

## 1. Red/green trees

Origin: Roslyn (C#'s compiler platform). Two views of the same syntax tree:

- **Green tree** — immutable, structurally shared, knows only each node's own
  **width**. No absolute position, no parent pointer. Cheap to build, cheap to
  reuse subtrees across edits.
- **Red tree** — a thin, ephemeral wrapper computed *on navigation*: walk down
  from the root, accumulating widths into an absolute offset, remembering the
  parent chain passed through. Nothing about it is stored; ask again and it's
  rebuilt.

**Trivia** — the term (also from Roslyn) for anything syntactically meaningless
but still present in the text: whitespace, newlines, comments. Contrasted with
**tokens** (keywords, identifiers, punctuation) and **nodes** (structural
groupings of tokens/nodes).

## 2. Prism's AST is lossy — and how we made it lossless

`ruby-prism` (Ruby core's own parser, C since Ruby 3.3) gives every node a
precise byte `Location`, plus granular sub-locations for many keywords/operators
(`if_keyword_loc`, `then_keyword_loc`, `opening_loc`, ...). It does **not** model
whitespace or newlines as tree nodes at all; comments are separate
(`ParseResult::comments()`), outside the AST.

The trick (`cst_playground.rs`, `assemble()`/`push_trivia()`): collect every
known location (node spans + all the granular keyword/operator locations) into
a sorted list. Prism's own locations never overlap, so **any byte range left
over between them must be whitespace or a comment** — reconstruct it as a
synthetic `Trivia` leaf. Verified byte-exact and gap-free against the real
source for every fixture used here.

One real bug hit along the way: prism mirrors a single `end` keyword's location
onto *every* level of an `if`/`elsif`/`else` chain (and `unless`/`else`) —
naively taking it at each level double-counts the same physical token. Fix:
only take `end_keyword_loc()` at the chain's actual terminal clause (the one
with no `subsequent`/`else_clause`).

## 3. Formatting — pure green tree, no red tree at all

`format`/`format_block` in the playground's pseudocode walk *only* the green
tree. `depth` is not tree data — it's a plain integer threaded through the
recursion, incremented by one each time the walk steps into a nested block.
**No position, no parent pointer, no red tree anywhere in this algorithm.**

The only place old trivia gets *read* (not kept) is inside `format_block`: it
scans a discarded whitespace run's newline count to decide "was there a blank
line here?" — then throws the actual text away and re-emits canonical spacing.

This generalizes badly on purpose: `match node.kind` needs one arm per Ruby
construct (conditionals, loops, collections, defs, classes, ...), forever
growing — which is why tuxedo's real `src/format/` is split into
`control_flow.rs`, `collections.rs`, `classes.rs`, `defs.rs`, etc. rather than
one function.

## 4. Linting — green tree to match, red tree to report

`check_if_unless_modifier` (modeled on `Style/IfUnlessModifier`): the *match*
— single-statement body, no `elsif`/`else` — is pure green-tree shape testing:
kind + child count, true regardless of where the code sits in the file. Only
the reported location (`line_col`, a red-tree computation) and the literal
source text sliced for the replacement need absolute offsets.

**Pattern-matching DSLs** (RuboCop's `NodePattern`, tree-sitter's query
language, ESLint's CSS-like selectors) only ever replace the green-tree half —
the shape test. None of them touch the red-tree half; a rule that needs a
position or literal text is back to imperative code in the match callback
regardless of which DSL matched it.

**Scaling a linter**: our pseudocode runs one full, independent walk per rule.
RuboCop instead runs **one shared traversal** for all ~400 cops, dispatching to
each cop's registered `on_if`/`on_send`/etc. callback per node — turning N full
walks into one walk with N cheap checks. Worth building once tuxedo's linter
has more than a handful of rules.

## 5. Semantic alignment — *only* red-tree work

`check_alignment` (modeled on RuboCop's multiline-string/operator alignment
cops): compares one node's **column** to another node's column. Column doesn't
exist until you've walked from the root to get an absolute offset and found
the preceding newline — there is no green-tree shortcut here. Contrast with
the linter rule above, which touches the red tree once (to report); alignment's
entire job *is* comparing red-tree positions to each other. Take the red tree
away and this rule has nothing left to check.

Concretely verified: our `alignment.rb` fixture's `message << "a" \` chain has
anchor column 11 (from `message << ` = 11 chars) and both continuation lines
sit at column 12 — flagged and auto-corrected by rewriting only the leading
whitespace before each misaligned line (direct string splice on the raw
source, using the diagnostic's byte offset — not a lookup into any trivia
node).

## 6. Tree-sitter — real findings, not assumptions

Downloaded and ran the actual npm packages (`web-tree-sitter` 0.27.0,
`tree-sitter-ruby` 0.23.1) against `conditionals.rb`, in Node first, then
embedded live in the artifact (real WASM, no simulation).

- **"Fast because C" is the wrong framing** — prism is also C (it's CRuby's own
  parser since 3.3). Tree-sitter's real distinguishing feature is
  **incremental re-parsing**: reuse unchanged subtrees after a small edit.
  Prism has no incremental API at all — full one-shot parse every time,
  optimized instead for diagnostic fidelity.
- **Language-agnostic core, per-language grammar**: `web-tree-sitter.wasm` is
  the generic incremental engine, zero Ruby-specific code. `tree-sitter-ruby`
  is a separate, independently-authored `grammar.js` compiled to its own
  `.wasm`, plugging into the same generic runtime interface. Every language
  just supplies a grammar; the engine never changes.
- **Its CST is not lossless out of the box either.** Empirically: 17 byte
  ranges in `conditionals.rb` are covered by *no* leaf node — all plain
  whitespace. Comments become real `extra` nodes in this grammar, but spaces
  and most newlines don't. Reconstructing exact source text from a
  tree-sitter tree needs the *same* gap-filling trick we built for prism.
- **A node's span can start before its own first printed child.** The `then`
  node wrapping an `if`'s consequence has range `[9-22]` while its only child
  (`call`) starts at `12` — the grammar silently attributes the leading
  `"\n  "` to `then`'s boundary without exposing it as a token. Different
  grammars make different, non-obvious calls about where structure ends and
  trivia begins.
- **Converting prism's AST to tree-sitter's CST shape is not a well-posed
  operation.** They're not a source/target pair — two independently-authored
  grammars for the same language, each making its own node-shape decisions.
  Reproducing tree-sitter-ruby's shape from prism's output means making all
  the same grammar-author decisions by hand — i.e., authoring a grammar, not
  writing a converter.
- Practical implication for a given tool: gaps don't matter for anything that
  only needs **positions** (alignment, `IfUnlessModifier`-style rules — both
  verified to work identically against tree-sitter's tree, node.startIndex is
  exact either way). Gaps matter enormously for a **formatter**, which must
  enumerate and classify every byte of trivia to re-emit it — tree-sitter
  gives you no shortcut there over what we built for prism.

## 7. Why prism for tuxedo specifically

Prism is Ruby core's own parser — same one CRuby, RuboCop (mid-migration), and
Sorbet lean on for semantic correctness against actual Ruby parsing behavior
(precedence, pattern-matching edge cases, string-literal weirdness). That
correctness comes from the people who define Ruby's grammar, not from an
externally-maintained grammar tracking Ruby's evolution from outside.

The capability given up by not using tree-sitter — incremental re-parsing on
every keystroke — is one a batch CLI formatter/linter (run once per file, per
invocation) was never going to use. Not paying for a feature you don't need.

This isn't a novel strategy either: RuboCop itself doesn't use a separate CST
library — it wraps its AST's precise source ranges plus its own comment
bookkeeping, exactly the "reconstruct trivia over a semantically-correct AST"
approach built here.

## 8. How a formatter and a linter reconcile (so `format | lint` doesn't fight itself)

The failure mode this guards against: format a file, then lint it, and the
linter complains about something the formatter just produced (or undoes what a
lint `--fix` just did). The fix isn't keeping two independent implementations
manually in sync — that drifts. It's a **strict partition of authority**:

- **Prettier/ESLint model**: two separate tools; `eslint-config-prettier`
  disables every ESLint rule that overlaps with anything Prettier decides, so
  there's no shared territory left to disagree over.
- **RuboCop/ESLint's own `--fix` model**: one engine, one rule config; "format"
  is just fix-mode for a subset of rules — config is inherently shared because
  it's literally the same rule definitions running in a different mode.

tuxedo has chosen the first model (per `CLAUDE.md`: "opinionated by design,
takes no per-project style config" for its formatting choices) — and it's
*already* correctly drawing this line in the real code, not just in theory.
`src/format/control_flow.rs::fmt_if_node` decides modifier-vs-block form purely
from what the source already is (`node.end_keyword_loc().is_none()`) — it never
converts one shape into the other based on line length. That decision is left
entirely to a future `Style/IfUnlessModifier`-style lint rule, which means:

- `format(x)` never changes whether something is a modifier, so if `x` already
  violates the rule, `format(x)` still violates it identically — no new
  violation, nothing masked.
- If `lint --fix` later rewrites block → modifier, a subsequent `format` finds
  `is_modifier` now true and just preserves it. No fight.

Config is already unified for the part that's genuinely cross-cutting:
`Config` (`src/config.rs`) holds both `FormatConfig` (`line_length`,
`indent_size`) and `LintConfig` in one struct from one
`.tuxedo.toml` — a lint fixer reads `config.format.*` from the same place
format does, no separate config to reconcile.

The remaining discipline is about **fixer output**, not config:

- A fix that only rearranges/deletes *existing verbatim* text (e.g.
  `IfUnlessModifier`'s `body_text + " " + keyword + " " + predicate_text`)
  can't drift from the formatter — it never invents new formatting, so
  `format` afterward is a no-op by construction.
- A fix that has to *render* something new (requoting a string, normalizing
  `=>` to `:`, rewrapping a long chain) must call the **same rendering
  functions** the formatter uses (`fmt_string_node`, etc.), not a hand-rolled
  printer — otherwise the two printers silently diverge the moment someone
  changes a style default in only one place.

General rule for adding anything new: every time the formatter gains an
opinion, check it against every lint rule's territory — if they'd overlap,
pick one owner (usually: pure `Layout`-shaped decisions → formatter, no
corresponding lint rule at all; `Style`-shaped reshaping decisions → lint,
formatter provably leaves it alone). Verify with a `format-verify.sh`-style
ratchet: run every lint rule against `format(x)` for the real-world corpus and
assert zero relevant offenses, the same idempotence discipline already used
for the formatter alone.
