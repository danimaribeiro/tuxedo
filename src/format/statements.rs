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
fn is_magic_comment(text: &str) -> bool {
    let inner = text.trim_start_matches('#').trim();
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
        if is_guard_clause(prev) {
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
    /// one line while `single_line` is set (a `( )` body, a `{ }` block, a
    /// ternary branch), one statement per line otherwise.
    ///
    /// The two shapes differ in more than their separator. The flat one
    /// composes -- its statements keep their own line points, so whatever
    /// group ends up holding it decides where they break. The block one
    /// does not: its leaves are rendered at absolute columns and come back
    /// as one opaque [`Doc::text`], terminating newline included, on which
    /// an enclosing [`Doc::indent`] is a no-op. That is sound for a body
    /// specifically, whose indent is fixed by nesting depth rather than by
    /// any group's decision to break -- see `statements_doc`.
    pub(super) fn fmt_statements_node(&mut self, node: &ruby_prism::StatementsNode<'_>) -> Doc {
        if self.single_line {
            let stmts: Vec<_> = node.body().iter().collect();
            return self.flat_statements_doc(&stmts);
        }
        Doc::text(self.block_statements_text(node))
    }

    /// [`Formatter::fmt_statements_node`] straight into `output`, for the
    /// constructs that still write their body imperatively. The body
    /// carries its own leading indent, so the render pads out to the
    /// column and nothing else -- the text arrives complete.
    pub(super) fn write_statements_node(&mut self, node: &ruby_prism::StatementsNode<'_>) {
        let doc = self.fmt_statements_node(node);
        let rendered = self.render_doc_to_string(doc);
        self.output.push_str(&rendered);
    }

    /// Statements on one line, `; ` between them: a parenthesized body, a
    /// `{ }` block body, a ternary branch.
    ///
    /// Composed rather than rendered. Each statement keeps the line points
    /// it built, so the group that ends up holding them measures them
    /// against the column they will really sit at; rendering here would
    /// fix that column now, and for a body an enclosing group has not
    /// placed yet there is no such column to fix it to.
    pub(super) fn flat_statements_doc(&mut self, stmts: &[ruby_prism::Node<'_>]) -> Doc {
        let leaves: Vec<Doc> = stmts.iter().map(|stmt| self.node_doc(stmt)).collect();
        Doc::join(leaves, Doc::text("; "))
    }

    /// The block form: one statement per line, rendered where it will sit
    /// and handed back as finished text with its terminating newline on
    /// it.
    fn block_statements_text(&mut self, node: &ruby_prism::StatementsNode<'_>) -> String {
        let doc = self.statements_doc(node);
        // The leaves already carry their own indent (see `statement_doc`),
        // so this renders with no nesting of its own -- every `hardline` in
        // it is a bare newline. Width plays no part either: each leaf is
        // finished text by the time it gets here.
        let rendered = doc.render(self.line_length);
        // A body with nothing in it writes nothing at all -- not even the
        // terminating newline. A file that is only comments parses to an
        // empty `StatementsNode`, and its comments are flushed by the
        // caller afterwards; a newline here would open the file with a
        // blank line.
        if rendered.is_empty() {
            return rendered;
        }
        rendered + "\n"
    }

    /// The body as a `Doc`: one leaf per own-line comment and per
    /// statement, separated by a [`Doc::hardline`], with a second hardline
    /// wherever a blank line belongs.
    ///
    /// Each leaf is rendered where it will actually sit and lifted back out
    /// (`statement_doc`), so it keeps the absolute indent the string
    /// emitter gives it and every width decision inside it is made against
    /// the real column. That is safe for a *body* specifically: a body's
    /// indent is fixed by nesting depth, not by any group's decision to
    /// break, so the indent a leaf is built at is always the one it is
    /// rendered at. What the `Doc` adds is that the separators are real
    /// hardlines, so a construct that wraps this body can report itself as
    /// multi-line instead of the caller testing for it.
    fn statements_doc(&mut self, node: &ruby_prism::StatementsNode<'_>) -> Doc {
        // `(blank line before it, the leaf)`.
        let mut items: Vec<(bool, Doc)> = Vec::new();

        let mut prev_end_line: Option<usize> = None;
        let mut prev_stmt: Option<ruby_prism::Node<'_>> = None;
        // Set once a magic comment has been emitted and cleared as soon as
        // whatever follows isn't another one -- a run of magic comments
        // (`# encoding: ...` immediately followed by
        // `# frozen_string_literal: ...`) needs no blank line between its
        // own members, only after the run ends.
        let mut pending_blank_after_magic = false;

        for stmt in node.body().iter() {
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
                let is_magic = is_magic_comment(&self.comments[self.comment_cursor].text);
                let blank = prev_end_line.is_some_and(|prev| comment_line > prev + 1)
                    || force_blank_pending
                    || (pending_blank_after_magic && !is_magic);
                force_blank_pending = false;
                pending_blank_after_magic = is_magic;

                let text =
                    Self::format_comment(&self.comments[self.comment_cursor].text).into_owned();
                let indent = self.indent_str.repeat(self.indent);
                items.push((blank, Doc::text(format!("{indent}{text}"))));
                prev_end_line = Some(comment_line);
                self.comment_cursor += 1;
            }

            let blank = prev_end_line.is_some_and(|prev| start_line > prev + 1)
                || force_blank_pending
                || pending_blank_after_magic;
            pending_blank_after_magic = false;

            self.heredoc_end_line = None;
            let mut leaf = self.statement_doc(&stmt);

            // A heredoc's node ends at its opening tag, so a statement
            // carrying one really ends at its terminator line.
            let mut end_line = self.line_of(stmt.location().end_offset().saturating_sub(1));
            if let Some(heredoc_line) = self.heredoc_end_line.take() {
                end_line = end_line.max(heredoc_line);
            }
            if let Some(comment) = self.trailing_comment_doc(end_line) {
                leaf = leaf.append(comment);
            }

            items.push((blank, leaf));
            prev_end_line = Some(end_line);
            prev_stmt = Some(stmt);
        }

        let mut doc = Doc::nil();
        for (i, (blank, leaf)) in items.into_iter().enumerate() {
            if i > 0 {
                doc = doc.append(Doc::hardline());
                if blank {
                    doc = doc.append(Doc::hardline());
                }
            }
            doc = doc.append(leaf);
        }
        doc
    }

    /// One statement, rendered exactly the way the string emitter renders
    /// it -- in place, so `current_column` is real -- and lifted back out
    /// as a leaf with its terminating newline removed, since
    /// `statements_doc` supplies the separators.
    fn statement_doc(&mut self, stmt: &ruby_prism::Node<'_>) -> Doc {
        let start = self.output.len();
        self.inline = false;
        self.visit(stmt);
        let mut text = self.output.split_off(start);
        if text.ends_with('\n') {
            text.pop();
        }
        Doc::text(text)
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
