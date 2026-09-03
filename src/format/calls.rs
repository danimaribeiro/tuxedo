use super::Formatter;
use pretty::RcDoc as Doc;
use ruby_prism::Visit;

impl Formatter {
    pub(super) fn fmt_call_node(&mut self, node: &ruby_prism::CallNode<'_>) {
        let name = node.name();
        let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");
        let inline = self.inline;
        self.inline = false;

        if let Some(receiver) = node.receiver() {
            if !inline {
                self.write_indent();
            }

            let is_index = name_str == "[]" || name_str == "[]=";
            let is_identifier_call = name_str
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');

            if is_index {
                self.inline = true;
                self.visit(&receiver);
                self.output.push('[');
                let mut args: Vec<_> = node
                    .arguments()
                    .map(|a| a.arguments().iter().collect())
                    .unwrap_or_default();
                let value = if name_str == "[]=" { args.pop() } else { None };
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.inline = true;
                    self.visit(arg);
                }
                self.output.push(']');
                if let Some(value) = value {
                    self.output.push_str(" = ");
                    self.inline = true;
                    self.visit(&value);
                }
            } else if name_str == "!" && node.arguments().is_none() {
                self.output.push('!');
                self.inline = true;
                self.visit(&receiver);
            } else if is_identifier_call {
                self.inline = true;
                self.visit(&receiver);
                let op = node
                    .call_operator_loc()
                    .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
                    .unwrap_or(".");
                self.output.push_str(op);

                if node.is_attribute_write() {
                    self.output
                        .push_str(name_str.strip_suffix('=').unwrap_or(name_str));
                    self.output.push_str(" = ");
                    if let Some(value) = node.arguments().and_then(|a| a.arguments().iter().next())
                    {
                        self.inline = true;
                        self.visit(&value);
                    }
                } else {
                    self.output.push_str(name_str);
                    let parenthesized = node.opening_loc().is_some();
                    self.write_call_args(node, parenthesized);
                }
            } else {
                // Operator-style call (`a + b`, `a <=> b`, ...). With
                // exactly one argument -- the overwhelmingly common case --
                // wrap it as a Doc so an overlong line breaks after the
                // operator with the argument indented one level, instead of
                // always rendering flat regardless of width.
                let single_arg = node
                    .arguments()
                    .map(|a| a.arguments().iter().collect::<Vec<_>>())
                    .filter(|args| args.len() == 1)
                    .map(|mut args| args.remove(0));

                if let Some(arg) = single_arg {
                    let receiver_doc = self.capture_as_doc(|f| {
                        f.inline = true;
                        f.visit(&receiver);
                    });
                    let arg_doc = self.capture_as_doc(|f| {
                        f.inline = true;
                        f.visit(&arg);
                    });
                    let nest_width = self.indent_width() + self.indent_str.len();
                    let doc = receiver_doc
                        .append(Doc::text(" "))
                        .append(Doc::text(name_str.to_string()))
                        .append(
                            Doc::line()
                                .append(arg_doc)
                                .nest(nest_width as isize)
                                .group(),
                        );
                    let rendered = self.render_doc_to_string(doc);
                    self.output.push_str(&rendered);
                    self.write_call_block(node);
                    if !inline {
                        self.output.push('\n');
                    }
                    return;
                }

                self.inline = true;
                self.visit(&receiver);
                self.output.push(' ');
                self.output.push_str(name_str);
                if let Some(args) = node.arguments() {
                    self.output.push(' ');
                    let arguments = args.arguments();
                    for (i, arg) in arguments.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.inline = true;
                        self.visit(&arg);
                    }
                }
            }
            self.write_call_block(node);
            if !inline {
                self.output.push('\n');
            }
        } else {
            if !inline {
                self.write_indent();
            }
            self.output.push_str(name_str);
            let parenthesized = node.opening_loc().is_some();
            self.write_call_args(node, parenthesized);
            self.write_call_block(node);
            if !inline {
                self.output.push('\n');
            }
        }
    }

    pub(super) fn fmt_super_node(&mut self, node: &ruby_prism::SuperNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("super");

        let parenthesized = node.lparen_loc().is_some();
        let positional: Vec<_> = node
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let block_pass = match node.block() {
            Some(b @ ruby_prism::Node::BlockArgumentNode { .. }) => Some(b),
            _ => None,
        };

        if parenthesized || !positional.is_empty() || block_pass.is_some() {
            self.output.push(if parenthesized { '(' } else { ' ' });
            for (i, arg) in positional.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(arg);
            }
            if let Some(block_pass) = &block_pass {
                if !positional.is_empty() {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(block_pass);
            }
            if parenthesized {
                self.output.push(')');
            }
        }

        if let Some(block @ ruby_prism::Node::BlockNode { .. }) = node.block() {
            self.visit(&block);
        }

        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_forwarding_super_node(&mut self, node: &ruby_prism::ForwardingSuperNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("super");
        if let Some(block) = node.block() {
            self.fmt_block_node(&block);
        }
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_yield_node(&mut self, node: &ruby_prism::YieldNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("yield");
        if let Some(args) = node.arguments() {
            // Parens around yield's arguments are load-bearing, not
            // decorative: `cond ? yield(x) : y` is valid Ruby while
            // `cond ? yield x : y` is a syntax error. Keep whichever form
            // the source used.
            let parenthesized = node.lparen_loc().is_some();
            self.output.push(if parenthesized { '(' } else { ' ' });
            for (i, arg) in args.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
            if parenthesized {
                self.output.push(')');
            }
        }
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_return_node(&mut self, node: &ruby_prism::ReturnNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("return");
        if let Some(args) = node.arguments() {
            self.output.push(' ');
            for (i, arg) in args.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
        }
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_break_node(&mut self, node: &ruby_prism::BreakNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("break");
        if let Some(args) = node.arguments() {
            self.output.push(' ');
            for (i, arg) in args.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
        }
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_next_node(&mut self, node: &ruby_prism::NextNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("next");
        if let Some(args) = node.arguments() {
            self.output.push(' ');
            for (i, arg) in args.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
        }
        if !inline {
            self.output.push('\n');
        }
    }
}
