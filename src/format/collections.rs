use super::Formatter;
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
        self.write_leaf(|f| {
            let bracketed = node.opening_loc().is_some();
            if bracketed {
                f.output.push('[');
            }
            for (i, element) in node.elements().iter().enumerate() {
                if i > 0 {
                    f.output.push_str(", ");
                }
                f.inline = true;
                f.visit(&element);
            }
            if bracketed {
                f.output.push(']');
            }
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
