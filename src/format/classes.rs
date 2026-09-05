use super::Formatter;
use ruby_prism::Visit;

impl Formatter {
    pub(super) fn fmt_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) {
        let inline = self.inline;
        let class_line = self.line_of(node.location().start_offset());
        if !inline {
            self.write_indent();
        }
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
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_module_node(&mut self, node: &ruby_prism::ModuleNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
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
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_singleton_class_node(&mut self, node: &ruby_prism::SingletonClassNode<'_>) {
        let inline = self.inline;
        let header_line = self.line_of(node.location().start_offset());
        if !inline {
            self.write_indent();
        }
        self.output.push_str("class << ");
        self.inline = true;
        self.visit(&node.expression());
        self.output.push('\n');
        self.emit_trailing_comment_if_same_line(header_line);

        self.indent += 1;
        if let Some(body) = node.body() {
            self.visit(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_constant_path_node(&mut self, node: &ruby_prism::ConstantPathNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }

        if let Some(parent) = node.parent() {
            // Without this the parent is written as a statement: as the
            // body of `def f; Thor::DynamicCommand; end` it took
            // fmt_constant_read_node's write_leaf path and came out as
            // `Thor` on its own line followed by `::DynamicCommand`, which
            // is still valid Ruby and a different constant lookup.
            self.inline = true;
            self.visit(&parent);
        }
        // `parent` is absent for a cbase path (`::Top`), where the leading
        // `::` is the whole prefix. Taking the delimiter from source rather
        // than hardcoding it after a parent keeps that case: `::Top` and
        // `Top` are different lookups.
        self.write_bytes(node.delimiter_loc().as_slice());
        self.write_bytes(node.name_loc().as_slice());

        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_constant_read_node(&mut self, node: &ruby_prism::ConstantReadNode<'_>) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }
}
