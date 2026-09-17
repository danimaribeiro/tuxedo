use super::Formatter;
use super::doc::Doc;

impl Formatter {
    pub(super) fn fmt_parentheses_node(&mut self, node: &ruby_prism::ParenthesesNode<'_>) {
        self.write_leaf(|f| {
            let inner = match node.body() {
                Some(body) => f.capture_as_doc(|f| {
                    let was_single_line = f.single_line;
                    f.single_line = true;
                    f.inline = true;
                    f.visit(&body);
                    f.single_line = was_single_line;
                }),
                None => Doc::nil(),
            };
            let doc = Doc::text("(").append(inner).append(Doc::text(")"));
            let rendered = f.render_doc_to_string(doc);
            f.output.push_str(&rendered);
        });
    }

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
            self.write_heredoc(&opening, body, body_end);
            return;
        }

        if node.opening_loc().is_some() {
            // Reproduce the literal verbatim (delimiters included) rather
            // than reconstructing it from `unescaped()`: `unescaped()`
            // resolves escape sequences to their actual bytes, which
            // corrupts anything containing `\"` or `\n` — e.g. `"a\"b\nc"`
            // would come back out as `"a"b` followed by a real newline and
            // `c"`, which isn't even valid Ruby.
            let bytes = node.location().as_slice();
            self.write_leaf(|f| f.write_bytes(bytes));
        } else {
            // A %w()/%i() word-list element has no quote delimiters of its
            // own in source (the location is just the bare word) — verbatim
            // copying would drop the quotes it needs as a standalone string.
            let unescaped = node.unescaped();
            self.write_leaf(|f| {
                f.output.push('"');
                f.write_bytes(unescaped);
                f.output.push('"');
            });
        }
    }

    pub(super) fn fmt_symbol_node(&mut self, node: &ruby_prism::SymbolNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_float_node(&mut self, node: &ruby_prism::FloatNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    /// A rational literal (`2.5r`, `3r`). Unlike most numeric nodes,
    /// `RationalNode` carries no child node for the number it wraps --
    /// `location()` is the only way to get its text -- so there was no
    /// `fmt_*` for it at all, and prism's default Visit fell through
    /// silently: a call argument like `Time.new(2002, 10, 31, 2, 2,
    /// 2.123456789r, "+02:00")` came out with that argument dropped
    /// entirely (`Time.new(2002, 10, 31, 2, 2, , "+02:00")` -- a syntax
    /// error one comma later, but really a silent data-loss bug).
    pub(super) fn fmt_rational_node(&mut self, node: &ruby_prism::RationalNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    /// An imaginary/complex literal (`5i`, `2.5i`). Same missing-handler
    /// gap as `fmt_rational_node`, same fix.
    pub(super) fn fmt_imaginary_node(&mut self, node: &ruby_prism::ImaginaryNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_source_file_node(&mut self, node: &ruby_prism::SourceFileNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_source_line_node(&mut self, node: &ruby_prism::SourceLineNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    pub(super) fn fmt_regular_expression_node(
        &mut self,
        node: &ruby_prism::RegularExpressionNode<'_>,
    ) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    /// `$1`, `$2`, ... — a regexp capture-group reference. Copied verbatim.
    pub(super) fn fmt_numbered_reference_read_node(
        &mut self,
        node: &ruby_prism::NumberedReferenceReadNode<'_>,
    ) {
        let bytes = node.as_node().location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    /// `$&`, `$\``, `$'`, `$+` — a regexp back-reference. Copied verbatim.
    pub(super) fn fmt_back_reference_read_node(
        &mut self,
        node: &ruby_prism::BackReferenceReadNode<'_>,
    ) {
        let bytes = node.as_node().location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    /// A command literal (`` `stty size` ``, `%x{ls}`) with no
    /// interpolation. The body is shell text, so it is copied verbatim.
    pub(super) fn fmt_x_string_node(&mut self, node: &ruby_prism::XStringNode<'_>) {
        let opening = node.opening_loc().as_slice();
        let content = node.content_loc().as_slice();
        let closing = node.closing_loc().as_slice();
        self.write_leaf(|f| {
            f.write_bytes(opening);
            f.write_bytes(content);
            f.write_bytes(closing);
        });
    }

    /// A command literal containing `#{}` interpolation.
    pub(super) fn fmt_interpolated_x_string_node(
        &mut self,
        node: &ruby_prism::InterpolatedXStringNode<'_>,
    ) {
        let opening = node.opening_loc().as_slice();
        let closing = node.closing_loc().as_slice();
        let parts = node.parts();
        self.write_leaf(|f| {
            f.write_bytes(opening);
            f.fmt_interpolated_parts(&parts);
            f.write_bytes(closing);
        });
    }

    pub(super) fn fmt_interpolated_string_node(
        &mut self,
        node: &ruby_prism::InterpolatedStringNode<'_>,
    ) {
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
            self.write_heredoc(&opening_bytes, body, body_end);
            return;
        }

        // No delimiters of its own means this isn't one literal at all —
        // it's Ruby's implicit concatenation of adjacent literals
        // (`"foo" "bar"`, or the same split over lines with a trailing
        // `\`). Prism models both as an InterpolatedStringNode, but the
        // parts here are *complete* literals carrying their own quotes,
        // not the bare fragments that sit between `#{}` holes inside a
        // single pair of quotes. Emitting their contents the way
        // `fmt_interpolated_parts` does would strip every quote and turn
        // `x = "foo" "bar"` into `x = foobar` — still valid Ruby, and a
        // completely different program. Copy each part verbatim instead.
        if opening.is_none() {
            self.write_implicit_string_concat(&parts);
            return;
        }

        self.write_leaf(|f| {
            if let Some(opening) = opening {
                f.write_bytes(opening);
            }
            f.fmt_interpolated_parts(&parts);
            if let Some(closing) = closing {
                f.write_bytes(closing);
            }
        });
    }

    pub(super) fn fmt_interpolated_symbol_node(
        &mut self,
        node: &ruby_prism::InterpolatedSymbolNode<'_>,
    ) {
        let opening = node.opening_loc().map(|loc| loc.as_slice());
        let closing = node.closing_loc().map(|loc| loc.as_slice());
        let parts = node.parts();
        self.write_leaf(|f| {
            if let Some(opening) = opening {
                f.write_bytes(opening);
            }
            f.fmt_interpolated_parts(&parts);
            if let Some(closing) = closing {
                f.write_bytes(closing);
            }
        });
    }

    pub(super) fn fmt_interpolated_regular_expression_node(
        &mut self,
        node: &ruby_prism::InterpolatedRegularExpressionNode<'_>,
    ) {
        let opening = node.opening_loc().as_slice();
        let closing = node.closing_loc().as_slice();
        let parts = node.parts();
        self.write_leaf(|f| {
            f.write_bytes(opening);
            f.fmt_interpolated_parts(&parts);
            f.write_bytes(closing);
        });
    }

    /// Adjacent string literals (`"foo" "bar"`). A newline between parts
    /// without a trailing `\` is a new statement, so overflow wraps with
    /// `\` and the continuation at one indent. Parts that fit stay on one
    /// line with a single space. A `{ }` body that's still being forced
    /// flat (`single_line`) never introduces those breaks.
    fn write_implicit_string_concat(&mut self, parts: &ruby_prism::NodeList<'_>) {
        let slices: Vec<Vec<u8>> = parts
            .iter()
            .map(|part| part.location().as_slice().to_vec())
            .collect();
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }

        let join_flat = self.single_line || self.capture_depth > 0 || slices.len() <= 1 || {
            let parts_width: usize = slices
                .iter()
                .map(|s| unicode_width::UnicodeWidthStr::width(String::from_utf8_lossy(s).as_ref()))
                .sum();
            let seps = slices.len().saturating_sub(1);
            let flat_width = parts_width + seps;
            self.current_column()
                .saturating_add(flat_width)
                .saturating_add(self.trailing_reserve)
                <= self.line_length
        };

        if join_flat {
            for (i, slice) in slices.iter().enumerate() {
                if i > 0 {
                    self.output.push(' ');
                }
                self.write_bytes(slice);
            }
        } else {
            for (i, slice) in slices.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(" \\");
                    self.output.push('\n');
                    self.indent += 1;
                    self.write_indent();
                    self.indent -= 1;
                }
                self.write_bytes(slice);
            }
        }

        if !inline {
            self.output.push('\n');
        }
    }

    /// Renders the parts of an interpolated string/symbol/regexp literal:
    /// literal text segments are copied verbatim from source (preserving
    /// their original escaping) and `#{ }`/`#@ivar` segments are rendered
    /// inline, collapsing any internal statements onto one line the way a
    /// `{ }` block body does.
    pub(super) fn fmt_interpolated_parts(&mut self, parts: &ruby_prism::NodeList<'_>) {
        for part in parts.iter() {
            if let Some(string) = part.as_string_node() {
                self.write_bytes(string.content_loc().as_slice());
            } else if let Some(embedded) = part.as_embedded_statements_node() {
                self.output.push_str("#{");
                if let Some(statements) = embedded.statements() {
                    let was_single_line = self.single_line;
                    self.single_line = true;
                    self.inline = true;
                    self.fmt_statements_node(&statements);
                    self.single_line = was_single_line;
                }
                self.output.push('}');
            } else {
                self.write_bytes(part.location().as_slice());
            }
        }
    }
}
