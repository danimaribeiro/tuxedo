use super::Formatter;
use ruby_prism::Visit;

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

    pub(super) fn fmt_statements_node(&mut self, node: &ruby_prism::StatementsNode<'_>) {
        let stmts = node.body();

        if self.single_line {
            for (i, stmt) in stmts.iter().enumerate() {
                if i > 0 {
                    self.output.push_str("; ");
                }
                self.inline = true;
                self.visit(&stmt);
            }
            return;
        }

        let mut prev_end_line: Option<usize> = None;
        let mut prev_stmt: Option<ruby_prism::Node<'_>> = None;

        for stmt in stmts.iter() {
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
                if prev_end_line.is_some_and(|prev| comment_line > prev + 1) || force_blank_pending
                {
                    self.ensure_blank_line();
                }
                force_blank_pending = false;
                self.write_indent();
                let formatted =
                    Self::format_comment(&self.comments[self.comment_cursor].text).into_owned();
                self.output.push_str(&formatted);
                self.output.push('\n');
                prev_end_line = Some(comment_line);
                self.comment_cursor += 1;
            }

            if prev_end_line.is_some_and(|prev| start_line > prev + 1) || force_blank_pending {
                self.ensure_blank_line();
            }

            self.inline = false;
            self.heredoc_end_line = None;
            self.visit(&stmt);

            // A heredoc's node ends at its opening tag, so a statement
            // carrying one really ends at its terminator line.
            let mut end_line = self.line_of(stmt.location().end_offset().saturating_sub(1));
            if let Some(heredoc_line) = self.heredoc_end_line.take() {
                end_line = end_line.max(heredoc_line);
            }
            self.emit_trailing_comment_if_same_line(end_line);
            prev_end_line = Some(end_line);
            prev_stmt = Some(stmt);
        }
    }

    /// `alias new_name old_name` (bare or symbol-style method names --
    /// `SymbolNode` renders either form faithfully based on the source, so
    /// this just needs the literal `alias` keyword between them). Prism's
    /// default visitor for this node visits `new_name`/`old_name` back to
    /// back with no keyword or separator at all, which is why it needs its
    /// own handler rather than falling through to that default.
    pub(super) fn fmt_alias_method_node(&mut self, node: &ruby_prism::AliasMethodNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("alias ");
        self.inline = true;
        self.visit(&node.new_name());
        self.output.push(' ');
        self.inline = true;
        self.visit(&node.old_name());
        if !inline {
            self.output.push('\n');
        }
    }

    /// `alias $new $old` -- the global-variable form of `alias`, a distinct
    /// node type from `fmt_alias_method_node` since globals can't be
    /// written as symbols. Same missing-keyword problem as that one.
    pub(super) fn fmt_alias_global_variable_node(
        &mut self,
        node: &ruby_prism::AliasGlobalVariableNode<'_>,
    ) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("alias ");
        self.inline = true;
        self.visit(&node.new_name());
        self.output.push(' ');
        self.inline = true;
        self.visit(&node.old_name());
        if !inline {
            self.output.push('\n');
        }
    }
}
