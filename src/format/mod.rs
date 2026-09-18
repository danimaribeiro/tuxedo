use crate::config::FormatConfig;
use doc::Doc;
mod assignment;
mod calls;
mod classes;
mod collections;
mod control_flow;
mod defs;
mod dispatch;
mod doc;
mod literals;
mod patterns;
mod statements;
mod variables;
mod verify;

/// True if `node` is a call with a real `{ }`/`do...end` block (as opposed to
/// a `&block` pass) -- of either shape. Used where a block of any kind, not
/// just a hard-breaking one, needs to be singled out (e.g. a `.method` chain
/// stops at a link that has one; an assignment's RHS with one keeps its own
/// fixed indent rather than aligning under the `=`).
fn has_real_block(node: &ruby_prism::Node<'_>) -> bool {
    node.as_call_node()
        .is_some_and(|call| matches!(call.block(), Some(ruby_prism::Node::BlockNode { .. })))
}

/// A block's or lambda's parameter list, if it has one written in the
/// source. Prism also fills the `parameters` slot for a block that only
/// *uses* implicit parameters (`it`, `_1`) -- a marker node with no text of
/// its own, which must not be rendered as a parameter list.
fn explicit_parameters(params: Option<ruby_prism::Node<'_>>) -> Option<ruby_prism::Node<'_>> {
    params.filter(|p| {
        !matches!(
            p,
            ruby_prism::Node::NumberedParametersNode { .. }
                | ruby_prism::Node::ItParametersNode { .. }
        )
    })
}

/// Prism groups `foo(a, k: 1, j: 2)`'s keyword pairs into one
/// `KeywordHashNode`. Flatten those pairs so a call's argument list wraps
/// one-argument-per-line the same way RuboCop counts arguments.
fn flatten_keyword_hash_args(items: Vec<ruby_prism::Node<'_>>) -> Vec<ruby_prism::Node<'_>> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if let Some(kw) = item.as_keyword_hash_node() {
            out.extend(kw.elements().iter());
        } else {
            out.push(item);
        }
    }
    out
}

/// Why a source could not be formatted. In both cases the caller must leave
/// the source exactly as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// The source is not valid Ruby. Prism recovers from errors and still
    /// hands back a tree, but that tree has holes in it, and formatting
    /// from it would write those holes back as if they were the program.
    Syntax {
        message: String,
        /// 1-based.
        line: usize,
        /// 1-based, in bytes.
        column: usize,
    },
    /// The formatted output does not parse to the same AST as the source:
    /// a bug in Tuxedo, caught before it could reach the file. See
    /// `verify`.
    AstMismatch { detail: String },
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Syntax {
                message,
                line,
                column,
            } => write!(f, "{line}:{column}: syntax error: {message}"),
            FormatError::AstMismatch { detail } => write!(
                f,
                "formatting would change what this code does, so it was left untouched \
                 (this is a bug in tuxedo, please report it)\n  {detail}"
            ),
        }
    }
}

impl std::error::Error for FormatError {}

pub fn format(source: &str) -> Result<String, FormatError> {
    format_with_config(source, &FormatConfig::default())
}

pub fn format_with_config(source: &str, config: &FormatConfig) -> Result<String, FormatError> {
    let result = format_unverified(source, config)?;
    if result != source {
        let before = verify::fingerprint(source);
        let after = verify::fingerprint(&result);
        match (before, after) {
            (Some(before), Some(after)) if before == after => {}
            (Some(before), Some(after)) => {
                return Err(FormatError::AstMismatch {
                    detail: verify::first_difference(&before, &after),
                });
            }
            _ => {
                return Err(FormatError::AstMismatch {
                    detail: "output is not valid Ruby".to_string(),
                });
            }
        }
    }
    Ok(result)
}

/// Formats without the AST-equivalence check. Only for measuring what the
/// check costs and what it catches; never for writing files.
pub fn format_unverified(source: &str, config: &FormatConfig) -> Result<String, FormatError> {
    let parsed = ruby_prism::parse(source.as_bytes());
    if let Some(error) = parsed.errors().next() {
        let offset = error.location().start_offset();
        let starts = line_starts(source);
        let line = match starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        return Err(FormatError::Syntax {
            message: error.message().to_string(),
            line: line + 1,
            column: offset - starts[line] + 1,
        });
    }
    let node = parsed.node();

    // A file that is nothing but comments has no statements to separate
    // them from, so the first one opens the output rather than following a
    // line break.
    let empty_program = node
        .as_program_node()
        .is_some_and(|program| program.statements().body().iter().next().is_none());
    let mut formatter = Formatter::new(source, &parsed, config);
    let mut doc = formatter.node_doc(&node);
    if let Some(trailing) = formatter.remaining_comments_doc() {
        if !empty_program {
            doc = doc.append(Doc::hardline());
        }
        doc = doc.append(trailing);
    }
    let rendered = formatter.splice_heredoc_bodies(doc.render(formatter.line_length));
    let trimmed = rendered.trim_end_matches('\n');
    let mut result = trimmed.to_string();
    if !result.is_empty() {
        result.push('\n');
    }
    // `__END__` and everything after it is the program's `DATA`, not code:
    // copied byte for byte, down to whether it ends in a newline.
    if let Some(data) = parsed.data_loc() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&String::from_utf8_lossy(data.as_slice()));
    } else if result.is_empty() {
        result.push('\n');
    }
    Ok(result)
}

struct CommentSpan {
    start: usize,
    text: String,
}

/// A `Doc` being assembled left to right, one piece of literal text or one
/// child node at a time.
///
/// Nothing more than a `Vec<Doc>` with the formatter's node lookup
/// attached, which is what most small constructs want: `*splat`,
/// `key => value`, `alias new old` are a handful of pieces in a fixed
/// order with no line point between them.
pub(super) struct InlineRun {
    leaves: Vec<Doc>,
}

/// Everything a speculative render moves that a later re-`visit` of the
/// same node would move again.
struct RenderSnapshot {
    comment_cursor: usize,
    pending_heredocs_len: usize,
    heredoc_end_line: Option<usize>,
    indent: usize,
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

struct Formatter {
    /// How deep the construct being built sits. Nothing writes this out --
    /// indentation is a [`Doc::indent`] around a body -- but a comment
    /// that belongs one level out from the body it was written inside has
    /// to know there is a level to step out of.
    indent: usize,
    single_line: bool,
    indent_str: String,
    line_length: usize,
    comments: Vec<CommentSpan>,
    comment_cursor: usize,
    line_starts: Vec<usize>,
    source: String,
    /// Verbatim heredoc bodies, indexed by the marker written into
    /// `output` after each opening tag. See `heredoc_doc` and
    /// `splice_heredoc_bodies`.
    pending_heredocs: Vec<String>,
    /// Source line of the last heredoc terminator emitted. Prism ends a
    /// heredoc node at its opening tag, not its terminator, so a statement
    /// containing one looks far shorter than it is — which made the
    /// blank-line preservation in `fmt_statements_node` see a multi-line
    /// gap where the source had none.
    heredoc_end_line: Option<usize>,
    /// An offset no comment may be claimed past while a construct's body
    /// is being built: the `}` of the `{ }` it sits in. A statement takes
    /// the comment at the end of its own line, and inside a one-line body
    /// that is the same line the closing brace and whatever follows it are
    /// on -- so `foo { bar } # why` had the body's only statement claim a
    /// comment written about the whole call.
    comment_limit: Option<usize>,
    /// Set while a block condition is being built, and taken by the first
    /// infix spine that comes of it -- the one whose line points are the
    /// condition's own. A spine nested inside a bracketed construct in the
    /// condition is not it: an argument list and a block body carry their
    /// own indentation and must not be shifted.
    condition_spine: bool,
    /// Where the `when`/`in` clause about to be rendered stops: the start
    /// of the next clause, the `else`, or the `end`. Set by the enclosing
    /// `case` right before it visits each clause, which has no other way
    /// to learn it; taken (and so cleared) by the clause.
    clause_closer: Option<usize>,
}

/// Delimiters of the placeholder that stands in for a heredoc body between
/// `heredoc_doc` and `splice_heredoc_bodies`. Unicode private-use
/// characters, which cannot appear in Ruby source that Prism accepted.
const HEREDOC_MARK_START: char = '\u{E000}';
const HEREDOC_MARK_END: char = '\u{E001}';

impl Formatter {
    fn new(source: &str, parsed: &ruby_prism::ParseResult<'_>, config: &FormatConfig) -> Self {
        let comments = parsed
            .comments()
            .map(|c| CommentSpan {
                start: c.location().start_offset(),
                text: String::from_utf8_lossy(c.text()).into_owned(),
            })
            .collect();

        Self {
            indent: 0,
            single_line: false,
            indent_str: " ".repeat(config.indent_size),
            line_length: config.line_length,
            comments,
            comment_cursor: 0,
            line_starts: line_starts(source),
            source: source.to_string(),
            pending_heredocs: Vec::new(),
            heredoc_end_line: None,
            comment_limit: None,
            condition_spine: false,
            clause_closer: None,
        }
    }

    fn line_of(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }

    /// Builds a bracketed, comma-or-other-separated group (array literal,
    /// hash literal, parenthesized call argument list, ...) as a single Doc
    /// group implementing Tuxedo's block-indentation spec: either the whole
    /// thing fits on one line, or the opening bracket sits alone, every
    /// element gets a fixed indent one level deeper than the current
    /// statement, and the closing bracket sits alone back at the statement's
    /// own indent -- never aligned under the opening bracket, so renaming
    /// the assignment target can't reindent the body.
    ///
    /// `open_sep`/`close_sep` are `Doc::line` (a space when the group stays
    /// flat -- hash braces: `{ a: 1 }`) or `Doc::softline` (nothing when
    /// flat -- arrays/call args: `[1, 2]`, `(1, 2)`), passed in so callers
    /// control the flat-mode spacing while sharing the same broken-mode
    /// layout.
    fn bracketed_group_doc(
        &self,
        open: &str,
        open_sep: Doc,
        body: Doc,
        close_sep: Doc,
        close: &str,
    ) -> Doc {
        let nest_width = self.indent_str.len();
        // The closing token must be indented *together with* close_sep, not
        // appended after the indent ends -- an indent only applies to a line
        // break whose subsequent content is still inside the same subtree.
        Doc::text(open)
            .append(open_sep.append(body).indent(nest_width))
            .append(close_sep.append(Doc::text(close)))
            .group()
    }

    /// Side effects a render takes that a subsequent re-`visit` would take
    /// again -- comments consumed, heredoc bodies queued, indent left
    /// shifted if a visitor failed to restore it. Taken before a render
    /// the caller may decide to throw away, restored when it does.
    fn render_snapshot(&self) -> RenderSnapshot {
        RenderSnapshot {
            comment_cursor: self.comment_cursor,
            pending_heredocs_len: self.pending_heredocs.len(),
            heredoc_end_line: self.heredoc_end_line,
            indent: self.indent,
        }
    }

    fn restore_render_snapshot(&mut self, snap: RenderSnapshot) {
        self.comment_cursor = snap.comment_cursor;
        self.pending_heredocs.truncate(snap.pending_heredocs_len);
        self.heredoc_end_line = snap.heredoc_end_line;
        self.indent = snap.indent;
    }

    /// Parenthesized/bracketed list: expand all-or-nothing if any item
    /// already hard-breaks *or* comes back from `node_doc` multi-line (a
    /// nested array/hash/call that wrapped). The second case has to be
    /// asked about rather than left to the printer, because the two are
    /// different documents: the expanded shape puts a comma and a hardline
    /// after every item, and a flat group that merely broke would put the
    /// separators wherever its own line points happen to be.
    ///
    /// `span` is the source range between the brackets (end of the opening
    /// token, start of the closing one). A comment anywhere in it forces
    /// the expanded shape too, with each comment kept next to its item --
    /// see `commented_bracketed_items_doc`.
    fn bracketed_items_doc(
        &mut self,
        open: &str,
        open_sep: Doc,
        items: &[ruby_prism::Node<'_>],
        close_sep: Doc,
        close: &str,
        span: Option<(usize, usize)>,
    ) -> Doc {
        if let Some((start, end)) = span
            && !self.single_line
            && self.has_pending_comment_in(start, end)
        {
            // A comment between the brackets has no flat form: `#` runs to
            // the end of its line, so the list has to expand and keep each
            // comment next to the item it was written against.
            return self.commented_bracketed_items_doc(open, items, close, (start, end));
        }
        let snap = self.render_snapshot();
        let leaves: Vec<Doc> = items.iter().map(|item| self.node_doc(item)).collect();
        if leaves.iter().any(Doc::breaks) {
            self.restore_render_snapshot(snap);
            return self.expanded_bracketed_items_doc(open, items, close);
        }
        let body = Doc::join(leaves, Doc::text(",").append(Doc::line()));
        self.bracketed_group_doc(open, open_sep, body, close_sep, close)
    }

    fn format_comment(text: &str) -> std::borrow::Cow<'_, str> {
        if text.starts_with('#')
            && text.len() > 1
            && !text[1..].starts_with(char::is_whitespace)
            && !text.starts_with("#!")
            && !text.starts_with("##")
        {
            std::borrow::Cow::Owned(format!("# {}", &text[1..]))
        } else {
            std::borrow::Cow::Borrowed(text)
        }
    }

    /// The pending comment that sits at the end of `line`, if there is
    /// one, as ` # text` ready to append to that line's leaf. The `Doc`
    /// counterpart of [`Formatter::emit_trailing_comment_if_same_line`],
    /// for callers that build a line rather than write one.
    ///
    /// No deferral is involved and none is needed: the caller appends this
    /// after everything else that belongs on the line, which is what
    /// `line_suffix` would otherwise buy. The one thing it does not buy is
    /// that the comment is measured along with the code.
    fn trailing_comment_doc(&mut self, line: usize) -> Option<Doc> {
        if self.comment_cursor < self.comments.len()
            && self.line_of(self.comments[self.comment_cursor].start) == line
            && self
                .comment_limit
                .is_none_or(|limit| self.comments[self.comment_cursor].start < limit)
        {
            let text = Self::format_comment(&self.comments[self.comment_cursor].text).into_owned();
            self.comment_cursor += 1;
            return Some(Doc::text(format!(" {text}")));
        }
        None
    }

    /// True if a comment that has not been emitted yet starts inside
    /// `start..end`. A construct that could otherwise render on one line
    /// (a `{ }` block, a bracketed list) must not when this holds: a `#`
    /// comment runs to the end of its line, so there is no flat form that
    /// keeps it next to the code it describes.
    fn has_pending_comment_in(&self, start: usize, end: usize) -> bool {
        self.comments[self.comment_cursor..]
            .iter()
            .take_while(|c| c.start < end)
            .any(|c| c.start >= start)
    }

    /// Emits, as own-line comments at the current indent, every pending
    /// comment that starts before `offset`. Called with the body's indent
    /// still in effect, right before a keyword that closes or splits a
    /// body (`end`, `else`, `when`, `rescue`, `}`, ...), so a comment
    /// sitting at the bottom of a body stays inside it:
    ///
    /// ```ruby
    /// def perform
    ///   # Override in subclasses
    /// end
    /// ```
    ///
    /// Without this the comment stays pending until the next statement
    /// list picks it up -- after the `end`, in a different scope.
    ///
    /// `after_statements` says whether the body had any statements: a
    /// blank line the author left above the comment is kept only then,
    /// since a blank line directly under the header is not
    /// (Layout/EmptyLinesAround*Body).
    fn comments_before_doc(&mut self, offset: usize, after_statements: bool) -> Option<Doc> {
        self.comments_doc(offset, after_statements, true)
    }

    /// `comments_before_doc` for a position that is not the bottom of a
    /// body -- above the first `when` of a `case`, above a list item -- so
    /// there is only one level a comment can sit at: the current one.
    fn leading_comments_doc(&mut self, offset: usize, after_previous: bool) -> Option<Doc> {
        self.comments_doc(offset, after_previous, false)
    }

    /// Every pending comment that starts before `offset`, as own-line
    /// leaves each preceded by the [`Doc::hardline`] that puts it on its
    /// own line. `None` when there are none, so the caller can leave the
    /// line it is on alone.
    ///
    /// The hardline goes *in front of* each comment rather than after it
    /// because the caller owns what comes next: a body ends, these
    /// comments follow it, and then the caller's own hardline puts `end`
    /// on its line.
    fn comments_doc(
        &mut self,
        offset: usize,
        after_statements: bool,
        bottom_of_body: bool,
    ) -> Option<Doc> {
        if self.single_line {
            return None;
        }
        // A comment the author aligned with the branch keyword that follows
        // (`# about the next branch` right above a `when`/`else`/`rescue`)
        // describes that branch, not the body above it, and stays at the
        // keyword's level. Only for keywords that open another branch:
        // nothing follows an `end` for a comment to be about.
        let closer_column = offset - self.line_starts[self.line_of(offset)];
        let rest = &self.source[offset.min(self.source.len())..];
        let closer_opens_branch = bottom_of_body
            && ["else", "elsif", "when", "in", "rescue", "ensure"]
                .iter()
                .any(|keyword| {
                    rest.strip_prefix(keyword).is_some_and(|after| {
                        !after.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                    })
                });

        let mut doc = Doc::nil();
        let mut any = false;
        while self.comment_cursor < self.comments.len()
            && self.comments[self.comment_cursor].start < offset
        {
            let start = self.comments[self.comment_cursor].start;
            let line = self.line_of(start);
            let column = start - self.line_starts[line];
            let at_keyword_level =
                closer_opens_branch && column <= closer_column && self.indent > 0;
            let blank =
                (after_statements || any) && self.source_line_is_blank(line.wrapping_sub(1));

            let text = Self::format_comment(&self.comments[self.comment_cursor].text).into_owned();
            let mut entry = if blank {
                Doc::blank_line()
            } else {
                Doc::hardline()
            }
            .append(Doc::text(text));
            // The break has to be dedented along with the comment: an
            // outdent moves the lines a document's own line points open,
            // and a `text` leaf has none.
            if at_keyword_level {
                entry = entry.dedent(self.indent_str.len());
            }
            doc = doc.append(entry);
            self.comment_cursor += 1;
            any = true;
        }
        any.then_some(doc)
    }

    fn source_line_is_blank(&self, line: usize) -> bool {
        let Some(&start) = self.line_starts.get(line) else {
            return false;
        };
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.source.len());
        self.source[start..end].trim().is_empty()
    }

    /// A header, the body one level in, and whatever comments sat at the
    /// bottom of that body -- everything an `if`/`when`/`rescue`/`def`
    /// shares except the keyword that closes it.
    ///
    /// `header_line` is the source line a trailing comment would have to
    /// sit on to belong to the header (`class Foo # :nodoc:`), and
    /// `closer` is where whatever ends the body starts, so a comment at
    /// the bottom is taken here rather than left to surface after the
    /// construct, in a different scope.
    ///
    /// Nothing is emitted for an empty body, not even the line break: a
    /// `def` with nothing in it is `def x` then `end`, with no blank line
    /// between them.
    pub(super) fn clause_body_doc(
        &mut self,
        header: Doc,
        header_line: usize,
        body: Option<ruby_prism::Node<'_>>,
        closer: Option<usize>,
    ) -> Doc {
        let nest = self.indent_str.len();
        let mut doc = header;
        if let Some(comment) = self.trailing_comment_doc(header_line) {
            doc = doc.append(comment);
        }

        let has_body = body.is_some();
        self.indent += 1;
        let mut inner = Doc::nil();
        if let Some(body) = body {
            inner = inner.append(Doc::hardline()).append(self.node_doc(&body));
        }
        if let Some(offset) = closer
            && let Some(comments) = self.comments_before_doc(offset, has_body)
        {
            inner = inner.append(comments);
        }
        self.indent -= 1;
        doc.append(inner.indent(nest))
    }

    /// [`Formatter::clause_body_doc`] closed off with `end` on its own
    /// line: the shape `class`/`module`/`def`/`do...end`/`for` share.
    pub(super) fn keyword_body_doc(
        &mut self,
        header: Doc,
        header_line: usize,
        body: Option<ruby_prism::Node<'_>>,
        end_offset: Option<usize>,
    ) -> Doc {
        self.clause_body_doc(header, header_line, body, end_offset)
            .append(Doc::hardline())
            .append(Doc::text("end"))
    }

    /// Whatever comments are left when the program has been built: a file
    /// that ends in comments, or one that is nothing but comments.
    fn remaining_comments_doc(&mut self) -> Option<Doc> {
        let mut doc = Doc::nil();
        let mut any = false;
        while self.comment_cursor < self.comments.len() {
            if any {
                doc = doc.append(Doc::hardline());
            }
            doc = doc.append(Doc::text(
                Self::format_comment(&self.comments[self.comment_cursor].text).into_owned(),
            ));
            self.comment_cursor += 1;
            any = true;
        }
        any.then_some(doc)
    }

    /// `name op value` as a `Doc` -- the shape every simple write node
    /// (`x = 1`, `@x ||= y`, `$x += 1`) reduces to.
    fn assignment_doc(&mut self, name: &[u8], op: &str, value: &ruby_prism::Node<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_bytes(&mut run, name);
        self.finish_assignment(run, op, value)
    }

    /// `assignment_doc` for a target that is a node rather than a name --
    /// a constant path (`Foo::Bar = 1`).
    fn assignment_to_doc(
        &mut self,
        target: &ruby_prism::Node<'_>,
        op: &str,
        value: &ruby_prism::Node<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, target);
        self.finish_assignment(run, op, value)
    }

    /// Closes an assignment whose target is already in `run`: ` op ` and
    /// then the value.
    ///
    /// The value renders at the statement's own indent -- never aligned
    /// under the `=`.
    ///
    /// RuboCop's stock `Layout/EndAlignment` is `keyword`: it wants an
    /// `if`/`case`/`begin` value's `end` under the column its opening
    /// keyword sits at. But that column is set by the length of the
    /// assignment target, so renaming `message` to `msg` reindents the
    /// whole value -- the bracket alignment this project rejects
    /// (AGENTS.md, property 2), reached through `=` instead of a
    /// bracket. `x << if ...` was already exempt from it; `=` was not.
    ///
    /// See `test-projects/thor/lib/thor/parser/option.rb`'s
    /// `type = case value ... when ... end`.
    fn finish_assignment(
        &mut self,
        mut run: InlineRun,
        op: &str,
        value: &ruby_prism::Node<'_>,
    ) -> Doc {
        self.run_text(&mut run, " ");
        self.run_text(&mut run, op);
        self.run_text(&mut run, " ");
        self.run_node(&mut run, value);
        self.end_run(run)
    }

    /// The must-break bracketed list: opening token alone, every item on
    /// its own line at indent+1, closing token alone at the current
    /// indent. Used when at least one item already hard-breaks, so a group
    /// can't choose "fits on one line" -- it already doesn't.
    ///
    /// The breaks are [`Doc::hardline`]s rather than raw newlines, which
    /// is what lets this shape sit inside an enclosing group and force it
    /// open on its own, with no need for the enclosing construct to
    /// predict the break before rendering.
    fn expanded_bracketed_items_doc(
        &mut self,
        open: &str,
        items: &[ruby_prism::Node<'_>],
        close: &str,
    ) -> Doc {
        let nest_width = self.indent_str.len();
        self.indent += 1;
        let body = self.expanded_items_doc(items);
        self.indent -= 1;
        Doc::text(open)
            .append(Doc::hardline().append(body).indent(nest_width))
            .append(Doc::hardline())
            .append(Doc::text(close))
    }

    /// The expanded shape of `expanded_bracketed_items_doc`, for a list
    /// with comments inside its brackets. Each comment stays where the
    /// author put it relative to the items:
    ///
    /// ```ruby
    /// foo(
    ///   # own-line: above the item that follows it
    ///   1, # trailing: after the item (and its comma) on the same line
    ///   2
    ///   # dangling: after the last item, above the closing bracket
    /// )
    /// ```
    fn commented_bracketed_items_doc(
        &mut self,
        open: &str,
        items: &[ruby_prism::Node<'_>],
        close: &str,
        (opening_end, closing_start): (usize, usize),
    ) -> Doc {
        let nest_width = self.indent_str.len();
        // `foo( # why`: nothing to attach it to but the list itself.
        let opening_comment = self.trailing_comment_doc(self.line_of(opening_end));

        self.indent += 1;
        let mut body = Doc::nil();
        let last = items.len().saturating_sub(1);
        for (i, item) in items.iter().enumerate() {
            if let Some(leading) = self.leading_comments_doc(item.location().start_offset(), i > 0)
            {
                body = body.append(leading);
            }
            body = body.append(Doc::hardline()).append(self.node_doc(item));
            if i != last {
                body = body.append(Doc::text(","));
            }

            // A trailing comment belongs to this item if it is on the
            // line the item ends on and before the next item starts
            // (`1, 2, # two` is about `2`).
            let item_end_line = self.line_of(item.location().end_offset().saturating_sub(1));
            let next_start = items
                .get(i + 1)
                .map_or(closing_start, |next| next.location().start_offset());
            let is_trailing = self
                .comments
                .get(self.comment_cursor)
                .is_some_and(|c| c.start < next_start && self.line_of(c.start) == item_end_line);
            if is_trailing && let Some(trailing) = self.trailing_comment_doc(item_end_line) {
                body = body.append(trailing);
            }
        }
        if let Some(dangling) = self.comments_before_doc(closing_start, !items.is_empty()) {
            body = body.append(dangling);
        }
        self.indent -= 1;

        Doc::text(open)
            .append(opening_comment.unwrap_or_else(Doc::nil))
            .append(body.indent(nest_width))
            .append(Doc::hardline())
            .append(Doc::text(close))
    }

    /// `items` one per line, comma-separated.
    fn expanded_items_doc(&mut self, items: &[ruby_prism::Node<'_>]) -> Doc {
        let leaves: Vec<Doc> = items.iter().map(|item| self.node_doc(item)).collect();
        Doc::join(leaves, Doc::text(",").append(Doc::hardline()))
    }

    /// The expanded shape of a bare command argument list: the first
    /// argument stays on the call line, every later one starts a line of
    /// its own at one indent. Ruby has no paren-less way to put the first
    /// argument on a new line, and Tuxedo does not rewrite command style
    /// into a parenthesized call.
    fn expanded_bare_args_doc(&mut self, items: &[ruby_prism::Node<'_>]) -> Doc {
        self.indent += 1;
        let leaves: Vec<Doc> = items.iter().map(|item| self.node_doc(item)).collect();
        self.indent -= 1;
        Doc::text(" ").append(Self::bare_args_body_doc(
            leaves,
            Doc::hardline(),
            self.indent_str.len(),
        ))
    }

    /// `a, b, c` for a bare command list, with `separator` after each
    /// comma: the first argument carries no line point in front of it (it
    /// continues the call line) and is left outside the `indent`, so a
    /// collection of its own that wraps keeps the statement's indent the
    /// way the string emitter gave it.
    fn bare_args_body_doc(leaves: Vec<Doc>, separator: Doc, nest_width: usize) -> Doc {
        let mut leaves = leaves.into_iter();
        let first = leaves.next().unwrap_or_else(Doc::nil);
        let rest =
            Doc::concat(leaves.map(|leaf| Doc::text(",").append(separator.clone()).append(leaf)));
        first.append(rest.indent(nest_width))
    }

    /// Renders a call's argument list, including a block-pass argument
    /// (`&:sym` / `&block`) if present. Prism stores a block-pass in the
    /// same `block()` slot as a real `{ }`/`do...end` block, but unlike a
    /// real block it belongs inside the argument list (comma-joined with
    /// any positional args), not after the closing paren.
    pub(super) fn call_args_doc(
        &mut self,
        node: &ruby_prism::CallNode<'_>,
        parenthesized: bool,
    ) -> Doc {
        let span = node
            .opening_loc()
            .zip(node.closing_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));
        let positional: Vec<_> = node
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let block_pass = match node.block() {
            Some(b @ ruby_prism::Node::BlockArgumentNode { .. }) => Some(b),
            _ => None,
        };
        self.arg_list_doc(parenthesized, positional, block_pass, span)
    }

    /// Shared argument-list builder for calls, `super`, and `yield`.
    ///
    /// Parenthesized lists are one all-or-nothing group, block indented
    /// like any other bracketed list (`bracketed_items_doc`). Keyword
    /// pairs are flattened out of Prism's `KeywordHashNode` first, so
    /// `foo(a, k: 1, j: 2)` wraps one pair per line rather than as one
    /// opaque leaf.
    ///
    /// Bare command calls stay unparenthesized and get their own group:
    /// either ` a, b, c` on the call line, or the first argument there and
    /// every later one on its own line at one indent. That covers
    /// Layout/MultilineMethodArgumentLineBreaks and Layout/LineLength for
    /// command style without adding parentheses (a single argument that
    /// itself exceeds the limit still cannot wrap).
    pub(super) fn arg_list_doc<'a>(
        &mut self,
        parenthesized: bool,
        mut items: Vec<ruby_prism::Node<'a>>,
        block_pass: Option<ruby_prism::Node<'a>>,
        span: Option<(usize, usize)>,
    ) -> Doc {
        if items.is_empty() && block_pass.is_none() && !parenthesized {
            return Doc::nil();
        }

        if let Some(block_pass) = block_pass {
            items.push(block_pass);
        }

        // Bare keyword args (`foo(a, k: 1, j: 2)`) are one KeywordHashNode
        // in Prism. Flatten each pair into a sibling of the positional
        // args so all-or-nothing wrapping applies to every argument the
        // way RuboCop's Layout/MultilineMethodArgumentLineBreaks counts
        // them -- not to the hash as a single opaque leaf (`k: 1, j: 2`
        // sharing a line on an already-broken call).
        let items = flatten_keyword_hash_args(items);

        if parenthesized && !items.is_empty() {
            return self.bracketed_items_doc(
                "(",
                Doc::softline(),
                &items,
                Doc::softline(),
                ")",
                span,
            );
        }

        if !parenthesized && items.len() > 1 && !self.single_line {
            return self.bare_args_doc(&items);
        }

        // A single bare argument, an empty `()`, or a list being forced
        // onto one line: no line point anywhere, so it is built left to
        // right and each argument measures the column it really starts at.
        let mut run = self.begin_run();
        self.run_text(&mut run, if parenthesized { "(" } else { " " });
        for (i, arg) in items.iter().enumerate() {
            if i > 0 {
                self.run_text(&mut run, ", ");
            }
            self.run_node(&mut run, arg);
        }
        if parenthesized {
            self.run_text(&mut run, ")");
        }
        self.end_run(run)
    }

    /// A bare command argument list as one all-or-nothing group. An
    /// argument that is already multi-line (a `do...end` block call, a
    /// collection that has to expand) leaves no flat form to choose, so
    /// the expanded shape is built directly -- the same two-step
    /// `bracketed_items_doc` takes, and for the same reason: such an
    /// argument is still an opaque text leaf that the printer would
    /// measure as one long line.
    fn bare_args_doc(&mut self, items: &[ruby_prism::Node<'_>]) -> Doc {
        let snap = self.render_snapshot();
        let leaves: Vec<Doc> = items.iter().map(|item| self.node_doc(item)).collect();
        if leaves.iter().any(Doc::breaks) {
            self.restore_render_snapshot(snap);
            return self.expanded_bare_args_doc(items);
        }
        Doc::text(" ")
            .append(Self::bare_args_body_doc(
                leaves,
                Doc::line(),
                self.indent_str.len(),
            ))
            .group()
    }

    /// Writes a leaf/literal node (a symbol, a literal, a variable read, ...)
    /// that carries no indentation or newline of its own. Most such nodes
    /// only ever appear as a sub-expression (inline), but they can also be a
    /// whole statement on their own (e.g. a bare `true` as a method body),
    /// in which case they need to behave like any other statement.
    /// True if a string-ish literal's opening delimiter marks a heredoc
    /// (`<<~SQL`, `<<-EOS`, `<<EOS`, `<<'EOS'`) rather than a normal quote.
    fn is_heredoc_opening(opening: &[u8]) -> bool {
        opening.starts_with(b"<<")
    }

    /// The `Doc` for a heredoc: the opening tag is all the node renders
    /// where it sits, and the body is deferred.
    ///
    /// A heredoc is the one Ruby construct whose text does not live where
    /// its node sits. `foo(<<~SQL, x)` means "emit `<<~SQL`, keep
    /// formatting the rest of *this* line, then dump the body on the lines
    /// that follow". The emitter is otherwise strictly linear, so writing
    /// the body at the point the node is visited is what produced
    /// `z = <<-FOO  hi` — the body pulled onto the opening line.
    ///
    /// Rather than teach all 39 newline sites to flush a queue, we record
    /// where in `output` the tag landed and splice the bodies in once, at
    /// the end, in `splice_heredoc_bodies`.
    ///
    /// `body` is copied verbatim, byte for byte. A heredoc body is opaque
    /// text, not code: its leading whitespace is data (re-indenting inside
    /// a `<<-SQL` changes the SQL), so it is never reformatted, and never
    /// re-indented — not even for `<<~`, where Ruby itself strips the
    /// margin at runtime.
    fn heredoc_doc(&mut self, opening: &[u8], body: String, body_end: usize) -> Doc {
        let terminator_line = self.line_of(body_end.saturating_sub(1));
        self.heredoc_end_line = Some(
            self.heredoc_end_line
                .map_or(terminator_line, |seen| seen.max(terminator_line)),
        );
        let index = self.pending_heredocs.len();
        self.pending_heredocs.push(body);
        // A marker rather than an offset into `output`: a speculative
        // render can be thrown away and the text written again somewhere
        // else, so an offset taken while it is built points at the wrong
        // place afterwards. A marker travels with the text wherever it
        // ends up.
        bytes_doc(opening).append(Doc::text(format!(
            "{HEREDOC_MARK_START}{index}{HEREDOC_MARK_END}"
        )))
    }

    /// Splices every deferred heredoc body into `output`, each one just
    /// after the newline ending the line its opening tag landed on.
    ///
    /// Walks in reverse offset order so that earlier splices cannot shift
    /// the offsets of the ones still to be placed. Two heredocs opened on
    /// the same line (`foo(<<~A, <<~B)`) therefore end up in source order:
    /// B's body is inserted at the line's end first, then A's is inserted
    /// at that same index, pushing B's after it.
    fn splice_heredoc_bodies(&mut self, rendered: String) -> String {
        if self.pending_heredocs.is_empty() {
            return rendered;
        }
        let bodies = std::mem::take(&mut self.pending_heredocs);

        let mut out = String::with_capacity(rendered.len());
        let mut queued: Vec<usize> = Vec::new();
        let mut chars = rendered.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                HEREDOC_MARK_START => {
                    let mut digits = String::new();
                    for d in chars.by_ref() {
                        if d == HEREDOC_MARK_END {
                            break;
                        }
                        digits.push(d);
                    }
                    if let Ok(index) = digits.parse::<usize>() {
                        queued.push(index);
                    }
                }
                '\n' => {
                    out.push('\n');
                    // Bodies land after the line their tag closed, in the
                    // order the tags appeared on it: `foo(<<~A, <<~B)`
                    // emits A's body then B's.
                    for index in queued.drain(..) {
                        out.push_str(&bodies[index]);
                    }
                }
                _ => out.push(c),
            }
        }
        if !queued.is_empty() {
            out.push('\n');
            for index in queued.drain(..) {
                out.push_str(&bodies[index]);
            }
        }
        out
    }

    /// The verbatim source of a heredoc body, from `body_start` through the
    /// end of its terminator line.
    ///
    /// A body always begins at the start of a line, but its first *node*
    /// may not: in `<<~MSG` the leading whitespace of the first line is not
    /// part of the first part's location (and the first part may be a
    /// `#{}` hole). Dropping it leaves that line at column 0, which
    /// changes how much margin Ruby strips from every other line.
    fn heredoc_body(&self, body_start: usize, body_end: usize) -> String {
        let body_start = self.source.as_bytes()[..body_start.min(self.source.len())]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |newline| newline + 1);
        self.source
            .get(body_start..body_end)
            .unwrap_or_default()
            .to_string()
    }

    /// `node`'s source text, copied through unchanged: the fallback for
    /// every node type that has no formatter of its own (see `dispatch`).
    /// Unformatted is always better than wrong.
    ///
    /// Comments inside the node are part of the copied text, so the
    /// comment cursor skips past them rather than emitting them a second
    /// time.
    ///
    /// Known gap: a heredoc opened inside `node` has its body on the lines
    /// *after* the node's location, so it is not copied. The
    /// AST-equivalence check in `format_with_config` catches that and
    /// refuses the file.
    fn verbatim_text(&mut self, node: &ruby_prism::Node<'_>) -> String {
        let location = node.location();
        let (start, end) = (location.start_offset(), location.end_offset());
        let text = self.source.get(start..end).unwrap_or_default().to_string();
        while self.comment_cursor < self.comments.len()
            && (start..end).contains(&self.comments[self.comment_cursor].start)
        {
            self.comment_cursor += 1;
        }
        text
    }

    /// Whether `node` has to be copied through verbatim because a comment
    /// sits between its parts.
    fn should_write_verbatim(&mut self, node: &ruby_prism::Node<'_>) -> bool {
        if self.single_line {
            return false;
        }
        let location = node.location();
        let (start, end) = (location.start_offset(), location.end_offset());
        // Cheap pre-check before walking the expression.
        if !self.has_pending_comment_in(start, end) || !self.has_comment_in_gaps(node) {
            return false;
        }
        // A heredoc's body lives outside the node's own range, so copying
        // the range would lose it.
        !self
            .source
            .get(start..end)
            .unwrap_or_default()
            .contains("<<")
    }

    /// Safety valve for a comment in a place the emitter has nowhere to
    /// put it: between the operands of an operator, or between the links
    /// of a method chain.
    ///
    /// ```ruby
    /// methods = public_instance_methods(true) -
    ///   # Except for public instance methods of Base and its ancestors
    ///   Base.public_instance_methods(true)
    /// ```
    ///
    /// Those constructs are built out of one-line leaves, so the comment
    /// would stay pending and surface after the statement, describing
    /// nothing. If a pending comment sits in one of `node`'s gaps (see
    /// `has_comment_in_gaps`), `Some` with `node`'s source text copied
    /// through instead: an unformatted expression beats a misplaced
    /// comment. `None` to carry on formatting normally.
    fn verbatim_doc_if_commented(&mut self, node: &ruby_prism::Node<'_>) -> Option<Doc> {
        if !self.should_write_verbatim(node) {
            return None;
        }
        Some(Doc::text(self.verbatim_text(node)))
    }

    /// True if a pending comment sits *between* the parts of a chain or
    /// operator expression: after a receiver and before the `.method` or
    /// operator that follows it, or between an operator and its right-hand
    /// side. Walks down the receiver / left-hand spine, since
    /// `a.b # why` + `.c` nests the commented link inside the outer call.
    ///
    /// Deliberately not "anywhere inside `node`": a comment inside a
    /// receiver's own brackets (`{ # why ... }.freeze`) is that list's to
    /// place, and going verbatim there would give up formatting the list.
    fn has_comment_in_gaps(&self, node: &ruby_prism::Node<'_>) -> bool {
        if let Some(call) = node.as_call_node() {
            let Some(receiver) = call.receiver() else {
                return false;
            };
            let gap_start = receiver.location().end_offset();
            let is_operator = call.call_operator_loc().is_none()
                && !call
                    .name()
                    .as_slice()
                    .first()
                    .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'[');
            let gap_end = if is_operator {
                call.arguments()
                    .and_then(|a| a.arguments().iter().next())
                    .map(|arg| arg.location().start_offset())
            } else {
                call.message_loc().map(|m| m.start_offset())
            };
            return gap_end.is_some_and(|end| self.has_pending_comment_in(gap_start, end))
                || self.has_comment_in_gaps(&receiver);
        }
        let (left, right) = if let Some(and) = node.as_and_node() {
            (and.left(), and.right())
        } else if let Some(or) = node.as_or_node() {
            (or.left(), or.right())
        } else if let Some(parens) = node.as_parentheses_node() {
            // `(a -\n # why\n b).uniq`
            return parens.body().is_some_and(|body| {
                body.as_statements_node().is_some_and(|statements| {
                    statements
                        .body()
                        .iter()
                        .any(|s| self.has_comment_in_gaps(&s))
                })
            });
        } else {
            return false;
        };
        self.has_pending_comment_in(
            left.location().end_offset(),
            right.location().start_offset(),
        ) || self.has_comment_in_gaps(&left)
            || self.has_comment_in_gaps(&right)
    }

    /// Starts an [`InlineRun`].
    fn begin_run(&self) -> InlineRun {
        InlineRun { leaves: Vec::new() }
    }

    /// Appends literal text to a run.
    fn run_text(&mut self, run: &mut InlineRun, text: impl Into<String>) {
        run.leaves.push(Doc::text(text.into()));
    }

    /// `run_text` from source bytes: text that isn't valid UTF-8 renders
    /// as nothing rather than as replacement characters, which the AST
    /// check would then flag.
    fn run_bytes(&mut self, run: &mut InlineRun, bytes: &[u8]) {
        run.leaves.push(bytes_doc(bytes));
    }

    /// Appends a child node to a run.
    fn run_node(&mut self, run: &mut InlineRun, node: &ruby_prism::Node<'_>) {
        let doc = self.node_doc(node);
        run.leaves.push(doc);
    }

    /// Ends a run: the pieces one after another, with no line point
    /// between them.
    fn end_run(&mut self, run: InlineRun) -> Doc {
        Doc::concat(run.leaves)
    }

    /// An infix chain (`a && b && c`, `a + b + c`) as one all-or-nothing
    /// group: every operand on the line, or each on its own at one indent
    /// with the operator left behind at the end of the line before it.
    ///
    /// The whole spine is one group rather than one per operator: nested
    /// groups each decide on their own and the result bin-packs
    /// (`a &&\n  b && c`), which is the thing Tuxedo exists not to do.
    /// The first operand is left outside the `indent`: it starts the line
    /// at the statement's own indent, so line points of its own belong at
    /// that indent too, not one level in from it.
    fn infix_doc(&self, leaves: Vec<Doc>, op: &str, condition: bool) -> Doc {
        if !leaves.iter().any(Doc::breaks) {
            return self.chain_doc(leaves, op, condition);
        }

        // An operand that is already multi-line ends the group it is in.
        // A group holding one can never fit, so it would break at *every*
        // one of its line points, and
        //
        //     a &&
        //       begin
        //         ...
        //       end
        //
        // reads worse than `a && begin ... end` for no gain: that operand
        // was going to be multi-line either way. The operands around it
        // still get a group each, so a spine that is only partly
        // multi-line keeps the line points it can still use.
        //
        // Asking the built document rather than the operand's node type
        // catches every such operand, including one that is multi-line for
        // a reason no list of keyword node types would name.
        let mut parts = Vec::new();
        let mut run: Vec<Doc> = Vec::new();
        for leaf in leaves {
            if leaf.breaks() {
                if !run.is_empty() {
                    parts.push(self.chain_doc(std::mem::take(&mut run), op, condition));
                }
                parts.push(leaf);
            } else {
                run.push(leaf);
            }
        }
        if !run.is_empty() {
            parts.push(self.chain_doc(run, op, condition));
        }

        // Nothing joins the segments but the operator: a line point here
        // is one the printer would always take.
        let mut parts = parts.into_iter();
        let first = parts.next().unwrap_or_else(Doc::nil);
        first.append(Doc::concat(
            parts.map(|part| Doc::text(format!(" {op} ")).append(part)),
        ))
    }

    /// The whole spine as one group: flat, or one operand per line.
    ///
    /// A spine that is a block condition takes a second level, so that a
    /// condition which breaks is not drawn at the same indent as the body
    /// underneath it -- see `condition_doc`.
    fn chain_doc(&self, leaves: Vec<Doc>, op: &str, condition: bool) -> Doc {
        let levels = if condition { 2 } else { 1 };
        let mut leaves = leaves.into_iter();
        let first = leaves.next().unwrap_or_else(Doc::nil);
        let rest = Doc::concat(
            leaves.map(|leaf| Doc::text(format!(" {op}")).append(Doc::line()).append(leaf)),
        );
        first
            .append(rest.indent(levels * self.indent_str.len()))
            .group()
    }
}

/// `Doc::text` from a span of the source.
///
/// The source is a `&str` and Prism's locations land on token
/// boundaries, so the bytes are valid UTF-8 everywhere this is reached.
/// Dropping the span is the safe way to be wrong about that anyway: the
/// AST check sees a program with a piece missing and refuses the file,
/// where a replacement character could parse as something.
fn bytes_doc(bytes: &[u8]) -> Doc {
    Doc::text(std::str::from_utf8(bytes).unwrap_or("").to_string())
}
