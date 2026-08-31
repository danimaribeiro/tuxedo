use super::Formatter;

impl Formatter {
    pub(super) fn fmt_local_variable_read_node(
        &mut self,
        node: &ruby_prism::LocalVariableReadNode<'_>,
    ) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_global_variable_read_node(
        &mut self,
        node: &ruby_prism::GlobalVariableReadNode<'_>,
    ) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_instance_variable_read_node(
        &mut self,
        node: &ruby_prism::InstanceVariableReadNode<'_>,
    ) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_class_variable_read_node(
        &mut self,
        node: &ruby_prism::ClassVariableReadNode<'_>,
    ) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_self_node(&mut self, _node: &ruby_prism::SelfNode<'_>) {
        self.write_leaf(|f| f.output.push_str("self"));
    }
}
