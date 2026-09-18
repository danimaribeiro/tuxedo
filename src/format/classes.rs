use super::Formatter;
use super::doc::Doc;

impl Formatter {
    fn write_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) {
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
        self.flush_comments_before(node.end_keyword_loc().start_offset(), node.body().is_some());
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    fn write_module_node(&mut self, node: &ruby_prism::ModuleNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("module ");
        self.inline = true;
        self.visit(&node.constant_path());
        self.output.push('\n');
        let header_line = self.line_of(node.constant_path().location().end_offset());
        self.emit_trailing_comment_if_same_line(header_line);

        self.indent += 1;
        if let Some(body) = node.body() {
            self.visit(&body);
        }
        self.flush_comments_before(node.end_keyword_loc().start_offset(), node.body().is_some());
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    fn write_singleton_class_node(&mut self, node: &ruby_prism::SingletonClassNode<'_>) {
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
        self.flush_comments_before(node.end_keyword_loc().start_offset(), node.body().is_some());
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end");
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_constant_path_node(
        &mut self,
        node: &ruby_prism::ConstantPathNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        if let Some(parent) = node.parent() {
            // Without this the parent is written as a statement: as the
            // body of `def f; Thor::DynamicCommand; end` it took
            // fmt_constant_read_node's write_leaf path and came out as
            // `Thor` on its own line followed by `::DynamicCommand`, which
            // is still valid Ruby and a different constant lookup.
            self.run_node(&mut run, &parent);
        }
        // `parent` is absent for a cbase path (`::Top`), where the leading
        // `::` is the whole prefix. Taking the delimiter from source rather
        // than hardcoding it after a parent keeps that case: `::Top` and
        // `Top` are different lookups.
        self.run_bytes(&mut run, node.delimiter_loc().as_slice());
        self.run_bytes(&mut run, node.name_loc().as_slice());
        self.end_run(run)
    }

    pub(super) fn fmt_constant_read_node(
        &mut self,
        node: &ruby_prism::ConstantReadNode<'_>,
    ) -> Doc {
        super::bytes_doc(node.name().as_slice())
    }

    // Each of these writes its own leading indent and lays its body out
    // with `self.indent` and real newlines, so it is `self_indented` in the
    // dispatch and rendered where it will sit -- see the note on
    // `fmt_def_node`.
    pub(super) fn fmt_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_class_node(node);
        })
    }

    pub(super) fn fmt_module_node(&mut self, node: &ruby_prism::ModuleNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_module_node(node);
        })
    }

    pub(super) fn fmt_singleton_class_node(
        &mut self,
        node: &ruby_prism::SingletonClassNode<'_>,
    ) -> Doc {
        self.spliced_doc(|f| {
            f.write_singleton_class_node(node);
        })
    }
}
