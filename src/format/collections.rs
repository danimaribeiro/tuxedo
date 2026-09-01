use super::Formatter;
use pretty::RcDoc as Doc;
use ruby_prism::Visit;

/// True if `node` is a call with a real `{ }`/`do...end` block (as opposed to
/// a `&block` pass). Such a block always hard-breaks onto its own lines
/// regardless of width, which isn't the kind of flat-or-broken choice a Doc
/// `group` is meant to arbitrate — a list containing one is left on the
/// existing, always-imperative rendering path instead of the Doc path.
fn has_real_block(node: &ruby_prism::Node<'_>) -> bool {
    node.as_call_node()
        .is_some_and(|call| matches!(call.block(), Some(ruby_prism::Node::BlockNode { .. })))
}

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
        let bracketed = node.opening_loc().is_some();
        let elements: Vec<_> = node.elements().iter().collect();

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
        let elements = node.elements();
        if elements.iter().next().is_none() {
            self.write_leaf(|f| f.output.push_str("{}"));
            return;
        }
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("{ ");
        for (i, element) in elements.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&element);
        }
        self.output.push_str(" }");
        if !inline {
            self.output.push('\n');
        }
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
