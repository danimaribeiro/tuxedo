use super::Formatter;
use super::bytes_doc;
use super::doc::Doc;

impl Formatter {
    pub(super) fn fmt_local_variable_read_node(
        &mut self,
        node: &ruby_prism::LocalVariableReadNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_global_variable_read_node(
        &mut self,
        node: &ruby_prism::GlobalVariableReadNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_instance_variable_read_node(
        &mut self,
        node: &ruby_prism::InstanceVariableReadNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_class_variable_read_node(
        &mut self,
        node: &ruby_prism::ClassVariableReadNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_self_node(&mut self, _node: &ruby_prism::SelfNode<'_>) -> Doc {
        Doc::text("self")
    }
}
