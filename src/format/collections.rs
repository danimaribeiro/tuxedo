use super::{Formatter, assoc_forces_list_break};
use pretty::RcDoc as Doc;

impl Formatter {
    pub(super) fn fmt_splat_node(&mut self, node: &ruby_prism::SplatNode<'_>) {
        self.output.push('*');
        if let Some(expression) = node.expression() {
            self.inline = true;
            self.visit(&expression);
        }
    }

    pub(super) fn fmt_block_argument_node(&mut self, node: &ruby_prism::BlockArgumentNode<'_>) {
        self.output.push('&');
        if let Some(expression) = node.expression() {
            self.inline = true;
            self.visit(&expression);
        }
    }

    pub(super) fn fmt_array_node(&mut self, node: &ruby_prism::ArrayNode<'_>) {
        let opening = node.opening_loc().map(|loc| loc.as_slice());
        let elements: Vec<_> = node.elements().iter().collect();

        if opening.is_some_and(|o| o.len() >= 2 && matches!(&o[..2], b"%w" | b"%i" | b"%W" | b"%I"))
        {
            self.fmt_percent_array(node, &elements);
            return;
        }

        let bracketed = opening.is_some();

        if !bracketed || elements.is_empty() {
            self.write_leaf(|f| {
                if bracketed {
                    f.output.push('[');
                }
                for (i, element) in elements.iter().enumerate() {
                    if i > 0 {
                        f.output.push_str(", ");
                    }
                    f.inline = true;
                    f.visit(element);
                }
                if bracketed {
                    f.output.push(']');
                }
            });
            return;
        }

        let span = node
            .opening_loc()
            .zip(node.closing_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));
        self.write_leaf(|f| {
            f.write_bracketed_items("[", Doc::line_(), &elements, Doc::line_(), "]", span);
        });
    }

    /// Renders a `%w()`/`%i()`/`%W()`/`%I()` word/symbol list, preserving
    /// its original percent-literal syntax (and whichever delimiter
    /// characters it used) rather than expanding it into a bracketed,
    /// comma-separated array of quoted strings/symbols.
    fn fmt_percent_array(
        &mut self,
        node: &ruby_prism::ArrayNode<'_>,
        elements: &[ruby_prism::Node<'_>],
    ) {
        let opening = node.opening_loc().unwrap().as_slice().to_vec();
        let closing = node.closing_loc().unwrap().as_slice().to_vec();

        if elements.is_empty() {
            self.write_leaf(|f| {
                f.write_bytes(&opening);
                f.write_bytes(&closing);
            });
            return;
        }

        self.write_leaf(|f| {
            let leaves: Vec<_> = elements
                .iter()
                .map(|element| f.fmt_percent_array_word(element))
                .collect();

            let opening = String::from_utf8_lossy(&opening).into_owned();
            let closing = String::from_utf8_lossy(&closing).into_owned();
            let body = Doc::intersperse(leaves, Doc::line());
            let rendered =
                f.wrap_bracketed_group(&opening, Doc::line_(), body, Doc::line_(), &closing);
            f.output.push_str(&rendered);
        });
    }

    /// Renders one element of a `%w`/`%i`/`%W`/`%I` list as its bare word,
    /// with no quotes/colon (those delimiters don't exist for an
    /// individual word in this syntax — only the list as a whole has
    /// opening/closing punctuation).
    fn fmt_percent_array_word(&mut self, element: &ruby_prism::Node<'_>) -> Doc<'static, ()> {
        if let Some(interpolated) = element.as_interpolated_string_node() {
            let parts = interpolated.parts();
            self.capture_as_doc(|f| f.fmt_interpolated_parts(&parts))
        } else if let Some(interpolated) = element.as_interpolated_symbol_node() {
            let parts = interpolated.parts();
            self.capture_as_doc(|f| f.fmt_interpolated_parts(&parts))
        } else {
            // A plain word (StringNode for %w/%W, SymbolNode for %i/%I) has
            // no delimiters in source at all -- its raw location is
            // already exactly the bare word. Copying it verbatim (rather
            // than through `unescaped()`) also preserves an escaped space
            // like `\ ` as-is instead of resolving it to an actual space,
            // which would silently merge two words back into one.
            Doc::text(String::from_utf8_lossy(element.location().as_slice()).into_owned())
        }
    }

    pub(super) fn fmt_range_node(&mut self, node: &ruby_prism::RangeNode<'_>) {
        if let Some(left) = node.left() {
            self.inline = true;
            self.visit(&left);
        }
        let op = std::str::from_utf8(node.operator_loc().as_slice()).unwrap_or("..");
        self.output.push_str(op);
        if let Some(right) = node.right() {
            self.inline = true;
            self.visit(&right);
        }
    }

    pub(super) fn fmt_hash_node(&mut self, node: &ruby_prism::HashNode<'_>) {
        let elements: Vec<_> = node.elements().iter().collect();
        let braces = (
            node.opening_loc().end_offset(),
            node.closing_loc().start_offset(),
        );
        self.fmt_hash_elements(&elements, Some(braces));
    }

    /// Bare keyword arguments at a call site (`foo(a, k: 1, j: 2)`). Prism
    /// parses these as a `KeywordHashNode`, *not* a `HashNode` — they are
    /// the same comma-joined pairs but carry no braces of their own.
    pub(super) fn fmt_keyword_hash_node(&mut self, node: &ruby_prism::KeywordHashNode<'_>) {
        let elements: Vec<_> = node.elements().iter().collect();
        self.fmt_hash_elements(&elements, None);
    }

    /// Renders a run of `key => value` / `key: value` pairs, optionally
    /// wrapped in `{ }`. Shared by braced hash literals and bare keyword
    /// argument lists so the two can never drift apart on separators.
    ///
    /// `braces` is the source range between the braces of a braced hash
    /// (`None` for bare keyword arguments).
    fn fmt_hash_elements(
        &mut self,
        elements: &[ruby_prism::Node<'_>],
        braces: Option<(usize, usize)>,
    ) {
        let braced = braces.is_some();
        if elements.is_empty() {
            if braced {
                self.write_leaf(|f| f.output.push_str("{}"));
            }
            return;
        }

        if braced {
            self.write_leaf(|f| {
                f.write_bracketed_items("{", Doc::line(), elements, Doc::line(), "}", braces);
            });
            return;
        }

        // Bare keyword args at a call site with no braces of their own.
        // Call args flatten KeywordHashNode before wrapping, so this is
        // the fallback if a keyword hash is visited directly.
        self.write_leaf(|f| {
            if elements.iter().any(assoc_forces_list_break) {
                f.write_expanded_items(elements);
                return;
            }
            let snap = f.capture_snapshot();
            let captured: Vec<String> = elements.iter().map(|e| f.capture_inline(e)).collect();
            if captured.iter().any(|s| s.contains('\n')) {
                f.restore_capture_snapshot(snap);
                f.write_expanded_items(elements);
                return;
            }
            let leaves = captured.into_iter().map(Doc::text);
            let body = Doc::intersperse(leaves, Doc::text(",").append(Doc::line()));
            let nest_width = f.indent_width() + f.indent_str.len();
            let doc = body.nest(nest_width as isize).group();
            let rendered = f.render_doc_to_string(doc);
            f.output.push_str(&rendered);
        });
    }

    /// A double-splat entry inside a hash literal or keyword argument list
    /// (`{ a: 1, **rest }`, `foo(**opts)`). The bare `**` form used to
    /// forward an anonymous kwrest has no value.
    pub(super) fn fmt_assoc_splat_node(&mut self, node: &ruby_prism::AssocSplatNode<'_>) {
        self.write_bytes(node.operator_loc().as_slice());
        if let Some(value) = node.value() {
            self.inline = true;
            self.visit(&value);
        }
    }

    pub(super) fn fmt_assoc_node(&mut self, node: &ruby_prism::AssocNode<'_>) {
        self.inline = true;
        self.visit(&node.key());
        let value = node.value();
        // Shorthand `key:` (3.1+ hash-literal value-omission, or a pattern
        // -match bind) has an `ImplicitNode` standing in for the omitted
        // value -- the key's own trailing `:` already says everything, so
        // there's nothing more to print. Printing it anyway would silently
        // rewrite `{ x: }` into `{ x: x }`, or worse, turn a pattern-match
        // bind into a match against an existing variable's current value.
        if matches!(value, ruby_prism::Node::ImplicitNode { .. }) {
            return;
        }
        if let Some(op) = node.operator_loc() {
            self.output.push(' ');
            self.output
                .push_str(std::str::from_utf8(op.as_slice()).unwrap_or("=>"));
            self.output.push(' ');
        } else {
            self.output.push(' ');
        }
        self.inline = true;
        self.visit(&value);
    }

    /// `{ key: value, ... }` / `ClassName[key: value, ...]` as a pattern
    /// (as opposed to `fmt_hash_node`'s hash *literal*) -- e.g. the
    /// right-hand side of `expr => { key: value }` or `expr in
    /// ClassName[key:]`. Kept as a plain imperative leaf rather than a
    /// Doc-wrapped group like the literal: patterns are short in practice
    /// and rarely need to wrap.
    pub(super) fn fmt_hash_pattern_node(&mut self, node: &ruby_prism::HashPatternNode<'_>) {
        if let Some(constant) = node.constant() {
            self.inline = true;
            self.visit(&constant);
        }

        let opening = node
            .opening_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"{".to_vec());
        let closing = node
            .closing_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"}".to_vec());
        let braced = opening == b"{";

        self.write_bytes(&opening);
        let elements: Vec<_> = node.elements().iter().collect();
        let has_content = !elements.is_empty() || node.rest().is_some();
        if braced && has_content {
            self.output.push(' ');
        }
        for (i, element) in elements.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(element);
        }
        if let Some(rest) = node.rest() {
            if !elements.is_empty() {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&rest);
        }
        if braced && has_content {
            self.output.push(' ');
        }
        self.write_bytes(&closing);
    }

    /// `[a, b, *rest, c]` / `ClassName[a, b]` as a pattern (as opposed to
    /// `fmt_array_node`'s array *literal*). Same rationale as
    /// `fmt_hash_pattern_node` for staying a plain imperative leaf.
    pub(super) fn fmt_array_pattern_node(&mut self, node: &ruby_prism::ArrayPatternNode<'_>) {
        if let Some(constant) = node.constant() {
            self.inline = true;
            self.visit(&constant);
        }

        let opening = node
            .opening_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"[".to_vec());
        let closing = node
            .closing_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"]".to_vec());
        self.write_bytes(&opening);

        let mut wrote_any = false;
        for required in node.requireds().iter() {
            if wrote_any {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&required);
            wrote_any = true;
        }
        if let Some(rest) = node.rest() {
            if wrote_any {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&rest);
            wrote_any = true;
        }
        for post in node.posts().iter() {
            if wrote_any {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&post);
            wrote_any = true;
        }

        self.write_bytes(&closing);
    }

    /// The omitted value of a shorthand `key:` entry (hash-literal
    /// value-omission or a pattern-match bind) -- see the comment in
    /// `fmt_assoc_node`, which returns before ever visiting this node in
    /// practice. Renders nothing; kept only so an unexpected direct visit
    /// doesn't fall through to prism's default (which would print the
    /// implied variable's read/target, defeating the point).
    pub(super) fn fmt_implicit_node(&mut self, _node: &ruby_prism::ImplicitNode<'_>) {}
}
