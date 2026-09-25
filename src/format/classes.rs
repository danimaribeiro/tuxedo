//! `class`, `module` and `class << self`, and the constant reads and
//! constant paths (`Foo::Bar`, `::Top`) that name them.
//!
//! Each is a header line, a body one indent in, and `end` back at the
//! header's level. An empty one is `header; end` on one line while that
//! fits (`class NotFound < Error; end`). A class body is the one statement list that
//! `access_modifier_style` applies to, which is why `class_body_doc` tells
//! the body it is one.

use super::Formatter;
use super::doc::Doc;
use crate::config::AccessModifierStyle;

impl Formatter {
    pub(super) fn fmt_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) -> Doc {
        let mut header = Doc::text("class ").append(self.node_doc(&node.constant_path()));
        if let Some(superclass) = node.superclass() {
            header = header
                .append(Doc::text(" < "))
                .append(self.node_doc(&superclass));
        }
        let header_line = self.line_of(node.location().start_offset());
        let empty = self.is_empty_definition(&node.as_node());
        self.class_body_doc(
            empty,
            header,
            header_line,
            node.body(),
            node.end_keyword_loc(),
        )
    }

    pub(super) fn fmt_module_node(&mut self, node: &ruby_prism::ModuleNode<'_>) -> Doc {
        let header = Doc::text("module ").append(self.node_doc(&node.constant_path()));
        let header_line = self.line_of(node.constant_path().location().end_offset());
        let empty = self.is_empty_definition(&node.as_node());
        self.class_body_doc(
            empty,
            header,
            header_line,
            node.body(),
            node.end_keyword_loc(),
        )
    }

    pub(super) fn fmt_singleton_class_node(
        &mut self,
        node: &ruby_prism::SingletonClassNode<'_>,
    ) -> Doc {
        let header = Doc::text("class << ").append(self.node_doc(&node.expression()));
        let header_line = self.line_of(node.location().start_offset());
        let empty = self.is_empty_definition(&node.as_node());
        self.class_body_doc(
            empty,
            header,
            header_line,
            node.body(),
            node.end_keyword_loc(),
        )
    }

    /// A class, module or `class <<` body: `definition_doc`, with the
    /// body's statement list told it is one, since that is the list whose
    /// access modifiers `access_modifier_style` places.
    fn class_body_doc(
        &mut self,
        empty: bool,
        header: Doc,
        header_line: usize,
        body: Option<ruby_prism::Node<'_>>,
        end: ruby_prism::Location<'_>,
    ) -> Doc {
        let was_body = self.class_body;
        if self.access_modifier_style == AccessModifierStyle::Outdent {
            self.class_body = body
                .as_ref()
                .and_then(|b| b.as_statements_node())
                .map(|s| s.location().start_offset());
        }
        let doc = self.definition_doc(empty, header, header_line, body, Some(end.start_offset()));
        self.class_body = was_body;
        doc
    }

    pub(super) fn fmt_constant_path_node(
        &mut self,
        node: &ruby_prism::ConstantPathNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        if let Some(parent) = node.parent() {
            // The parent goes into the same run as the name: it is part of
            // this path, not a statement of its own. `Thor` on one line and
            // `::DynamicCommand` on the next is still valid Ruby, and a
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
