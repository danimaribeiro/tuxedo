use super::Formatter;
use ruby_prism::Visit;

impl Formatter {
    pub(super) fn fmt_if_node(&mut self, node: &ruby_prism::IfNode<'_>) {
        let keyword = node.if_keyword_loc().map(|k| k.as_slice());
        let is_ternary = keyword.is_none();
        let is_elsif = keyword == Some(b"elsif");
        let is_modifier = !is_ternary && !is_elsif && node.end_keyword_loc().is_none();

        if is_ternary {
            // Ternary: predicate ? true_branch : false_branch
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            self.inline = true;
            self.visit(&node.predicate());
            self.output.push_str(" ? ");
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" : ");
            if let Some(consequent) = node.subsequent()
                && let Some(else_node) = consequent.as_else_node()
                && let Some(body) = else_node.statements()
            {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        if is_modifier {
            // Modifier: body if condition
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" if ");
            self.inline = true;
            self.visit(&node.predicate());
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        // Block if / elsif
        let inline = self.inline;
        if !is_elsif && !inline {
            self.write_indent();
        }
        self.output
            .push_str(if is_elsif { "elsif " } else { "if " });
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        self.output.push('\n');
        let header_line = self.line_of(node.predicate().location().start_offset());
        self.emit_trailing_comment_if_same_line(header_line);

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;

        if let Some(consequent) = node.subsequent() {
            self.write_indent();
            // An elsif/else terminates the same chain this `if` opened, so
            // it needs the chain's *original* inline-ness to decide whether
            // its own trailing "end" gets a newline -- not `self.inline`,
            // which was just forced to false above for the predicate/body.
            self.inline = inline;
            self.visit(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end");
            if !inline {
                self.output.push('\n');
            }
        }
    }

    pub(super) fn fmt_else_node(&mut self, node: &ruby_prism::ElseNode<'_>) {
        // Whether the if/case chain this closes is itself being rendered
        // inline (as a sub-expression, e.g. a call argument) -- see
        // fmt_if_node, which restores this into self.inline before
        // recursing here for exactly this reason. A captured inline value
        // must not carry a hardcoded trailing newline: joining it with more
        // content on the same Doc/text line (a following `, arg)`) would
        // otherwise leave that content dangling on its own line, which
        // isn't just ugly -- for a call argument it's invalid Ruby.
        let inline = self.inline;
        self.output.push_str("else\n");

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    /// Renders `begin ... end`, including any attached `rescue`/`else`/
    /// `ensure` clauses. Also reached with no `begin`/`end` keywords at all:
    /// prism wraps a method/block body in a `BeginNode` whenever it has a
    /// `rescue`/`else`/`ensure` clause (e.g. `def foo; rescue; end`), in
    /// which case the enclosing `def`/block already writes its own
    /// keyword/indent/`end` and this only needs to render the clauses.
    pub(super) fn fmt_begin_node(&mut self, node: &ruby_prism::BeginNode<'_>) {
        let has_keyword = node.begin_keyword_loc().is_some();
        let inline = self.inline;

        if has_keyword {
            if !inline {
                self.write_indent();
            }
            self.output.push_str("begin\n");
            self.indent += 1;
        }

        if let Some(statements) = node.statements() {
            self.fmt_statements_node(&statements);
        }

        if has_keyword {
            self.indent -= 1;
        }

        let has_clauses = node.rescue_clause().is_some()
            || node.else_clause().is_some()
            || node.ensure_clause().is_some();
        if has_clauses {
            // rescue/else/ensure always align with the keyword that opens
            // the body they guard. For an explicit `begin` we've already
            // dedented back to its level above; for an implicit one (no
            // `begin`/`end` here at all) `self.indent` is still one level
            // deeper — whatever the enclosing def/block set it to for the
            // body — so dedent by one to reach that keyword's own level.
            let dedent = usize::from(!has_keyword);
            self.indent -= dedent;

            if let Some(rescue) = node.rescue_clause() {
                self.fmt_rescue_node(&rescue);
            }
            if let Some(else_clause) = node.else_clause() {
                self.write_indent();
                self.output.push_str("else\n");
                self.indent += 1;
                if let Some(statements) = else_clause.statements() {
                    self.fmt_statements_node(&statements);
                }
                self.indent -= 1;
            }
            if let Some(ensure) = node.ensure_clause() {
                self.fmt_ensure_node(&ensure);
            }

            self.indent += dedent;
        }

        if has_keyword {
            self.write_indent();
            self.output.push_str("end");
            if !inline {
                self.output.push('\n');
            }
        }
    }

    /// `expr rescue fallback` -- the modifier form, as opposed to an
    /// explicit `begin`/`rescue` block (`fmt_rescue_node`). Prism's default
    /// visitor for this node just visits `expression()` then
    /// `rescue_expression()` back to back with no separator, which is why
    /// this needs its own handler rather than falling through: without the
    /// literal " rescue " keyword between them, a statement like
    /// `transmit(...) rescue nil` followed by more code lost the boundary
    /// between "nil" and the next line entirely.
    pub(super) fn fmt_rescue_modifier_node(&mut self, node: &ruby_prism::RescueModifierNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.inline = true;
        self.visit(&node.expression());
        self.output.push_str(" rescue ");
        self.inline = true;
        self.visit(&node.rescue_expression());
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_rescue_node(&mut self, node: &ruby_prism::RescueNode<'_>) {
        self.write_indent();
        self.output.push_str("rescue");

        let exceptions: Vec<_> = node.exceptions().iter().collect();
        for (i, exception) in exceptions.iter().enumerate() {
            self.output.push_str(if i == 0 { " " } else { ", " });
            self.inline = true;
            self.visit(exception);
        }

        if let Some(reference) = node.reference() {
            self.output.push_str(" => ");
            self.inline = true;
            self.visit(&reference);
        }
        self.output.push('\n');

        self.indent += 1;
        if let Some(statements) = node.statements() {
            self.fmt_statements_node(&statements);
        }
        self.indent -= 1;

        if let Some(subsequent) = node.subsequent() {
            self.fmt_rescue_node(&subsequent);
        }
    }

    pub(super) fn fmt_ensure_node(&mut self, node: &ruby_prism::EnsureNode<'_>) {
        self.write_indent();
        self.output.push_str("ensure\n");

        self.indent += 1;
        if let Some(statements) = node.statements() {
            self.fmt_statements_node(&statements);
        }
        self.indent -= 1;
    }

    pub(super) fn fmt_unless_node(&mut self, node: &ruby_prism::UnlessNode<'_>) {
        let is_modifier = node.end_keyword_loc().is_none();

        if is_modifier {
            // Modifier: body unless condition
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" unless ");
            self.inline = true;
            self.visit(&node.predicate());
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        // Block unless
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("unless ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;

        if let Some(consequent) = node.else_clause() {
            self.write_indent();
            self.inline = inline;
            self.fmt_else_node(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end");
            if !inline {
                self.output.push('\n');
            }
        }
    }

    pub(super) fn fmt_while_node(&mut self, node: &ruby_prism::WhileNode<'_>) {
        let is_modifier = node.closing_loc().is_none();

        if is_modifier {
            // Modifier: body while condition
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" while ");
            self.inline = true;
            self.visit(&node.predicate());
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        // Block while
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("while ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_until_node(&mut self, node: &ruby_prism::UntilNode<'_>) {
        let is_modifier = node.closing_loc().is_none();

        if is_modifier {
            // Modifier: body until condition
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" until ");
            self.inline = true;
            self.visit(&node.predicate());
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        // Block until
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("until ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_for_node(&mut self, node: &ruby_prism::ForNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("for ");
        self.inline = true;
        self.visit(&node.index());
        self.output.push_str(" in ");
        self.inline = true;
        self.visit(&node.collection());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    pub(super) fn fmt_case_node(&mut self, node: &ruby_prism::CaseNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("case");
        if let Some(predicate) = node.predicate() {
            self.output.push(' ');
            self.inline = true;
            self.visit(&predicate);
            self.inline = false;
        }
        self.output.push('\n');

        for when in node.conditions().iter() {
            self.visit(&when);
        }

        if let Some(consequent) = node.else_clause() {
            self.write_indent();
            self.inline = inline;
            self.fmt_else_node(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end");
            if !inline {
                self.output.push('\n');
            }
        }
    }

    pub(super) fn fmt_when_node(&mut self, node: &ruby_prism::WhenNode<'_>) {
        self.write_indent();
        self.output.push_str("when ");
        for (i, condition) in node.conditions().iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&condition);
        }
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;
    }

    pub(super) fn fmt_and_node(&mut self, node: &ruby_prism::AndNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.inline = true;
        self.visit(&node.left());
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("&&");
        self.output.push(' ');
        self.output.push_str(op_str);
        self.output.push(' ');
        self.inline = true;
        self.visit(&node.right());
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_or_node(&mut self, node: &ruby_prism::OrNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.inline = true;
        self.visit(&node.left());
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("||");
        self.output.push(' ');
        self.output.push_str(op_str);
        self.output.push(' ');
        self.inline = true;
        self.visit(&node.right());
        if !inline {
            self.output.push('\n');
        }
    }
}
