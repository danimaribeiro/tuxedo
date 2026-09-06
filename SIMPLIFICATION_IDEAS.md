# Formatter simplification ideas

Notes from reviewing `src/format/mod.rs` and `src/format/assignment.rs` (plus a skim of the
other files) for structural duplication. Ideas only — nothing here has been applied.

## 1. The giant `impl Visit` block in `mod.rs` (lines ~610-1181, ~570 lines)

Every entry is the same shape:

```rust
fn visit_x_node(&mut self, node: &ruby_prism::XNode<'_>) { self.fmt_x_node(node); }
```

A declarative macro would collapse this to a short table:

```rust
macro_rules! delegate_visits {
    ($($visit_fn:ident, $Node:ident, $fmt_fn:ident;)*) => { $(
        fn $visit_fn(&mut self, node: &ruby_prism::$Node<'_>) { self.$fmt_fn(node); }
    )* };
}
```

~570 lines -> ~90 lines, zero behavior change, and it makes the visit-fn <-> fmt-fn mapping
explicit instead of implicit in matching names.

## 2. `assignment.rs` — the write-node family

Five target kinds (local/instance/global/class variable, constant) x four write flavors
(`=`, op`=`, `||=`, `&&=`) produce ~18 functions that are byte-identical except for the node
type and the operator string, e.g.:

```rust
self.write_assignment(node.name().as_slice(), "=", &node.value());
```

A macro like `simple_write!(fmt_local_variable_write_node, LocalVariableWriteNode, "=");`
(plus a variant for the operator-write flavor that pulls from `binary_operator_loc()`) would
turn ~150 lines into a short table, same idea as #1.

## 3. Repeated operator-extraction line

This appears ~7 times, always the same:

```rust
let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
```

Worth a one-line helper: `fn operator_str(loc: Location) -> &str`.

## 4. Repeated save/toggle-inline/restore scaffolding

Shows up in `write_assignment_to`, `fmt_multi_write_node`, `write_call_write`,
`write_index_write`, `write_leaf`:

```rust
let inline = self.inline;
if !inline { self.write_indent(); }
self.inline = true;
// ... render content ...
if !inline { self.output.push('\n'); }
```

This is the "am I a statement or a sub-expression" boilerplate, hand-duplicated at each site.
A single wrapper, e.g. `fn as_statement(&mut self, render: impl FnOnce(&mut Self))`, would
remove ~5 copies and reduce the chance of one of them getting the save/restore wrong — this
exact pattern is the shape of bug the recent "corrupting output" fixes were about.

## 5. `write_call_write` / `write_index_write` are near-duplicates

Both do "write receiver, write LHS syntax, write ` op= `, write value", differing only in how
the LHS renders (`receiver.attr` vs `receiver[args]`). Could share one function parameterized
by a closure that renders the LHS.

## 6. Target node formatters

`fmt_local_variable_target_node`, `fmt_instance_variable_target_node`,
`fmt_class_variable_target_node`, `fmt_global_variable_target_node`,
`fmt_constant_target_node` are all just `self.write_bytes(node.name().as_slice())` — same
macro-table treatment as #1/#2.

## Bigger picture

Ties into the existing `[[feedback_convert_push_str_to_doc]]` direction: most of the
duplication in #4-#6 exists *because* the imperative `push_str` + manual `inline`/`indent`
bookkeeping forces every write-site to re-derive "am I inline or a statement, do I need a
leading indent, do I need a trailing newline." As more of `assignment.rs`/`calls.rs` migrates
onto `Doc` (where that bookkeeping is implicit in how the `Doc` gets composed/nested), a chunk
of #4-#6 would likely disappear on its own rather than needing a dedicated refactor.

## Suggested order

1. #1 and #2 — mechanical macros, near-zero risk, biggest line-count reduction.
2. #4 and #5 — real behavioral unification, needs test coverage before/after.
3. #3 and #6 — quick opportunistic cleanups, do alongside whichever file they touch.
