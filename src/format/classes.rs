use super::Formatter;
use ruby_prism::Visit;

impl Formatter {
    pub(super) fn fmt_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) {
        let class_line = self.line_of(node.location().start_offset());
        self.write_indent();
        self.output.push_str("class ");
        self.inline = true;
        self.visit(&node.constant_path());

        if let Some(superclass) = node.superclass() {
            self.output.push_str(" < ");
            self.visit(&superclass);
        }

        self.output.push('\n');
        self.emit_trailing_comment_if_same_line(class_line);

        self.indent += 1;
        if let Some(body) = node.body() {
            self.visit(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    pub(super) fn fmt_module_node(&mut self, node: &ruby_prism::ModuleNode<'_>) {
        self.write_indent();
        self.output.push_str("module ");
        self.inline = true;
        self.visit(&node.constant_path());
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.body() {
            self.visit(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    pub(super) fn fmt_constant_path_node(&mut self, node: &ruby_prism::ConstantPathNode<'_>) {
        if let Some(parent) = node.parent() {
            self.visit(&parent);
            self.output.push_str("::");
        }
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.unwrap().as_slice()).unwrap_or(""));
    }

    pub(super) fn fmt_constant_read_node(&mut self, node: &ruby_prism::ConstantReadNode<'_>) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }
}
