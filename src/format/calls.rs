use super::doc::Doc;
use super::{Formatter, has_real_block, manages_own_indentation};

/// True for a call that can be a link in a `.method` chain: a plain
/// identifier-style call (not `[]`/`[]=`, not an attribute write, not a
/// `!`-style unary) with no real block of its own. A link with a block
/// changes where the chain visually has to break, so `collect_chain` stops
/// peeling there rather than trying to fold it in.
fn is_plain_method_call(call: &ruby_prism::CallNode<'_>) -> bool {
    let name = call.name();
    let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");
    let is_index = name_str == "[]" || name_str == "[]=";
    let is_identifier_call = name_str
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_');
    is_identifier_call
        && !is_index
        && !call.is_attribute_write()
        && !has_real_block(&call.as_node())
}

/// Walks a call's receiver chain (`a.b.c.d` -> `d` is `node`) as far back as
/// it stays a plain method call (see `is_plain_method_call`), returning the
/// non-call (or otherwise unchainable) node the chain bottoms out at and
/// the links in between, in base-to-`node` order -- `node` itself is not
/// included, since the caller already holds it. `None` means there's
/// nothing to chain (the receiver isn't itself a chainable call), so the
/// caller should fall back to its normal single-call rendering.
fn collect_chain<'a>(
    node: &ruby_prism::CallNode<'a>,
) -> Option<(ruby_prism::Node<'a>, Vec<ruby_prism::CallNode<'a>>)> {
    let mut links = Vec::new();
    let mut base = node.receiver()?;

    while let Some(call) = base.as_call_node() {
        if !is_plain_method_call(&call) {
            break;
        }
        match call.receiver() {
            Some(next_base) => {
                links.push(call);
                base = next_base;
            }
            None => break,
        }
    }

    if links.is_empty() {
        None
    } else {
        links.reverse();
        Some((base, links))
    }
}

impl Formatter {
    /// Renders `node` and its chainable receivers (see `collect_chain`) as
    /// one Doc group: either the whole chain fits on one line, or it breaks
    /// with a leading `.`/`&.` on every link, each at a fixed indent from
    /// the statement -- the same all-or-nothing, block-indented shape as an
    /// overlong array/call-args list, just applied to `.method` calls
    /// instead of comma-separated elements. Returns `None` (rendering
    /// nothing) when there's no real chain to break, so the caller should
    /// fall back to its normal single-call path.
    fn try_build_chain_doc(&mut self, node: &ruby_prism::CallNode<'_>) -> Option<String> {
        let (base, links) = collect_chain(node)?;

        let base_doc = self.capture_as_doc(|f| {
            f.inline = true;
            f.visit(&base);
        });

        let total = links.len() + 1;
        // Every segment renders one indent level deeper than the
        // statement once the chain breaks (they all follow a
        // `Doc::softline()` nested at `nest_width` below). A segment's own
        // argument list, if it also needs to wrap, has to know that --
        // write_call_args nests relative to `self.indent`, which a plain
        // capture wouldn't otherwise reflect, misindenting its args and
        // closing paren by one level short.
        self.indent += 1;
        let segments: Vec<_> = links
            .iter()
            .chain(std::iter::once(node))
            .enumerate()
            .map(|(i, link)| {
                let is_last = i + 1 == total;
                let op = link
                    .call_operator_loc()
                    .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
                    .unwrap_or(".");
                let link_name = link.name();
                let link_name_str = std::str::from_utf8(link_name.as_slice()).unwrap_or("");
                let parenthesized = link.opening_loc().is_some();
                self.capture_as_doc(|f| {
                    f.output.push_str(op);
                    f.output.push_str(link_name_str);
                    let reserve = if is_last {
                        f.do_end_header_width(node.block())
                    } else {
                        0
                    };
                    f.trailing_reserve += reserve;
                    f.write_call_args(link, parenthesized);
                    f.trailing_reserve -= reserve;
                })
            })
            .collect();
        self.indent -= 1;

        let nest_width = self.indent_width() + self.indent_str.len();
        let tail = Doc::concat(segments.into_iter().map(|seg| Doc::softline().append(seg)));
        let doc = base_doc.append(tail.indent(nest_width).group());
        Some(self.render_doc_to_string(doc))
    }

    pub(super) fn fmt_call_node(&mut self, node: &ruby_prism::CallNode<'_>) {
        let name = node.name();
        let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");
        // A comment between the links of a chain, or between the operands
        // of an operator.
        if node.receiver().is_some() && self.write_verbatim_if_commented(&node.as_node()) {
            return;
        }

        let inline = self.inline;
        self.inline = false;

        if let Some(receiver) = node.receiver() {
            if !inline {
                self.write_indent();
            }

            // An operator method called with an explicit `.`/`&.`
            // (`a&.+(1)`, `env&.[]("k")`, `x&.!= y`) is written as an
            // ordinary method call, and has to stay one: the sugar forms
            // below (`a + 1`, `env["k"]`) have no way to spell `&.`.
            let explicit_dot = node.call_operator_loc().is_some();
            let is_index = (name_str == "[]" || name_str == "[]=") && !explicit_dot;
            let is_named_call = name_str
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            let is_identifier_call = is_named_call || explicit_dot;

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
            } else if matches!(name_str, "!" | "~" | "+@" | "-@")
                && node.arguments().is_none()
                && !explicit_dot
            {
                // A unary operator method (defined as `def +@`/`def -@`/
                // `def ~`/`def !`) is called in prefix notation (`+x`),
                // unlike every other operator method here, which is infix
                // (`a + b`). The generic operator-style branch below
                // assumes infix and would render this receiver-first
                // (`x +@`, dropping the fact there's no actual right-hand
                // side) -- invalid Ruby.
                //
                // `not x` is the `!` method too, but binds far looser:
                // `not a == b` is `!(a == b)` while `!a == b` is
                // `(!a) == b`. Keep the keyword, and its parentheses.
                let is_not_keyword = node
                    .message_loc()
                    .is_some_and(|loc| loc.as_slice() == b"not");
                let not_parenthesized = is_not_keyword && node.opening_loc().is_some();
                if is_not_keyword {
                    self.output
                        .push_str(if not_parenthesized { "not(" } else { "not " });
                } else {
                    self.output
                        .push_str(name_str.strip_suffix('@').unwrap_or(name_str));
                }
                // `+ 1.second` calls `+@` on `1.second`; `+1.second` calls
                // `second` on the literal `+1`. Only a space keeps a sign
                // from fusing with a numeric literal that follows it.
                let receiver_starts_with_digit = self
                    .source
                    .as_bytes()
                    .get(receiver.location().start_offset())
                    .is_some_and(u8::is_ascii_digit);
                if matches!(name_str, "+@" | "-@") && receiver_starts_with_digit {
                    self.output.push(' ');
                }
                self.inline = true;
                self.visit(&receiver);
                if not_parenthesized {
                    self.output.push(')');
                }
            } else if is_named_call && node.is_attribute_write() {
                self.inline = true;
                self.visit(&receiver);
                let op = node
                    .call_operator_loc()
                    .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
                    .unwrap_or(".");
                self.output.push_str(op);
                self.output
                    .push_str(name_str.strip_suffix('=').unwrap_or(name_str));
                self.output.push_str(" = ");
                if let Some(value) = node.arguments().and_then(|a| a.arguments().iter().next()) {
                    self.inline = true;
                    self.visit(&value);
                }
            } else if is_identifier_call {
                if let Some(rendered) = self.try_build_chain_doc(node) {
                    self.output.push_str(&rendered);
                } else {
                    self.inline = true;
                    self.visit(&receiver);
                    let op = node
                        .call_operator_loc()
                        .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
                        .unwrap_or(".");
                    self.output.push_str(op);
                    self.output.push_str(name_str);
                    let parenthesized = node.opening_loc().is_some();
                    let reserve = self.do_end_header_width(node.block());
                    self.trailing_reserve += reserve;
                    self.write_call_args(node, parenthesized);
                    self.trailing_reserve -= reserve;
                }
            } else {
                // Operator-style call (`a + b`, `a <=> b`, ...). With
                // exactly one argument -- the overwhelmingly common case --
                // wrap it as a Doc so an overlong line breaks after the
                // operator with the argument indented one level, instead of
                // always rendering flat regardless of width.
                // Excludes an argument that renders its own multi-line body
                // imperatively (if/case/begin/a block-call): capture_as_doc
                // bakes in absolute indentation for such a body's *internal*
                // lines (from `self.indent` at capture time), but the Doc
                // wrapper below only nests the break right before the
                // captured text, not the raw newlines inside it -- so a
                // multi-line body's first line would shift by `nest_width`
                // while its `else`/`end` stayed put, misaligning them.
                let single_arg = node
                    .arguments()
                    .map(|a| a.arguments().iter().collect::<Vec<_>>())
                    .filter(|args| args.len() == 1)
                    .map(|mut args| args.remove(0))
                    .filter(|arg| !manages_own_indentation(arg));

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
                        .append(Doc::line().append(arg_doc).indent(nest_width).group());
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
            if !inline && !self.output.ends_with('\n') {
                self.output.push('\n');
            }
        } else {
            if !inline {
                self.write_indent();
            }
            self.output.push_str(name_str);
            let parenthesized = node.opening_loc().is_some();
            let reserve = self.do_end_header_width(node.block());
            self.trailing_reserve += reserve;
            self.write_call_args(node, parenthesized);
            self.trailing_reserve -= reserve;
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
        let span = node
            .lparen_loc()
            .zip(node.rparen_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));
        self.write_arg_list(parenthesized, positional, block_pass, span);

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
            let positional: Vec<_> = args.arguments().iter().collect();
            let span = node
                .lparen_loc()
                .zip(node.rparen_loc())
                .map(|(open, close)| (open.end_offset(), close.start_offset()));
            self.write_arg_list(parenthesized, positional, None, span);
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
