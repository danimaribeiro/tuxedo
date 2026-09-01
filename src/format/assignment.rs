use super::Formatter;
use ruby_prism::Visit;

impl Formatter {
    pub(super) fn fmt_local_variable_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    pub(super) fn fmt_local_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    pub(super) fn fmt_local_variable_or_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    pub(super) fn fmt_local_variable_and_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    pub(super) fn fmt_instance_variable_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    pub(super) fn fmt_instance_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    pub(super) fn fmt_instance_variable_or_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    pub(super) fn fmt_instance_variable_and_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    pub(super) fn fmt_constant_write_node(&mut self, node: &ruby_prism::ConstantWriteNode<'_>) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    pub(super) fn fmt_constant_operator_write_node(
        &mut self,
        node: &ruby_prism::ConstantOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    pub(super) fn fmt_constant_or_write_node(
        &mut self,
        node: &ruby_prism::ConstantOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    pub(super) fn fmt_constant_and_write_node(
        &mut self,
        node: &ruby_prism::ConstantAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    pub(super) fn fmt_global_variable_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    pub(super) fn fmt_global_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    pub(super) fn fmt_global_variable_or_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    pub(super) fn fmt_global_variable_and_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    pub(super) fn fmt_class_variable_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    pub(super) fn fmt_class_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    pub(super) fn fmt_class_variable_or_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    pub(super) fn fmt_class_variable_and_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    pub(super) fn fmt_constant_path_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathWriteNode<'_>,
    ) {
        let target = node.target();
        self.write_assignment_to(|f| f.visit(&target.as_node()), "=", &node.value());
    }

    pub(super) fn fmt_constant_path_operator_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathOperatorWriteNode<'_>,
    ) {
        let target = node.target();
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment_to(|f| f.visit(&target.as_node()), op, &node.value());
    }

    pub(super) fn fmt_constant_path_or_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathOrWriteNode<'_>,
    ) {
        let target = node.target();
        self.write_assignment_to(|f| f.visit(&target.as_node()), "||=", &node.value());
    }

    pub(super) fn fmt_constant_path_and_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathAndWriteNode<'_>,
    ) {
        let target = node.target();
        self.write_assignment_to(|f| f.visit(&target.as_node()), "&&=", &node.value());
    }

    pub(super) fn fmt_multi_write_node(&mut self, node: &ruby_prism::MultiWriteNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }

        let lefts = node.lefts();
        let rights = node.rights();
        let mut first = true;
        for target in lefts.iter() {
            if !first {
                self.output.push_str(", ");
            }
            first = false;
            self.visit(&target);
        }
        if let Some(rest) = node.rest() {
            // A bare trailing comma (`a, b, = expr`) parses as an
            // `ImplicitRestNode` with nothing following it — it should stay
            // a bare comma, not grow an explicit `*` (that's only correct
            // for an actual anonymous splat target like `a, *, b = expr`,
            // which parses as a `SplatNode` instead and is handled by the
            // `visit` below).
            if matches!(rest, ruby_prism::Node::ImplicitRestNode { .. }) {
                self.output.push(',');
            } else {
                if !first {
                    self.output.push_str(", ");
                }
                self.visit(&rest);
            }
            first = false;
        }
        for target in rights.iter() {
            if !first {
                self.output.push_str(", ");
            }
            first = false;
            self.visit(&target);
        }

        self.output.push_str(" = ");
        self.inline = true;
        self.visit(&node.value());
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_local_variable_target_node(
        &mut self,
        node: &ruby_prism::LocalVariableTargetNode<'_>,
    ) {
        self.write_bytes(node.name().as_slice());
    }

    pub(super) fn fmt_index_or_write_node(&mut self, node: &ruby_prism::IndexOrWriteNode<'_>) {
        self.write_index_write(node.receiver(), node.arguments(), "||=", &node.value());
    }

    pub(super) fn fmt_index_and_write_node(&mut self, node: &ruby_prism::IndexAndWriteNode<'_>) {
        self.write_index_write(node.receiver(), node.arguments(), "&&=", &node.value());
    }

    pub(super) fn fmt_index_operator_write_node(
        &mut self,
        node: &ruby_prism::IndexOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_index_write(node.receiver(), node.arguments(), op, &node.value());
    }

    /// Shared renderer for `receiver[args] op= value` (`IndexOrWriteNode`,
    /// `IndexAndWriteNode`, `IndexOperatorWriteNode`) — the compound-
    /// assignment counterpart to the plain `receiver[args] = value` case,
    /// which is a `CallNode` (`[]=`) handled in `calls.rs` instead.
    fn write_index_write(
        &mut self,
        receiver: Option<ruby_prism::Node<'_>>,
        arguments: Option<ruby_prism::ArgumentsNode<'_>>,
        op: &str,
        value: &ruby_prism::Node<'_>,
    ) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }

        if let Some(receiver) = receiver {
            self.inline = true;
            self.visit(&receiver);
        }
        self.output.push('[');
        if let Some(arguments) = arguments {
            for (i, arg) in arguments.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
        }
        self.output.push(']');
        self.output.push(' ');
        self.output.push_str(op);
        self.output.push(' ');

        self.inline = true;
        self.visit(value);

        if !inline {
            self.output.push('\n');
        }
    }
}
