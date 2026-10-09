//! Literals: numbers, strings, symbols, regular expressions, backticks,
//! heredoc openings, `true`/`false`/`nil`, `__FILE__`/`__LINE__`, `$1`/`$&`,
//! and parentheses.
//!
//! A literal's text is copied from source; the only rewrite is the quote
//! of a plain string or quoted symbol (`requote`, under `quote_style`),
//! and only when the value survives it. Interpolation holes (`#{}`) are
//! held to one line and never requoted. Adjacent string literals
//! (`"a" "b"`) always go one part per line, joined with `\`.

use super::Formatter;
use super::bytes_doc;
use super::doc::Doc;
use crate::config::QuoteStyle;

impl Formatter {
    pub(super) fn fmt_parentheses_node(&mut self, node: &ruby_prism::ParenthesesNode<'_>) -> Doc {
        // Held to one line, unless a comment sits inside: a `#` comment runs
        // to the end of its line, so there is no one-line shape for it. A
        // single expression holding a construct with no one-line spelling
        // -- a `begin`/`rescue`, an `if`/`else`, a `{ }` block that has to
        // expand -- is laid out like any other, since flattening it would
        // join its parts with semicolons at whatever width.
        let (start, end) = (node.location().start_offset(), node.location().end_offset());
        let expands = node.body().is_some_and(|body| {
            body.as_statements_node()
                .is_some_and(|s| s.body().iter().count() == 1)
                && (super::defs::holds_block_construct(&body)
                    || super::defs::holds_expanding_brace_block(&body))
        });
        let commented = !self.single_line && (expands || self.has_pending_comment_in(start, end));
        // Anything written after the last statement -- a trailing comment,
        // or comments on lines of their own -- ends a line `)` cannot share.
        let body_end = node.body().map_or(node.opening_loc().end_offset(), |b| {
            b.location().end_offset()
        });
        let closing_start = node.closing_loc().start_offset();
        let comment_before_close =
            commented && self.has_pending_comment_in(body_end, closing_start);
        let inner = match node.body() {
            Some(body) => {
                let was_single_line = std::mem::replace(&mut self.single_line, !commented);
                let doc = self.node_doc(&body);
                self.single_line = was_single_line;
                doc
            }
            None => Doc::nil(),
        };
        let bottom = comment_before_close
            .then(|| self.leading_comments_doc(closing_start, false))
            .flatten();
        let closing = if comment_before_close {
            bottom
                .unwrap_or_else(Doc::nil)
                .append(Doc::hardline())
                .append(Doc::text(")"))
        } else {
            Doc::text(")")
        };
        Doc::text("(").append(inner).append(closing)
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
            let opening_start = opening.start_offset();
            let opening = opening.as_slice().to_vec();
            return self.heredoc_doc(&opening, opening_start, body, body_end);
        }

        if node.opening_loc().is_some() {
            // Reproduce the literal verbatim (delimiters included) rather
            // than reconstructing it from `unescaped()`: `unescaped()`
            // resolves escape sequences to their actual bytes, which
            // corrupts anything containing `\"` or `\n` — e.g. `"a\"b\nc"`
            // would come back out as `"a"b` followed by a real newline and
            // `c"`, which isn't even valid Ruby.
            let requoted = node.closing_loc().and_then(|closing| {
                self.requote(
                    node.opening_loc()?.as_slice(),
                    node.content_loc().as_slice(),
                    closing.as_slice(),
                )
            });
            match requoted {
                Some(bytes) => bytes_doc(&bytes),
                None => bytes_doc(node.location().as_slice()),
            }
        } else {
            // A %w()/%i() word-list element has no quote delimiters of its
            // own in source (the location is just the bare word) — verbatim
            // copying would drop the quotes it needs as a standalone string.
            // Single quotes hold the word only if it has no `'` or `\`.
            let word = node.unescaped();
            let quote = if self.quote_style == QuoteStyle::Single
                && !word.iter().any(|b| matches!(b, b'\'' | b'\\'))
            {
                "'"
            } else {
                "\""
            };
            Doc::text(quote)
                .append(bytes_doc(word))
                .append(Doc::text(quote))
        }
    }

    pub(super) fn fmt_symbol_node(&mut self, node: &ruby_prism::SymbolNode<'_>) -> Doc {
        let requoted = node
            .opening_loc()
            .zip(node.closing_loc())
            .and_then(|(opening, closing)| {
                self.requote(
                    opening.as_slice(),
                    node.value_loc()?.as_slice(),
                    closing.as_slice(),
                )
            });
        match requoted {
            Some(bytes) => bytes_doc(&bytes),
            None => bytes_doc(node.location().as_slice()),
        }
    }

    /// The literal written with the configured quote, or `None` when it is
    /// already written that way, is not a plain `'`/`"` literal (`%q`,
    /// `?a`, a heredoc), or cannot change quotes without changing its
    /// value. Covers `'a'`, `:'a'` and the hash key `'a': 1`, whose closing
    /// is `':`.
    ///
    /// `'` -> `"` needs content with no `"`, no `\` and no `#{`, `#@` or
    /// `#$`, which double quotes would read as escapes or interpolation.
    /// `"` -> `'` needs no `'`, and only `\\` and `\"` as escapes, both of
    /// which single quotes spell differently.
    fn requote(&self, opening: &[u8], content: &[u8], closing: &[u8]) -> Option<Vec<u8>> {
        let to = match self.quote_style {
            QuoteStyle::Double => b'"',
            QuoteStyle::Single => b'\'',
            QuoteStyle::Preserve => return None,
        };
        let (lead, from) = match opening {
            [q @ (b'\'' | b'"')] => (&opening[..0], *q),
            [b':', q @ (b'\'' | b'"')] => (&opening[..1], *q),
            _ => return None,
        };
        let tail = match closing {
            [q, tail @ ..] if *q == from && matches!(tail, [] | [b':']) => tail,
            _ => return None,
        };
        if from == to {
            return None;
        }

        let mut body = Vec::with_capacity(content.len());
        if to == b'"' {
            for (i, &b) in content.iter().enumerate() {
                let interpolates =
                    b == b'#' && matches!(content.get(i + 1), Some(b'{' | b'@' | b'$'));
                if b == b'"' || b == b'\\' || interpolates {
                    return None;
                }
                body.push(b);
            }
        } else {
            let mut bytes = content.iter();
            while let Some(&b) = bytes.next() {
                match b {
                    b'\'' => return None,
                    b'\\' => match bytes.next() {
                        Some(b'\\') => body.extend_from_slice(b"\\\\"),
                        Some(b'"') => body.push(b'"'),
                        _ => return None,
                    },
                    _ => body.push(b),
                }
            }
        }

        let mut out = lead.to_vec();
        out.push(to);
        out.extend(body);
        out.push(to);
        out.extend_from_slice(tail);
        Some(out)
    }

    pub(super) fn fmt_float_node(&mut self, node: &ruby_prism::FloatNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    /// A rational literal (`2.5r`, `3r`). `RationalNode` carries the
    /// numerator and denominator as values, not the text written, so the
    /// source span is the only way to get `2.5r` back rather than `5/2r`.
    pub(super) fn fmt_rational_node(&mut self, node: &ruby_prism::RationalNode<'_>) -> Doc {
        bytes_doc(node.location().as_slice())
    }

    /// An imaginary literal (`5i`, `2.5i`), copied from source like a
    /// rational one.
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
        let opening = node.opening_loc();
        if Self::is_heredoc_opening(opening.as_slice()) {
            let closing = node.closing_loc();
            let body_start = node.content_loc().start_offset();
            let body_end = closing.end_offset();
            let body = self.heredoc_body(body_start, body_end);
            let opening_bytes = opening.as_slice().to_vec();
            return self.heredoc_doc(&opening_bytes, opening.start_offset(), body, body_end);
        }
        bytes_doc(node.opening_loc().as_slice())
            .append(bytes_doc(node.content_loc().as_slice()))
            .append(bytes_doc(node.closing_loc().as_slice()))
    }

    /// A command literal containing `#{}` interpolation.
    pub(super) fn fmt_interpolated_x_string_node(
        &mut self,
        node: &ruby_prism::InterpolatedXStringNode<'_>,
    ) -> Doc {
        let opening_loc = node.opening_loc();
        let parts = node.parts();
        if Self::is_heredoc_opening(opening_loc.as_slice()) {
            return self.interpolated_heredoc_doc(opening_loc, Some(node.closing_loc()), &parts);
        }
        let opening = opening_loc.as_slice();
        let closing = node.closing_loc();
        self.interpolated_literal_doc(Some(opening), &parts, Some(closing.as_slice()))
    }

    /// A heredoc containing `#{}`. The body starts at the first part; an
    /// empty heredoc has no parts, in which case the body is just the
    /// terminator line.
    fn interpolated_heredoc_doc(
        &mut self,
        opening_loc: ruby_prism::Location<'_>,
        closing_loc: Option<ruby_prism::Location<'_>>,
        parts: &ruby_prism::NodeList<'_>,
    ) -> Doc {
        let body_start = parts
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
        self.heredoc_doc(&opening_bytes, opening_loc.start_offset(), body, body_end)
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
            return self.interpolated_heredoc_doc(opening_loc, node.closing_loc(), &parts);
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

    /// An interpolated regex used directly as a condition (`if /#{x}/`),
    /// which matches against `$_`. Prism gives it its own node type; its
    /// holes are formatted like any other regex's.
    pub(super) fn fmt_interpolated_match_last_line_node(
        &mut self,
        node: &ruby_prism::InterpolatedMatchLastLineNode<'_>,
    ) -> Doc {
        let opening = node.opening_loc().as_slice();
        let closing = node.closing_loc().as_slice();
        let parts = node.parts();
        self.interpolated_literal_doc(Some(opening), &parts, Some(closing))
    }

    /// Adjacent string literals (`"foo" "bar"`), one part per line with a
    /// trailing `\` and the continuation at one indent, however short the
    /// parts are. On one line the construct reads as a missing comma, so
    /// the break is the author's statement that it is deliberate. A newline
    /// between parts without the `\` is a new statement, hence the
    /// backslash. A run being held to one line (`single_line`) has no line
    /// to break onto and keeps the parts joined by a space, and a single
    /// part has no separator at all.
    fn implicit_string_concat_doc(&mut self, parts: &ruby_prism::NodeList<'_>) -> Doc {
        let requoted = self.requote_concat(parts);
        let leaves: Vec<Doc> = parts
            .iter()
            .enumerate()
            .map(|(i, part)| match part.as_interpolated_string_node() {
                Some(_) => self.node_doc(&part),
                None => match &requoted[i] {
                    Some(bytes) => bytes_doc(bytes),
                    None => bytes_doc(part.location().as_slice()),
                },
            })
            .collect();

        if self.single_line || leaves.len() <= 1 {
            return Doc::join(leaves, Doc::text(" "));
        }

        let separator = Doc::text(" \\").append(Doc::hardline());
        Doc::join(leaves, separator).indent(self.indent_str.len())
    }

    /// Each part of an implicit concatenation written with the configured
    /// quote, or all `None` unless every part would then carry the same
    /// quote: `"a" 'b'` is left alone when `'b'` cannot change, rather than
    /// changing `'a'` to match a neighbour that stays as it is.
    fn requote_concat(&self, parts: &ruby_prism::NodeList<'_>) -> Vec<Option<Vec<u8>>> {
        let none = || parts.iter().map(|_| None).collect();
        let mut result = Vec::new();
        let mut quotes = Vec::new();
        for part in parts.iter() {
            let (bytes, quote) = if let Some(string) = part.as_string_node() {
                let (Some(opening), Some(closing)) = (string.opening_loc(), string.closing_loc())
                else {
                    return none();
                };
                let requoted = self.requote(
                    opening.as_slice(),
                    string.content_loc().as_slice(),
                    closing.as_slice(),
                );
                let quote = requoted.as_ref().map_or(opening.as_slice()[0], |b| b[0]);
                (requoted, quote)
            } else if let Some(interpolated) = part.as_interpolated_string_node() {
                match interpolated.opening_loc() {
                    Some(opening) => (None, opening.as_slice()[0]),
                    None => return none(),
                }
            } else {
                return none();
            };
            result.push(bytes);
            quotes.push(quote);
        }
        if quotes.windows(2).all(|pair| pair[0] == pair[1]) {
            result
        } else {
            none()
        }
    }

    /// An interpolated literal (`"a#{b}c"`, `:"a#{b}"`, `/a#{b}/`,
    /// `` `a#{b}` ``) as a `Doc`: the delimiters and each literal text
    /// segment as their own leaves, each `#{ }`/`#@ivar` hole as one
    /// opaque leaf. Literal segments are copied verbatim from source
    /// (preserving their original escaping); a hole is rendered inline,
    /// collapsing any internal statements onto one line the way a `{ }`
    /// block body does.
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
                    //
                    // The exception is a hole the author already wrote
                    // across lines around a construct with no one-line
                    // spelling (`#{reqs.map do |r| ... end.join}` with two
                    // statements in the block, an `if`/`else`): flat, it
                    // comes out joined with semicolons. It keeps its lines
                    // instead, laid out like the code it is.
                    //
                    // Literals inside the hole keep their quotes: a `"` in
                    // one would nest a double-quoted string in a
                    // double-quoted one, legal but harder to read than the
                    // `'` the author used.
                    let loc = statements.location();
                    let keeps_lines = self.line_of(loc.start_offset())
                        != self.line_of(loc.end_offset().saturating_sub(1))
                        && super::defs::holds_block_construct(&statements.as_node());
                    let was_single_line = std::mem::replace(&mut self.single_line, !keeps_lines);
                    let was_quote_style =
                        std::mem::replace(&mut self.quote_style, QuoteStyle::Preserve);
                    let inner = self.fmt_statements_node(&statements);
                    self.quote_style = was_quote_style;
                    self.single_line = was_single_line;
                    doc = doc.append(if keeps_lines { inner } else { inner.flat() });
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
