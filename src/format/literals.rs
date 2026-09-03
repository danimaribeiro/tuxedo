use super::Formatter;
use pretty::RcDoc as Doc;
use ruby_prism::Visit;

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
            let locations: Vec<_> = parts.iter().map(|part| part.location()).collect();
            self.write_leaf(|f| {
                for (i, location) in locations.iter().enumerate() {
                    if i > 0 {
                        f.output.push(' ');
                    }
                    f.write_bytes(location.as_slice());
                }
            });
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
