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

/// The `.`/`&.` a chain link is reached through, as written.
fn call_operator<'a>(call: &'a ruby_prism::CallNode<'a>) -> &'a str {
    call.call_operator_loc()
        .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
        .unwrap_or(".")
}

/// A call's text before its block, in two parts: everything up to the
/// argument list, and the list itself.
///
/// They are kept apart because the room a following ` do |params|` header
/// takes has to come off the argument list and nothing else. The receiver
/// has had its own line by the time the header lands, so holding room back
/// from it only makes a collection receiver expand to leave space it is not
/// competing for -- `[...].each do |camel, under, human, title|` broke its
/// array one element per line for a header 32 columns away.
struct CallHead {
    prefix: Doc,
    args: Doc,
}

impl CallHead {
    /// A call whose shape has no argument list to hold room back from.
    fn whole(doc: Doc) -> Self {
        CallHead {
            prefix: doc,
            args: Doc::nil(),
        }
    }
}

/// A call's real `{ }` / `do...end` block, as opposed to a block-pass
/// argument (`&:sym`), which Prism stores in the same slot but which
/// belongs inside the argument list.
fn real_block<'a>(node: &ruby_prism::CallNode<'a>) -> Option<ruby_prism::Node<'a>> {
    match node.block() {
        Some(block @ ruby_prism::Node::BlockNode { .. }) => Some(block),
        _ => None,
    }
}

impl Formatter {
    /// Renders `node` and its chainable receivers (see `collect_chain`) as
    /// one Doc group: either the whole chain fits on one line, or it breaks
    /// with a leading `.`/`&.` on every link, each at a fixed indent from
    /// the statement -- the same all-or-nothing, block-indented shape as an
    /// overlong array/call-args list, just applied to `.method` calls
    /// instead of comma-separated elements. Returns `None` when there's no
    /// real chain to break, so the caller should fall back to its normal
    /// single-call path.
    ///
    /// Each link is a composed `Doc`, so an argument list of its own that
    /// has to wrap nests one level deeper than the link it belongs to --
    /// the indent a `Doc` carries is relative to the one already in effect,
    /// which here is the chain's own.
    fn try_build_chain_doc(&mut self, node: &ruby_prism::CallNode<'_>) -> Option<Doc> {
        let (base, links) = collect_chain(node)?;

        // The base stays in `output` while the links are built, so a link's
        // arguments measure the column the base leaves the line at rather
        // than column zero.
        let mut run = self.begin_run();
        self.run_node(&mut run, &base);
        // Every link sits one indent level deeper than the statement once
        // the chain breaks. A `Doc` of a link's own counts its indent from
        // the chain's, so it needs no help -- but an argument list with a
        // comment in it is still written out at `self.indent` (see
        // `write_commented_bracketed_items`), and so is any argument that
        // lays out its own body, and those have to be told.
        self.indent += 1;
        let segments: Vec<Doc> = links
            .iter()
            .chain(std::iter::once(node))
            .map(|link| {
                let args = self.call_args_doc(link, link.opening_loc().is_some());
                Doc::text(call_operator(link))
                    .append(super::bytes_doc(link.name().as_slice()))
                    .append(args)
            })
            .collect();
        self.indent -= 1;
        let base_doc = self.end_run(run);

        let nest_width = self.indent_str.len();
        let tail = Doc::concat(segments.into_iter().map(|seg| Doc::softline().append(seg)));
        Some(self.join_after_head(base_doc, tail.indent(nest_width).group()))
    }

    pub(super) fn fmt_call_node(&mut self, node: &ruby_prism::CallNode<'_>) -> Doc {
        // A comment between the links of a chain, or between the operands
        // of an operator.
        if node.receiver().is_some()
            && let Some(doc) = self.verbatim_doc_if_commented(&node.as_node())
        {
            return doc;
        }

        // ` do |params|` follows the argument list on the same line, and
        // the block is not part of the document the list is measured in
        // (see `with_block_doc`), so the room it takes has to be held back
        // by hand.
        let reserve = self.do_end_header_width(node.block());
        self.trailing_reserve += reserve;
        let head = self.call_head_doc(node);
        self.trailing_reserve -= reserve;

        match real_block(node) {
            Some(block) => self.with_block_doc(head, &block, reserve),
            None => self.join_after_head(head.prefix, head.args),
        }
    }

    /// A call without its block: the receiver, the message and the
    /// arguments, in whichever of Ruby's spellings the call uses.
    fn call_head_doc(&mut self, node: &ruby_prism::CallNode<'_>) -> CallHead {
        let name = node.name();
        let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");

        let Some(receiver) = node.receiver() else {
            let mut run = self.begin_run();
            self.run_text(&mut run, name_str);
            let args = self.call_args_doc(node, node.opening_loc().is_some());
            let prefix = self.end_run(run);
            return CallHead { prefix, args };
        };

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
            return CallHead::whole(self.index_call_doc(node, &receiver, name_str));
        }
        if matches!(name_str, "!" | "~" | "+@" | "-@")
            && node.arguments().is_none()
            && !explicit_dot
        {
            return CallHead::whole(self.unary_call_doc(node, &receiver, name_str));
        }
        if is_named_call && node.is_attribute_write() {
            return CallHead::whole(self.attribute_write_doc(node, &receiver, name_str));
        }
        if is_identifier_call {
            // The chain is one group and the block header follows all of
            // it, so the whole thing is the part to hold room back from.
            if let Some(doc) = self.try_build_chain_doc(node) {
                return CallHead {
                    prefix: Doc::nil(),
                    args: doc,
                };
            }
            let mut run = self.begin_run();
            self.run_node(&mut run, &receiver);
            self.run_text(&mut run, call_operator(node));
            self.run_text(&mut run, name_str);
            let args = self.call_args_doc(node, node.opening_loc().is_some());
            let prefix = self.end_run(run);
            return CallHead { prefix, args };
        }
        CallHead::whole(self.operator_call_doc(node, receiver, name_str))
    }

    /// `recv[a, b]` and `recv[a] = value` -- the `[]`/`[]=` methods in
    /// their bracket spelling.
    fn index_call_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        receiver: &ruby_prism::Node<'_>,
        name_str: &str,
    ) -> Doc {
        let mut args: Vec<_> = node
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let value = if name_str == "[]=" { args.pop() } else { None };

        let mut run = self.begin_run();
        self.run_node(&mut run, receiver);
        self.run_text(&mut run, "[");
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.run_text(&mut run, ", ");
            }
            self.run_node(&mut run, arg);
        }
        self.run_text(&mut run, "]");
        if let Some(value) = value {
            self.run_text(&mut run, " = ");
            self.run_node(&mut run, &value);
        }
        self.end_run(run)
    }

    /// A unary operator method (defined as `def +@`/`def -@`/`def ~`/
    /// `def !`) is called in prefix notation (`+x`), unlike every other
    /// operator method here, which is infix (`a + b`).
    /// `operator_call_doc` assumes infix and would render this
    /// receiver-first (`x +@`, dropping the fact there's no actual
    /// right-hand side) -- invalid Ruby.
    fn unary_call_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        receiver: &ruby_prism::Node<'_>,
        name_str: &str,
    ) -> Doc {
        // `not x` is the `!` method too, but binds far looser: `not a == b`
        // is `!(a == b)` while `!a == b` is `(!a) == b`. Keep the keyword,
        // and its parentheses.
        let is_not_keyword = node
            .message_loc()
            .is_some_and(|loc| loc.as_slice() == b"not");
        let not_parenthesized = is_not_keyword && node.opening_loc().is_some();

        let mut run = self.begin_run();
        if is_not_keyword {
            self.run_text(&mut run, if not_parenthesized { "not(" } else { "not " });
        } else {
            self.run_text(&mut run, name_str.strip_suffix('@').unwrap_or(name_str));
        }
        // `+ 1.second` calls `+@` on `1.second`; `+1.second` calls
        // `second` on the literal `+1`. Only a space keeps a sign from
        // fusing with a numeric literal that follows it.
        let receiver_starts_with_digit = self
            .source
            .as_bytes()
            .get(receiver.location().start_offset())
            .is_some_and(u8::is_ascii_digit);
        if matches!(name_str, "+@" | "-@") && receiver_starts_with_digit {
            self.run_text(&mut run, " ");
        }
        self.run_node(&mut run, receiver);
        if not_parenthesized {
            self.run_text(&mut run, ")");
        }
        self.end_run(run)
    }

    /// `recv.attr = value` -- the `attr=` method in its assignment
    /// spelling.
    fn attribute_write_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        receiver: &ruby_prism::Node<'_>,
        name_str: &str,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, receiver);
        self.run_text(&mut run, call_operator(node));
        self.run_text(&mut run, name_str.strip_suffix('=').unwrap_or(name_str));
        self.run_text(&mut run, " = ");
        if let Some(value) = node.arguments().and_then(|a| a.arguments().iter().next()) {
            self.run_node(&mut run, &value);
        }
        self.end_run(run)
    }

    /// Operator-style call (`a + b`, `a <=> b`, ...) as one all-or-nothing
    /// group, so an overlong line breaks after each operator with the
    /// operands indented one level instead of rendering flat regardless of
    /// width. `a + b + c` is nested `CallNode`s, and one group per node
    /// bin-packs, so the same-operator spine is flattened into one group
    /// first -- the same shape, and the same helper, as `&&`/`||`.
    ///
    /// Falls back to the flat shape when an operand lays out its own
    /// multi-line body (`if`/`case`/`begin`/a `do...end` call): no line
    /// point of this group could move it.
    fn operator_call_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        receiver: ruby_prism::Node<'_>,
        name_str: &str,
    ) -> Doc {
        let single_arg = node
            .arguments()
            .map(|a| a.arguments().iter().collect::<Vec<_>>())
            .filter(|args| args.len() == 1)
            .map(|mut args| args.remove(0));

        let Some(arg) = single_arg else {
            let mut run = self.begin_run();
            self.run_node(&mut run, &receiver);
            self.run_text(&mut run, " ");
            self.run_text(&mut run, name_str);
            if let Some(args) = node.arguments() {
                self.run_text(&mut run, " ");
                for (i, arg) in args.arguments().iter().enumerate() {
                    if i > 0 {
                        self.run_text(&mut run, ", ");
                    }
                    self.run_node(&mut run, &arg);
                }
            }
            return self.end_run(run);
        };

        // An operand that lays out its own multi-line body ends the
        // grouping here: this pair goes flat and whatever is nested in the
        // receiver still gets a group of its own.
        if manages_own_indentation(&receiver) || manages_own_indentation(&arg) {
            return self.flat_infix_doc(&[receiver, arg], name_str);
        }

        let mut operands = Vec::new();
        Self::operator_operands(node, name_str, &mut operands);
        operands.push(arg);

        let leaves: Vec<Doc> = operands.iter().map(|o| self.nested_node_doc(o)).collect();
        self.infix_chain_doc(leaves, name_str)
    }

    /// The operands of `node`'s receiver spine, in source order, as long as
    /// it stays the same operator written the same way (`a + b + c` is
    /// `+(+(a, b), c)`). Anything else -- a different message, an explicit
    /// `.`, a block, more or fewer than one argument -- ends the spine and
    /// becomes an operand of its own.
    fn operator_operands<'a>(
        node: &ruby_prism::CallNode<'a>,
        name_str: &str,
        out: &mut Vec<ruby_prism::Node<'a>>,
    ) {
        let Some(receiver) = node.receiver() else {
            return;
        };
        let inner = receiver.as_call_node().filter(|call| {
            call.call_operator_loc().is_none()
                && call.block().is_none()
                && call.receiver().is_some()
                && call.name().as_slice() == name_str.as_bytes()
                && call
                    .arguments()
                    .is_some_and(|a| a.arguments().iter().count() == 1)
        });
        match inner {
            Some(inner) => {
                Self::operator_operands(&inner, name_str, out);
                if let Some(arg) = inner.arguments().and_then(|a| a.arguments().iter().next()) {
                    out.push(arg);
                }
            }
            None => out.push(receiver),
        }
    }

    /// `head` with a real `{ }` / `do...end` block appended.
    ///
    /// A block lays its own body out imperatively, with raw newlines and
    /// absolute indents, so it cannot share a document with a group the
    /// printer has not laid out yet: the printer measures a text leaf as
    /// one long line, and the argument list in front of it would break for
    /// want of room the body does not really take on that line. `head` is
    /// rendered here, where it will sit, and the whole thing comes back as
    /// one leaf -- with `reserve` held back for the ` do |params|` header
    /// the block is about to append (see `do_end_header_width`).
    fn with_block_doc(
        &mut self,
        head: CallHead,
        block: &ruby_prism::Node<'_>,
        reserve: usize,
    ) -> Doc {
        self.spliced_doc(|f| {
            let rendered = f.render_doc_to_string(head.prefix);
            f.output.push_str(&rendered);
            f.trailing_reserve += reserve;
            let rendered = f.render_doc_to_string(head.args);
            f.trailing_reserve -= reserve;
            f.output.push_str(&rendered);
            // A block always continues the line its call ends on: it writes
            // no indent of its own and must not be terminated with a
            // newline.
            f.inline = true;
            f.visit(block);
        })
    }

    pub(super) fn fmt_super_node(&mut self, node: &ruby_prism::SuperNode<'_>) -> Doc {
        let positional: Vec<_> = node
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let block_pass = match node.block() {
            Some(b @ ruby_prism::Node::BlockArgumentNode { .. }) => Some(b),
            _ => None,
        };
        let block = match node.block() {
            Some(block @ ruby_prism::Node::BlockNode { .. }) => Some(block),
            _ => None,
        };
        let span = node
            .lparen_loc()
            .zip(node.rparen_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));

        let reserve = self.do_end_header_width(node.block());
        self.trailing_reserve += reserve;
        let mut run = self.begin_run();
        self.run_text(&mut run, "super");
        let args = self.arg_list_doc(node.lparen_loc().is_some(), positional, block_pass, span);
        let head = CallHead {
            prefix: self.end_run(run),
            args,
        };
        self.trailing_reserve -= reserve;

        match block {
            Some(block) => self.with_block_doc(head, &block, reserve),
            None => self.join_after_head(head.prefix, head.args),
        }
    }

    pub(super) fn fmt_forwarding_super_node(
        &mut self,
        node: &ruby_prism::ForwardingSuperNode<'_>,
    ) -> Doc {
        let Some(block) = node.block() else {
            return Doc::text("super");
        };
        self.spliced_doc(|f| {
            f.output.push_str("super");
            f.write_block_node(&block);
        })
    }

    pub(super) fn fmt_yield_node(&mut self, node: &ruby_prism::YieldNode<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, "yield");
        let Some(args) = node.arguments() else {
            return self.end_run(run);
        };
        // Parens around yield's arguments are load-bearing, not decorative:
        // `cond ? yield(x) : y` is valid Ruby while `cond ? yield x : y` is
        // a syntax error. Keep whichever form the source used.
        let span = node
            .lparen_loc()
            .zip(node.rparen_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));
        let doc = self.arg_list_doc(
            node.lparen_loc().is_some(),
            args.arguments().iter().collect(),
            None,
            span,
        );
        let prefix = self.end_run(run);
        self.join_after_head(prefix, doc)
    }

    pub(super) fn fmt_return_node(&mut self, node: &ruby_prism::ReturnNode<'_>) -> Doc {
        self.jump_doc("return", node.arguments())
    }

    pub(super) fn fmt_break_node(&mut self, node: &ruby_prism::BreakNode<'_>) -> Doc {
        self.jump_doc("break", node.arguments())
    }

    pub(super) fn fmt_next_node(&mut self, node: &ruby_prism::NextNode<'_>) -> Doc {
        self.jump_doc("next", node.arguments())
    }

    /// `return`/`break`/`next`, with their values comma-joined on the same
    /// line. No line point: a jump with more than one value is rare enough
    /// that a wrapped shape has never been needed, and `arg_list_doc`'s is
    /// not it -- these take no parentheses.
    fn jump_doc(&mut self, keyword: &str, arguments: Option<ruby_prism::ArgumentsNode<'_>>) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, keyword);
        if let Some(arguments) = arguments {
            self.run_text(&mut run, " ");
            for (i, arg) in arguments.arguments().iter().enumerate() {
                if i > 0 {
                    self.run_text(&mut run, ", ");
                }
                self.run_node(&mut run, &arg);
            }
        }
        self.end_run(run)
    }
}
