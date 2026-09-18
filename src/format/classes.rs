use super::Formatter;
use super::doc::Doc;

impl Formatter {
    pub(super) fn fmt_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) -> Doc {
        let mut header = Doc::text("class ").append(self.node_doc(&node.constant_path()));
        if let Some(superclass) = node.superclass() {
            header = header
                .append(Doc::text(" < "))
                .append(self.node_doc(&superclass));
        }
        let header_line = self.line_of(node.location().start_offset());
        self.keyword_body_doc(
            header,
            header_line,
            node.body(),
            Some(node.end_keyword_loc().start_offset()),
        )
    }

    pub(super) fn fmt_module_node(&mut self, node: &ruby_prism::ModuleNode<'_>) -> Doc {
        let header = Doc::text("module ").append(self.node_doc(&node.constant_path()));
        let header_line = self.line_of(node.constant_path().location().end_offset());
        self.keyword_body_doc(
            header,
            header_line,
            node.body(),
            Some(node.end_keyword_loc().start_offset()),
        )
    }

    pub(super) fn fmt_singleton_class_node(
        &mut self,
        node: &ruby_prism::SingletonClassNode<'_>,
    ) -> Doc {
        let header = Doc::text("class << ").append(self.node_doc(&node.expression()));
        let header_line = self.line_of(node.location().start_offset());
        self.keyword_body_doc(
            header,
            header_line,
            node.body(),
            Some(node.end_keyword_loc().start_offset()),
        )
    }

    pub(super) fn fmt_constant_path_node(
        &mut self,
        node: &ruby_prism::ConstantPathNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        if let Some(parent) = node.parent() {
            // The parent is part of this path, not a statement of its
            // own: as the body of `def f; Thor::DynamicCommand; end` it
            // once came out as `Thor` on its own line followed by
            // `::DynamicCommand`, which is still valid Ruby and a
            // different constant lookup.
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
}
