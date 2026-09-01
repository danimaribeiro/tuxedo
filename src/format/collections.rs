use super::{Formatter, assoc_has_real_block, has_real_block};
use pretty::RcDoc as Doc;
use ruby_prism::Visit;

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

        if !bracketed || elements.is_empty() || elements.iter().any(has_real_block) {
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

        self.write_leaf(|f| {
            let leaves: Vec<_> = elements
                .iter()
                .map(|element| {
                    f.capture_as_doc(|f| {
                        f.inline = true;
                        f.visit(element);
                    })
                })
                .collect();

            let nest_width = f.indent_width() + f.indent_str.len();
            let doc = Doc::text("[")
                .append(
                    Doc::intersperse(leaves, Doc::text(",").append(Doc::line()))
                        .nest(nest_width as isize)
                        .group(),
                )
                .append(Doc::text("]"));

            let rendered = f.render_doc_to_string(doc);
            f.output.push_str(&rendered);
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

            let nest_width = f.indent_width() + f.indent_str.len();
            let opening = String::from_utf8_lossy(&opening).into_owned();
            let closing = String::from_utf8_lossy(&closing).into_owned();
            let doc = Doc::text(opening)
                .append(
                    Doc::intersperse(leaves, Doc::line())
                        .nest(nest_width as isize)
                        .group(),
                )
                .append(Doc::text(closing));

            let rendered = f.render_doc_to_string(doc);
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
        if elements.is_empty() {
            self.write_leaf(|f| f.output.push_str("{}"));
            return;
        }

        if elements.iter().any(assoc_has_real_block) {
            self.write_leaf(|f| {
                f.output.push_str("{ ");
                for (i, element) in elements.iter().enumerate() {
                    if i > 0 {
                        f.output.push_str(", ");
                    }
                    f.inline = true;
                    f.visit(element);
                }
                f.output.push_str(" }");
            });
            return;
        }

        self.write_leaf(|f| {
            let leaves: Vec<_> = elements
                .iter()
                .map(|element| {
                    f.capture_as_doc(|f| {
                        f.inline = true;
                        f.visit(element);
                    })
                })
                .collect();

            let nest_width = f.indent_width() + f.indent_str.len();
            let doc = Doc::text("{ ")
                .append(
                    Doc::intersperse(leaves, Doc::text(",").append(Doc::line()))
                        .nest(nest_width as isize)
                        .group(),
                )
                .append(Doc::text(" }"));

            let rendered = f.render_doc_to_string(doc);
            f.output.push_str(&rendered);
        });
    }

    pub(super) fn fmt_assoc_node(&mut self, node: &ruby_prism::AssocNode<'_>) {
        self.inline = true;
        self.visit(&node.key());
        if let Some(op) = node.operator_loc() {
            self.output.push(' ');
            self.output
                .push_str(std::str::from_utf8(op.as_slice()).unwrap_or("=>"));
            self.output.push(' ');
        } else {
            self.output.push(' ');
        }
        self.inline = true;
        self.visit(&node.value());
    }
}
