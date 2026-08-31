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

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;

        if let Some(consequent) = node.subsequent() {
            self.write_indent();
            self.visit(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end\n");
        }
    }

    pub(super) fn fmt_else_node(&mut self, node: &ruby_prism::ElseNode<'_>) {
        self.output.push_str("else\n");

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
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
            self.fmt_else_node(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end\n");
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
        self.output.push_str("end\n");
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
        self.output.push_str("end\n");
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
            self.fmt_else_node(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end\n");
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
