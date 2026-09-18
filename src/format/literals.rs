use super::Formatter;
use super::bytes_doc;
use super::doc::Doc;

impl Formatter {
    pub(super) fn fmt_parentheses_node(&mut self, node: &ruby_prism::ParenthesesNode<'_>) -> Doc {
        let inner = match node.body() {
            Some(body) => {
                let was_single_line = std::mem::replace(&mut self.single_line, true);
                let doc = self.node_doc(&body);
                self.single_line = was_single_line;
                doc
            }
            None => Doc::nil(),
        };
        Doc::text("(").append(inner).append(Doc::text(")"))
    }

    pub(super) fn fmt_true_node(&mut self, _node: &ruby_prism::TrueNode<'_>) -> Doc {
        Doc::text("true")
    }

    pub(super) fn fmt_false_node(&mut self, _node: &ruby_prism::FalseNode<'_>) -> Doc {
        Doc::text("false")
    }

    pub(super) fn fmt_nil_node(&mut self, _node: &ruby_prism::NilNode<'_>) -> Doc {
        Doc::text("nil")
    }

    pub(super) fn fmt_integer_node(&mut self, node: &ruby_prism::IntegerNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    pub(super) fn fmt_string_node(&mut self, node: &ruby_prism::StringNode<'_>) -> Doc {
        if let Some(opening) = node.opening_loc()
            && Self::is_heredoc_opening(opening.as_slice())
        {
            // A heredoc with no interpolation. Body runs from the start of
            // its content through the end of the terminator line.
            let closing = node.closing_loc();
            let body_start = node.content_loc().start_offset();
            let body_end = closing
                .as_ref()
                .map_or(node.content_loc().end_offset(), |c| c.end_offset());
            let body = self.heredoc_body(body_start, body_end);
            let opening = opening.as_slice().to_vec();
            return self.heredoc_doc(&opening, body, body_end);
        }

        if node.opening_loc().is_some() {
            // Reproduce the literal verbatim (delimiters included) rather
            // than reconstructing it from `unescaped()`: `unescaped()`
            // resolves escape sequences to their actual bytes, which
            // corrupts anything containing `\"` or `\n` — e.g. `"a\"b\nc"`
            // would come back out as `"a"b` followed by a real newline and
            // `c"`, which isn't even valid Ruby.
            bytes_doc(node.location().as_slice())
        } else {
            // A %w()/%i() word-list element has no quote delimiters of its
            // own in source (the location is just the bare word) — verbatim
            // copying would drop the quotes it needs as a standalone string.
            Doc::text("\"")
                .append(bytes_doc(node.unescaped()))
                .append(Doc::text("\""))
        }
    }

    pub(super) fn fmt_symbol_node(&mut self, node: &ruby_prism::SymbolNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    pub(super) fn fmt_float_node(&mut self, node: &ruby_prism::FloatNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    /// A rational literal (`2.5r`, `3r`). Unlike most numeric nodes,
    /// `RationalNode` carries no child node for the number it wraps --
    /// `location()` is the only way to get its text -- so there was no
    /// `fmt_*` for it at all, and prism's default Visit fell through
    /// silently: a call argument like `Time.new(2002, 10, 31, 2, 2,
    /// 2.123456789r, "+02:00")` came out with that argument dropped
    /// entirely (`Time.new(2002, 10, 31, 2, 2, , "+02:00")` -- a syntax
    /// error one comma later, but really a silent data-loss bug).
    pub(super) fn fmt_rational_node(&mut self, node: &ruby_prism::RationalNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    /// An imaginary/complex literal (`5i`, `2.5i`). Same missing-handler
    /// gap as `fmt_rational_node`, same fix.
    pub(super) fn fmt_imaginary_node(&mut self, node: &ruby_prism::ImaginaryNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    pub(super) fn fmt_source_file_node(&mut self, node: &ruby_prism::SourceFileNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    pub(super) fn fmt_source_line_node(&mut self, node: &ruby_prism::SourceLineNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    pub(super) fn fmt_regular_expression_node(
        &mut self,
        node: &ruby_prism::RegularExpressionNode<'_>,
    ) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    /// `$1`, `$2`, ... — a regexp capture-group reference. Copied verbatim.
    pub(super) fn fmt_numbered_reference_read_node(
        &mut self,
        node: &ruby_prism::NumberedReferenceReadNode<'_>,
    ) -> Doc {
        bytes_doc(node.as_node().location().as_slice())
    }

    /// `$&`, `$\``, `$'`, `$+` — a regexp back-reference. Copied verbatim.
    pub(super) fn fmt_back_reference_read_node(
        &mut self,
        node: &ruby_prism::BackReferenceReadNode<'_>,
    ) -> Doc {
        bytes_doc(node.as_node().location().as_slice())
    }

    /// A command literal (`` `stty size` ``, `%x{ls}`) with no
    /// interpolation. The body is shell text, so it is copied verbatim.
    pub(super) fn fmt_x_string_node(&mut self, node: &ruby_prism::XStringNode<'_>) -> Doc {
        bytes_doc(node.opening_loc().as_slice())
            .append(bytes_doc(node.content_loc().as_slice()))
            .append(bytes_doc(node.closing_loc().as_slice()))
    }

    /// A command literal containing `#{}` interpolation.
    pub(super) fn fmt_interpolated_x_string_node(
        &mut self,
        node: &ruby_prism::InterpolatedXStringNode<'_>,
    ) -> Doc {
        let opening = node.opening_loc().as_slice();
        let closing = node.closing_loc().as_slice();
        let parts = node.parts();
        self.interpolated_literal_doc(Some(opening), &parts, Some(closing))
    }

    pub(super) fn fmt_interpolated_string_node(
        &mut self,
        node: &ruby_prism::InterpolatedStringNode<'_>,
    ) -> Doc {
        let opening = node.opening_loc().map(|loc| loc.as_slice());
        let closing = node.closing_loc().map(|loc| loc.as_slice());
        let parts = node.parts();

        if let Some(opening_loc) = node.opening_loc()
            && Self::is_heredoc_opening(opening_loc.as_slice())
        {
            // A heredoc containing `#{}`. The body starts at the first
            // part; an empty heredoc has no parts, in which case the body
            // is just the terminator line.
            let closing_loc = node.closing_loc();
            let parts_list = node.parts();
            let body_start = parts_list
                .iter()
                .next()
                .map(|part| part.location().start_offset())
                .or_else(|| closing_loc.as_ref().map(|c| c.start_offset()));
            let body_end = closing_loc.as_ref().map_or(0, |c| c.end_offset());
            let body = match (body_start, closing_loc.as_ref()) {
                (Some(start), Some(closing)) => self.heredoc_body(start, closing.end_offset()),
                _ => String::new(),
            };
            let opening_bytes = opening_loc.as_slice().to_vec();
            return self.heredoc_doc(&opening_bytes, body, body_end);
        }

        // No delimiters of its own means this isn't one literal at all —
        // it's Ruby's implicit concatenation of adjacent literals
        // (`"foo" "bar"`, or the same split over lines with a trailing
        // `\`). Prism models both as an InterpolatedStringNode, but the
        // parts here are *complete* literals carrying their own quotes,
        // not the bare fragments that sit between `#{}` holes inside a
        // single pair of quotes. Emitting their contents the way
        // `interpolated_literal_doc` does would strip every quote and turn
        // `x = "foo" "bar"` into `x = foobar` — still valid Ruby, and a
        // completely different program. Copy each part verbatim instead.
        if opening.is_none() {
            return self.implicit_string_concat_doc(&parts);
        }

        self.interpolated_literal_doc(opening, &parts, closing)
    }

    pub(super) fn fmt_interpolated_symbol_node(
        &mut self,
        node: &ruby_prism::InterpolatedSymbolNode<'_>,
    ) -> Doc {
        let opening = node.opening_loc().map(|loc| loc.as_slice());
        let closing = node.closing_loc().map(|loc| loc.as_slice());
        let parts = node.parts();
        self.interpolated_literal_doc(opening, &parts, closing)
    }

    pub(super) fn fmt_interpolated_regular_expression_node(
        &mut self,
        node: &ruby_prism::InterpolatedRegularExpressionNode<'_>,
    ) -> Doc {
        let opening = node.opening_loc().as_slice();
        let closing = node.closing_loc().as_slice();
        let parts = node.parts();
        self.interpolated_literal_doc(Some(opening), &parts, Some(closing))
    }

    /// Adjacent string literals (`"foo" "bar"`). A newline between parts
    /// without a trailing `\` is a new statement, so overflow wraps with
    /// `\` and the continuation at one indent — a `group` whose separator
    /// is a `line` preceded by an `if_break` that contributes the `\` only
    /// in the broken shape. Parts that fit stay on one line with a single
    /// space. A run being held to one line (`single_line`) never
    /// introduces those breaks, and neither does a single part, which has
    /// no separator to break at.
    fn implicit_string_concat_doc(&mut self, parts: &ruby_prism::NodeList<'_>) -> Doc {
        let leaves: Vec<Doc> = parts
            .iter()
            .map(|part| bytes_doc(part.location().as_slice()))
            .collect();

        if self.single_line || leaves.len() <= 1 {
            return Doc::join(leaves, Doc::text(" "));
        }

        let separator = Doc::if_break(Doc::text(" \\"), Doc::nil()).append(Doc::line());
        Doc::join(leaves, separator)
            .indent(self.indent_str.len())
            .group()
    }

    /// An interpolated literal (`"a#{b}c"`, `:"a#{b}"`, `/a#{b}/`,
    /// `` `a#{b}` ``) as a `Doc`: the delimiters and each literal text
    /// segment as their own leaves, each `#{ }`/`#@ivar` hole as one
    /// opaque leaf. Literal segments are copied verbatim from source
    /// (preserving their original escaping); a hole is rendered inline,
    /// collapsing any internal statements onto one line the way a `{ }`
    /// block body does.
    ///
    /// The run is written into `output` in place and only lifted back out
    /// once the last part is done, rather than rendered into a detached
    /// buffer: a hole renders through the string emitter, which measures
    /// `current_column` to decide how much room it has, so everything
    /// ahead of it on the line — the opening delimiter and every earlier
    /// part — has to really be there while it renders. `bounds` records
    /// where each leaf ended so the run can be cut back apart afterwards.
    pub(super) fn interpolated_literal_doc(
        &mut self,
        opening: Option<&[u8]>,
        parts: &ruby_prism::NodeList<'_>,
        closing: Option<&[u8]>,
    ) -> Doc {
        let mut doc = match opening {
            Some(opening) => super::bytes_doc(opening),
            None => Doc::nil(),
        };
        for part in parts.iter() {
            if let Some(string) = part.as_string_node() {
                doc = doc.append(super::bytes_doc(string.content_loc().as_slice()));
            } else if let Some(embedded) = part.as_embedded_statements_node() {
                doc = doc.append(Doc::text("#{"));
                if let Some(statements) = embedded.statements() {
                    // Flat whatever it costs. A break inside a `#{}` hole
                    // is valid Ruby -- the hole is code, not string
                    // content -- but it lands in the middle of a literal
                    // whose own line structure is fixed, so the
                    // continuation sits at a column belonging to the
                    // string rather than to the expression.
                    let was_single_line = std::mem::replace(&mut self.single_line, true);
                    let inner = self.fmt_statements_node(&statements);
                    self.single_line = was_single_line;
                    doc = doc.append(inner.flat());
                }
                doc = doc.append(Doc::text("}"));
            } else {
                doc = doc.append(super::bytes_doc(part.location().as_slice()));
            }
        }
        match closing {
            Some(closing) => doc.append(super::bytes_doc(closing)),
            None => doc,
        }
    }
}
