use super::Formatter;

impl Formatter {
    pub(super) fn fmt_true_node(&mut self, _node: &ruby_prism::TrueNode<'_>) {
        self.write_leaf(|f| f.output.push_str("true"));
    }

    pub(super) fn fmt_false_node(&mut self, _node: &ruby_prism::FalseNode<'_>) {
        self.write_leaf(|f| f.output.push_str("false"));
    }

    pub(super) fn fmt_nil_node(&mut self, _node: &ruby_prism::NilNode<'_>) {
        self.write_leaf(|f| f.output.push_str("nil"));
    }

    pub(super) fn fmt_integer_node(&mut self, node: &ruby_prism::IntegerNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_string_node(&mut self, node: &ruby_prism::StringNode<'_>) {
        let unescaped = node.unescaped();
        self.write_leaf(|f| {
            f.output.push('"');
            f.write_bytes(unescaped);
            f.output.push('"');
        });
    }

    pub(super) fn fmt_symbol_node(&mut self, node: &ruby_prism::SymbolNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_float_node(&mut self, node: &ruby_prism::FloatNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }
}
