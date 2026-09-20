//! `# tuxedo:off` / `# tuxedo:on`: statements between the two markers are
//! copied through as written.
//!
//! A marker is a comment on a line of its own whose text is exactly
//! `tuxedo:off` or `tuxedo:on`. It is honoured between the statements of a
//! body (or the file), which is where a hand-aligned table lives; a marker
//! anywhere else -- inside a hash literal, say -- would silently format
//! what it was meant to protect, so it is an error instead. So are an `on`
//! with nothing open and an `off` inside a region already off, reported
//! like a syntax error.
//!
//! A region with no `on` runs to the end of the body it opened in.

use ruby_prism::Visit;

use super::{CommentSpan, FormatError, Formatter, line_starts};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Marker {
    Off,
    On,
}

pub(super) fn marker_of(comment: &str) -> Option<Marker> {
    match comment.strip_prefix('#')?.trim() {
        "tuxedo:off" => Some(Marker::Off),
        "tuxedo:on" => Some(Marker::On),
        _ => None,
    }
}

/// Whether nothing but whitespace precedes `offset` on its line.
fn on_own_line(source: &str, starts: &[usize], offset: usize) -> bool {
    let line = match starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i - 1,
    };
    source[starts[line]..offset].trim().is_empty()
}

fn marker_error(starts: &[usize], offset: usize, message: &str) -> FormatError {
    let line = match starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i - 1,
    };
    FormatError::Syntax {
        message: message.to_string(),
        line: line + 1,
        column: offset - starts[line] + 1,
    }
}

/// An `off` at `offset` that no statement list opened a region at.
pub(super) fn stray_marker(source: &str, offset: usize) -> FormatError {
    marker_error(
        &line_starts(source),
        offset,
        "`# tuxedo:off` must sit between statements, with one after it in the same body",
    )
}

/// The markers in the file in order, or the first one that does not pair
/// up. Returns the offsets of the `off` markers, which `format` checks were
/// all honoured once the body has been built.
pub(super) fn check_markers(
    source: &str,
    comments: &[CommentSpan],
) -> Result<Vec<usize>, FormatError> {
    let starts = line_starts(source);
    let error = |offset: usize, message: &str| marker_error(&starts, offset, message);

    let mut open = false;
    let mut offs = Vec::new();
    for comment in comments {
        let Some(marker) = marker_of(&comment.text) else {
            continue;
        };
        if !on_own_line(source, &starts, comment.start) {
            continue;
        }
        match (marker, open) {
            (Marker::Off, false) => {
                open = true;
                offs.push(comment.start);
            }
            (Marker::Off, true) => {
                return Err(error(
                    comment.start,
                    "`# tuxedo:off` inside a region that is already off",
                ));
            }
            (Marker::On, true) => open = false,
            (Marker::On, false) => {
                return Err(error(
                    comment.start,
                    "`# tuxedo:on` without a `# tuxedo:off` before it",
                ));
            }
        }
    }
    Ok(offs)
}

/// The end of the last heredoc terminator inside `node`, which lies past
/// the node's own end: a heredoc's body follows the line it is opened on.
fn heredoc_end(node: &ruby_prism::Node<'_>) -> usize {
    struct Finder(usize);

    impl Finder {
        fn note(
            &mut self,
            opening: Option<ruby_prism::Location<'_>>,
            closing: Option<ruby_prism::Location<'_>>,
        ) {
            if let (Some(opening), Some(closing)) = (opening, closing)
                && opening.as_slice().starts_with(b"<<")
            {
                self.0 = self.0.max(closing.end_offset());
            }
        }
    }

    impl<'pr> Visit<'pr> for Finder {
        fn visit_string_node(&mut self, node: &ruby_prism::StringNode<'pr>) {
            self.note(node.opening_loc(), node.closing_loc());
            ruby_prism::visit_string_node(self, node);
        }

        fn visit_interpolated_string_node(
            &mut self,
            node: &ruby_prism::InterpolatedStringNode<'pr>,
        ) {
            self.note(node.opening_loc(), node.closing_loc());
            ruby_prism::visit_interpolated_string_node(self, node);
        }

        fn visit_x_string_node(&mut self, node: &ruby_prism::XStringNode<'pr>) {
            self.note(Some(node.opening_loc()), Some(node.closing_loc()));
            ruby_prism::visit_x_string_node(self, node);
        }

        fn visit_interpolated_x_string_node(
            &mut self,
            node: &ruby_prism::InterpolatedXStringNode<'pr>,
        ) {
            self.note(Some(node.opening_loc()), Some(node.closing_loc()));
            ruby_prism::visit_interpolated_x_string_node(self, node);
        }
    }

    let mut finder = Finder(node.location().end_offset());
    finder.visit(node);
    finder.0
}

impl Formatter {
    /// Whether the comment under the cursor opens a region: an own-line
    /// `# tuxedo:off`.
    pub(super) fn at_off_marker(&self) -> bool {
        self.comments.get(self.comment_cursor).is_some_and(|c| {
            marker_of(&c.text) == Some(Marker::Off)
                && on_own_line(&self.source, &self.line_starts, c.start)
        })
    }

    /// The region opened by the `off` under the cursor, as source text, and
    /// the offset it ends at. The cursor moves past every comment inside.
    ///
    /// It runs from the marker to the matching `on` (not included: that
    /// comment is written like any other), or to the end of `list`. What it
    /// covers is every statement of `list` that starts inside, the comments
    /// between them, and the rest of the line the last of them ends on --
    /// through a heredoc's terminator if one is opened there.
    pub(super) fn verbatim_region(
        &mut self,
        list: &ruby_prism::StatementsNode<'_>,
    ) -> (String, usize) {
        let start = self.comments[self.comment_cursor].start;
        let close = self.comments[self.comment_cursor + 1..]
            .iter()
            .find(|c| {
                marker_of(&c.text) == Some(Marker::On)
                    && on_own_line(&self.source, &self.line_starts, c.start)
            })
            .map_or(usize::MAX, |c| c.start);

        let mut end = start + self.comments[self.comment_cursor].text.len();
        for stmt in list.body().iter() {
            let at = stmt.location().start_offset();
            if at >= start && at < close {
                end = end.max(heredoc_end(&stmt));
            }
        }
        let comment_limit = close.min(list.location().end_offset());
        for comment in &self.comments[self.comment_cursor..] {
            if comment.start >= comment_limit {
                break;
            }
            end = end.max(comment.start + comment.text.len());
        }

        let bytes = self.source.as_bytes();
        if end > start && bytes[end - 1] == b'\n' {
            end -= 1;
        }
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }

        while self.comment_cursor < self.comments.len()
            && self.comments[self.comment_cursor].start < end
        {
            self.comment_cursor += 1;
        }
        self.honored_off.push(start);
        // Line endings are the one thing a region does not keep: the rest of
        // the file is written with `\n`, and a file with both is worse.
        (self.source[start..end].replace("\r\n", "\n"), end)
    }
}
