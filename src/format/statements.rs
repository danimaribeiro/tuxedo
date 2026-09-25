//! Statement lists -- every body in a file, from the top level down --
//! and `alias`.
//!
//! A body is one statement per line, or joined with `; ` where the
//! construct that owns it is held to one line (a `( )` body, a `#{}`
//! hole). Blank lines between statements are kept, at most one in a row,
//! and one is added where RuboCop's layout cops require it: around method,
//! class and module definitions, access modifiers and `attr_*` calls, and
//! after a guard clause or a run of them (`wants_forced_blank_line`).

use super::Formatter;
use super::doc::Doc;

/// A method/class/module definition -- rubocop's Layout/EmptyLineBetweenDefs
/// wants a blank line before/after each of these by default
/// (EmptyLineBetweenMethodDefs/ClassDefs/ModuleDefs all default true).
fn is_def_like(node: &ruby_prism::Node<'_>) -> bool {
    matches!(
        node,
        ruby_prism::Node::DefNode { .. }
            | ruby_prism::Node::ClassNode { .. }
            | ruby_prism::Node::ModuleNode { .. }
    )
}

/// A bare `private`/`protected`/`public`/`module_function` call (no
/// receiver, no arguments -- the visibility-scope form, not
/// `private def foo; end` or `private :foo`, which don't open/close a
/// scope the same way). Layout/EmptyLinesAroundAccessModifier's stock
/// default (`EnforcedStyle: around`) wants a blank line both before and
/// after.
fn is_access_modifier_call(node: &ruby_prism::Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| {
        call.receiver().is_none()
            && call.arguments().is_none()
            && call.block().is_none()
            && matches!(
                std::str::from_utf8(call.name().as_slice()),
                Ok("private" | "protected" | "public" | "module_function")
            )
    })
}

/// A bare `attr_accessor`/`attr_reader`/`attr_writer`/`attr` call.
/// Layout/EmptyLinesAroundAttributeAccessor wants a blank line around a
/// *run* of these (not between two consecutive ones).
fn is_attr_accessor_call(node: &ruby_prism::Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| {
        call.receiver().is_none()
            && matches!(
                std::str::from_utf8(call.name().as_slice()),
                Ok("attr_accessor" | "attr_reader" | "attr_writer" | "attr")
            )
    })
}

/// A magic comment (`# frozen_string_literal: true`, `# encoding: utf-8`,
/// ...) -- Layout/EmptyLineAfterMagicComment wants a blank line separating
/// the last of a run of these from whatever follows, since they're
/// interpreter directives rather than documentation for the code below.
///
/// A Vim modeline (`# vim: set fileencoding=big5:`) is a fourth spelling
/// MRI itself recognizes for the encoding directive, alongside the plain
/// and Emacs (`-*- ... -*-`) forms below -- editors read the rest of the
/// `key=value` list too, but only the encoding keys change how the parser
/// itself reads the file.
fn is_magic_comment(text: &str) -> bool {
    let inner = text.trim_start_matches('#').trim();

    if let Some(rest) = inner
        .strip_prefix("vim:")
        .or_else(|| inner.strip_prefix("vi:"))
    {
        let rest = rest.strip_prefix("set ").unwrap_or(rest);
        return rest
            .split([':', ' ', ','])
            .filter_map(|pair| pair.split_once('='))
            .any(|(key, _)| matches!(key.trim(), "fileencoding" | "encoding"));
    }

    let inner = inner.strip_prefix("-*-").map(str::trim).unwrap_or(inner);
    let inner = inner.strip_suffix("-*-").map(str::trim).unwrap_or(inner);
    let Some((key, _)) = inner.split_once(':') else {
        return false;
    };
    matches!(
        key.trim(),
        "encoding"
            | "coding"
            | "frozen_string_literal"
            | "warn_indent"
            | "shareable_constant_value"
    )
}

/// A modifier or plain block-form `if`/`unless` (no `elsif`/`else`) whose
/// entire body is a single "jump" statement (`return`/`next`/`break`/
/// `redo`/`retry`, or a bare `raise`/`fail`) -- the shape
/// Layout/EmptyLineAfterGuardClause wants a blank line after. An
/// `elsif`/`else` chain disqualifies it: that's a real branch, not a
/// short-circuit guard.
fn is_guard_clause(node: &ruby_prism::Node<'_>) -> bool {
    let statements = if let Some(if_node) = node.as_if_node() {
        if if_node.subsequent().is_some() {
            return false;
        }
        if_node.statements()
    } else if let Some(unless_node) = node.as_unless_node() {
        if unless_node.else_clause().is_some() {
            return false;
        }
        unless_node.statements()
    } else {
        return false;
    };

    let Some(statements) = statements else {
        return false;
    };
    let body: Vec<_> = statements.body().iter().collect();
    let [only] = body.as_slice() else {
        return false;
    };

    matches!(
        only,
        ruby_prism::Node::ReturnNode { .. }
            | ruby_prism::Node::NextNode { .. }
            | ruby_prism::Node::BreakNode { .. }
            | ruby_prism::Node::RedoNode { .. }
            | ruby_prism::Node::RetryNode { .. }
    ) || only.as_call_node().is_some_and(|call| {
        call.receiver().is_none()
            && matches!(
                std::str::from_utf8(call.name().as_slice()),
                Ok("raise" | "fail")
            )
    })
}

impl Formatter {
    /// Whether a blank line must separate `prev` (the statement just
    /// rendered) from `current` (the one about to be), even if the source
    /// had none -- the insertion counterpart to the gap-preservation the
    /// caller already does from line numbers alone. Mirrors four rubocop
    /// Layout cops that *add* blank lines rather than just flag missing
    /// ones: EmptyLineBetweenDefs, EmptyLineAfterGuardClause,
    /// EmptyLinesAroundAccessModifier, EmptyLinesAroundAttributeAccessor.
    fn wants_forced_blank_line(
        &self,
        prev: &ruby_prism::Node<'_>,
        current: &ruby_prism::Node<'_>,
    ) -> bool {
        // A run of guards reads as one block: the cop wants the blank line
        // after the last of them, not between them.
        if is_guard_clause(prev) && !is_guard_clause(current) {
            return true;
        }

        if is_access_modifier_call(prev) || is_access_modifier_call(current) {
            return true;
        }

        if is_attr_accessor_call(prev) != is_attr_accessor_call(current) {
            return true;
        }

        // rubocop's AllowAdjacentOneLineDefs exception doesn't apply here:
        // Tuxedo always expands a def's body onto multiple lines (even an
        // endless method's), so there's no such thing as an adjacent
        // one-line def pair in its own output to exempt.
        if is_def_like(prev) || is_def_like(current) {
            return true;
        }

        false
    }

    /// A body, in whichever of its two shapes is in force: `; `-joined on
    /// one line while `single_line` is set (a `( )` body, a `#{}` hole),
    /// one statement per line otherwise.
    ///
    /// Neither shape carries a leading indent. A body is indented by the
    /// [`Doc::indent`] the construct that owns it wraps it in, which is
    /// what lets the same document be a method body at one depth and a
    /// module body at another.
    pub(super) fn fmt_statements_node(&mut self, node: &ruby_prism::StatementsNode<'_>) -> Doc {
        let stmts: Vec<_> = node.body().iter().collect();
        if self.single_line {
            return self.flat_statements_doc(&stmts);
        }
        self.block_statements_doc(node)
    }

    /// Statements on one line, `; ` between them.
    pub(super) fn flat_statements_doc(&mut self, stmts: &[ruby_prism::Node<'_>]) -> Doc {
        let leaves: Vec<Doc> = stmts.iter().map(|stmt| self.node_doc(stmt)).collect();
        Doc::join(leaves, Doc::text("; "))
    }

    /// The block form: one leaf per own-line comment and per statement,
    /// separated by a [`Doc::hardline`], with a second hardline wherever a
    /// blank line belongs.
    ///
    /// Which blank lines belong is half source and half rule. A gap the
    /// author left between two statements is kept; a gap RuboCop's
    /// Layout/EmptyLine* cops would insert is added whether or not the
    /// author left one (`wants_forced_blank_line`).
    fn block_statements_doc(&mut self, node: &ruby_prism::StatementsNode<'_>) -> Doc {
        // `(blank line before it, the leaf)`.
        let mut items: Vec<(bool, Doc, bool)> = Vec::new();
        // The body of a class, module or `class <<` puts a bare access
        // modifier, and the comments written above it, one level out.
        let outdent = self.class_body == Some(node.location().start_offset());

        let mut prev_end_line: Option<usize> = None;
        let mut prev_stmt: Option<ruby_prism::Node<'_>> = None;
        // Where the heredocs in here end, handed to the statement this list
        // belongs to: when it collapses onto one line (`-> { eval <<~X }`)
        // its terminator still ends the statement.
        let mut heredocs_end = self.heredoc_end_line.take();
        // Set once a magic comment has been emitted and cleared as soon as
        // whatever follows isn't another one -- a run of magic comments
        // (`# encoding: ...` immediately followed by
        // `# frozen_string_literal: ...`) needs no blank line between its
        // own members, only after the run ends.
        let mut pending_blank_after_magic = false;

        // Where the last `# tuxedo:off` region ended: a statement that
        // starts before it was copied through with the rest.
        let mut region_end = 0;

        // The start line of each statement, one ahead of the main loop
        // below: cheap enough to precompute, and it lets a statement check
        // whether the next one shares its line without needing to hold a
        // whole extra `Node`.
        let start_lines: Vec<usize> = node
            .body()
            .iter()
            .map(|s| self.line_of(s.location().start_offset()))
            .collect();

        for (index, stmt) in node.body().iter().enumerate() {
            if stmt.location().start_offset() < region_end {
                prev_stmt = Some(stmt);
                continue;
            }
            let first_item = items.len();
            let start_line = self.line_of(stmt.location().start_offset());
            // Computed once per statement and consulted at whichever point
            // actually ends up first -- the leading comment block if there
            // is one (a blank line belongs before an attached doc comment,
            // not wedged between the comment and the statement it
            // documents), otherwise the statement itself.
            let force_blank = prev_stmt
                .as_ref()
                .is_some_and(|prev| self.wants_forced_blank_line(prev, &stmt));
            let mut force_blank_pending = force_blank;

            while self.comment_cursor < self.comments.len() {
                let comment_line = self.line_of(self.comments[self.comment_cursor].start);
                if comment_line >= start_line {
                    break;
                }
                if self.at_off_marker() {
                    let blank = prev_end_line.is_some_and(|prev| comment_line > prev + 1)
                        || force_blank_pending
                        || pending_blank_after_magic;
                    force_blank_pending = false;
                    pending_blank_after_magic = false;
                    let (text, end) = self.verbatim_region(node);
                    items.push((blank, Doc::text(text), false));
                    prev_end_line = Some(self.line_of(end));
                    region_end = end;
                    continue;
                }
                let is_magic = is_magic_comment(&self.comments[self.comment_cursor].text);
                let blank = prev_end_line.is_some_and(|prev| comment_line > prev + 1)
                    || force_blank_pending
                    || (pending_blank_after_magic && !is_magic);
                force_blank_pending = false;
                pending_blank_after_magic = is_magic;

                let text = Self::own_line_comment(&self.comments[self.comment_cursor].text);
                items.push((blank, Doc::text(text), false));
                // An embedded document spans lines; the code after it is
                // adjacent to its `=end`, not to its `=begin`.
                prev_end_line = Some(
                    comment_line
                        + self.comments[self.comment_cursor]
                            .text
                            .trim_end()
                            .matches('\n')
                            .count(),
                );
                self.comment_cursor += 1;
            }

            if stmt.location().start_offset() < region_end {
                prev_stmt = Some(stmt);
                continue;
            }

            let blank = prev_end_line.is_some_and(|prev| start_line > prev + 1)
                || force_blank_pending
                || pending_blank_after_magic;
            pending_blank_after_magic = false;

            self.heredoc_end_line = None;
            let mut leaf = self.node_doc(&stmt);

            // A heredoc's node ends at its opening tag, so its own trailing
            // comment -- on that same line, whether or not more of the
            // statement follows the tag -- is found before the terminator
            // line below is folded in to extend the statement for blank
            // -line and next-statement purposes.
            let mut end_line = self.line_of(stmt.location().end_offset().saturating_sub(1));
            // A `;`-joined line (`a; b # why`) puts more than one statement
            // on `end_line`; only the last one is actually next to the
            // comment; an earlier one that also checked by line number
            // alone would claim it first and leave it on the wrong
            // statement once each gets its own line.
            let last_on_line = start_lines
                .get(index + 1)
                .is_none_or(|&next| next != end_line);
            if last_on_line && let Some(comment) = self.trailing_comment_doc(end_line) {
                leaf = leaf.append(comment);
            }
            if let Some(heredoc_line) = self.heredoc_end_line.take() {
                end_line = end_line.max(heredoc_line);
                heredocs_end = heredocs_end.max(Some(heredoc_line));
            }

            items.push((blank, leaf, false));
            if outdent && is_access_modifier_call(&stmt) {
                for item in &mut items[first_item..] {
                    item.2 = true;
                }
            }
            prev_end_line = Some(end_line);
            prev_stmt = Some(stmt);
        }

        self.heredoc_end_line = heredocs_end;

        let mut doc = Doc::nil();
        let nest = self.indent_str.len();
        // A list opening with an outdented item draws the break before it
        // itself, so the break can sit under the dedent too.
        let owns_first_break = outdent && items.first().is_some_and(|item| item.2);
        self.body_owns_first_break = owns_first_break;
        for (i, (blank, leaf, out)) in items.into_iter().enumerate() {
            let separator = if i == 0 {
                if owns_first_break {
                    Doc::hardline()
                } else {
                    Doc::nil()
                }
            } else if blank {
                Doc::blank_line()
            } else {
                Doc::hardline()
            };
            // The printer takes a line's indent from what follows the
            // break, so the leaf goes under the dedent with its separator.
            // A modifier or a comment is one line: nothing in it breaks.
            let piece = separator.append(leaf);
            doc = doc.append(if out { piece.dedent(nest) } else { piece });
        }
        doc
    }

    /// `alias new_name old_name` (bare or symbol-style method names --
    /// `SymbolNode` renders either form faithfully based on the source, so
    /// this just needs the literal `alias` keyword between them). Prism's
    /// default visitor for this node visits `new_name`/`old_name` back to
    /// back with no keyword or separator at all, which is why it needs its
    /// own handler rather than falling through to that default.
    pub(super) fn fmt_alias_method_node(&mut self, node: &ruby_prism::AliasMethodNode<'_>) -> Doc {
        self.alias_doc(&node.new_name(), &node.old_name())
    }

    /// `alias $new $old` -- the global-variable form of `alias`, a distinct
    /// node type from `fmt_alias_method_node` since globals can't be
    /// written as symbols. Same missing-keyword problem as that one.
    pub(super) fn fmt_alias_global_variable_node(
        &mut self,
        node: &ruby_prism::AliasGlobalVariableNode<'_>,
    ) -> Doc {
        self.alias_doc(&node.new_name(), &node.old_name())
    }

    /// `alias new old` for both forms above.
    fn alias_doc(
        &mut self,
        new_name: &ruby_prism::Node<'_>,
        old_name: &ruby_prism::Node<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, "alias ");
        self.run_node(&mut run, new_name);
        self.run_text(&mut run, " ");
        self.run_node(&mut run, old_name);
        self.end_run(run)
    }
}
