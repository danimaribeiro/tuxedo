use super::Formatter;
use ruby_prism::Visit;

impl Formatter {
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

        for stmt in stmts.iter() {
            let start_line = self.line_of(stmt.location().start_offset());

            while self.comment_cursor < self.comments.len() {
                let comment_line = self.line_of(self.comments[self.comment_cursor].start);
                if comment_line >= start_line {
                    break;
                }
                if prev_end_line.is_some_and(|prev| comment_line > prev + 1) {
                    self.ensure_blank_line();
                }
                self.write_indent();
                let formatted =
                    Self::format_comment(&self.comments[self.comment_cursor].text).into_owned();
                self.output.push_str(&formatted);
                self.output.push('\n');
                prev_end_line = Some(comment_line);
                self.comment_cursor += 1;
            }

            if prev_end_line.is_some_and(|prev| start_line > prev + 1) {
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
