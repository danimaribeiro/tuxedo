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

/// True if `node` is a call or lambda whose block hard-breaks onto its own
/// lines regardless of width -- a `do...end` block, which always renders
/// multi-line via `self.indent`/`write_indent`. A `{ }` block is not one of
/// these: its body is always forced onto one physical output line (see
/// `write_brace_block_body`'s `self.single_line = true`), so it carries no raw `\n`
/// a `Doc` wrapper would need to account for.
fn has_multiline_block(node: &ruby_prism::Node<'_>) -> bool {
    if let Some(call) = node.as_call_node() {
        return call
            .block()
            .and_then(|block| block.as_block_node())
            .is_some_and(|block| block.opening_loc().as_slice() != b"{");
    }
    if let Some(lambda) = node.as_lambda_node() {
        return lambda.opening_loc().as_slice() != b"{";
    }
    false
}

/// True for a node whose formatter lays out its own multi-line body
/// imperatively (via `self.indent`/`write_indent`) rather than through a
/// composable `Doc`. Ternaries and modifier `if`/`unless`/`while`/`until`
/// are excluded: they render on one line (or via their own Doc wrap) and
/// must not kick a surrounding call/array off the Doc-wrap path.
///
/// Such a node can't be safely captured with `capture_as_doc` and then
/// nested inside a `Doc::line()`-driven wrapper: the wrapper only shifts
/// the break immediately before the captured text, not the raw newlines
/// already baked into it, so only the first line would move.
pub(super) fn manages_own_indentation(node: &ruby_prism::Node<'_>) -> bool {
    if let Some(if_node) = node.as_if_node() {
        // Block `if`/`elsif` has an `end`; ternary has no `if` keyword;
        // modifier `x if y` has an `if` keyword but no `end`.
        return if_node.if_keyword_loc().is_some() && if_node.end_keyword_loc().is_some();
    }
    if let Some(unless_node) = node.as_unless_node() {
        return unless_node.end_keyword_loc().is_some();
    }
    if let Some(while_node) = node.as_while_node() {
        return while_node.closing_loc().is_some();
    }
    if let Some(until_node) = node.as_until_node() {
        return until_node.closing_loc().is_some();
    }
    matches!(
        node,
        ruby_prism::Node::CaseNode { .. }
            | ruby_prism::Node::CaseMatchNode { .. }
            | ruby_prism::Node::BeginNode { .. }
            | ruby_prism::Node::ForNode { .. }
            | ruby_prism::Node::DefNode { .. }
            | ruby_prism::Node::ClassNode { .. }
            | ruby_prism::Node::ModuleNode { .. }
            | ruby_prism::Node::SingletonClassNode { .. }
    ) || has_multiline_block(node)
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

    let mut formatter = Formatter::new(source, &parsed, config);
    formatter.visit(&node);
    formatter.flush_remaining_comments();
    formatter.splice_heredoc_bodies();
    let trimmed = formatter.output.trim_end_matches('\n');
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

/// A `Doc` being assembled left to right for a construct whose children
/// are not all on the `Doc` path yet.
///
/// Each child is written into `output` as it is added, so the next one
/// measures the column it will really start at -- a hash value has to see
/// its key, a splat's expression has to see the `*`. The text is dropped
/// once the run ends (`end_run`); only the `Doc`s are kept. Without it a
/// child that still writes strings would decide its own layout as though
/// its parent had written nothing, and lay out an overlong value flat.
pub(super) struct InlineRun {
    start: usize,
    leaves: Vec<Doc>,
}

struct CaptureSnapshot {
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
    output: String,
    indent: usize,
    inline: bool,
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
    /// Extra width to reserve when computing how much room a `Doc` group
    /// has to render flat -- for content that a caller knows will be
    /// appended to the same line right after the group finishes (e.g. a
    /// call's `do |params|` block header, appended by `write_call_block`
    /// right after `write_call_args` renders). `render_doc_to_string` only
    /// sees `current_column()`, i.e. what's already in `output` *before*
    /// the group -- it has no way to see what comes after, so without this
    /// a group can render flat and still push the full line past
    /// `line_length`. See `do_end_header_width`.
    trailing_reserve: usize,
    /// How many `capture_to_string` frames are active, plus any raised by
    /// `nested_node_doc`. A `{ }` body or implicit string-concat wrap that
    /// injects raw newlines while the result is being built as one leaf
    /// would bake those breaks in, so a surrounding Doc nest (e.g.
    /// `a || b { ... }`) only shifts the first line. Stay flat while it is
    /// raised.
    capture_depth: usize,
    /// Set while a construct is being laid out on one physical line
    /// whatever it costs: a `{ }` body nested inside another, or one being
    /// measured for its flat form. A group that broke for width in there
    /// would put a newline into text the caller is about to close with
    /// ` }` on the same line.
    ///
    /// Not the same as `single_line`, which a parenthesized body also sets
    /// to join its statements with `; ` -- a group breaking inside *that*
    /// is how an overlong `(a || b)` wraps, and has to stay possible.
    forced_flat: bool,
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

/// The width a `Doc` is laid out against when nothing in it is allowed to
/// break (see `forced_flat`). Not `usize::MAX`: the printer adds the
/// column the document starts at to it.
const UNBOUNDED_WIDTH: usize = usize::MAX / 2;

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
            output: String::new(),
            indent: 0,
            inline: false,
            single_line: false,
            indent_str: " ".repeat(config.indent_size),
            line_length: config.line_length,
            comments,
            comment_cursor: 0,
            line_starts: line_starts(source),
            source: source.to_string(),
            pending_heredocs: Vec::new(),
            heredoc_end_line: None,
            trailing_reserve: 0,
            capture_depth: 0,
            forced_flat: false,
            clause_closer: None,
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str(&self.indent_str);
        }
    }

    fn line_of(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }

    /// The display width of the current line of `output` rendered so far,
    /// i.e. where the next character would land. Derived on demand (rather
    /// than tracked incrementally) so there's no counter that can drift out
    /// of sync with `output`.
    fn current_column(&self) -> usize {
        let line_start = self.output.rfind('\n').map_or(0, |i| i + 1);
        unicode_width::UnicodeWidthStr::width(&self.output[line_start..])
    }

    /// The absolute column the current indent level starts at, i.e. how many
    /// spaces `write_indent` would emit right now.
    fn indent_width(&self) -> usize {
        self.indent * self.indent_str.len()
    }

    /// Renders a `Doc` value to a string, laid out where it will actually
    /// sit: its first line continues the output line written so far, and
    /// every line it breaks onto starts at the indent in effect.
    ///
    /// Both of those are told to the printer in absolute columns -- the
    /// document is padded out to the current column and rendered against
    /// the full `line_length` -- rather than by handing it the width left
    /// on the current line. The difference shows up on the second and
    /// later lines: with a remaining width, a document that starts at
    /// column 90 is measured as having 10 columns to play with *on every
    /// line*, including the ones that begin back at the statement's own
    /// indent with the whole line ahead of them. A nested group would then
    /// break for want of room it actually has, and the deeper it sat the
    /// less room it thought it had.
    ///
    /// The indent each `Doc::indent` counts from is the statement's, not
    /// the left margin, which is what lets one construct's `Doc` nest
    /// inside another's: an indent relative to the left margin would be
    /// right only for whichever of the two was rendered at the top, and
    /// would double-count in the other.
    ///
    /// `trailing_reserve` comes off the line length instead of the column,
    /// so it holds room at the *end* of the line for something not in the
    /// document yet (a block's ` do |x|` header). `forced_flat` takes the
    /// width out of the picture altogether.
    fn render_doc_to_string(&self, doc: Doc) -> String {
        let column = self.current_column();
        let width = if self.forced_flat {
            UNBOUNDED_WIDTH
        } else {
            self.line_length.saturating_sub(self.trailing_reserve)
        };
        let padded = Doc::text(" ".repeat(column)).append(doc.indent(self.indent_width()));
        let rendered = padded.render(width);
        // The pad is one space per column and nothing breaks inside it, so
        // it is exactly `column` bytes at the front of the result.
        rendered[column..].to_string()
    }

    /// The width of a `do...end` block's header line (` do` plus an
    /// optional ` |params|`) that will follow immediately after the
    /// current call's argument list once `write_call_block` renders it --
    /// 0 for a brace block (its body is captured and measured
    /// independently, not appended inline after another Doc) or no block
    /// at all. Meant to be added to `trailing_reserve` before Doc-wrapping
    /// whatever precedes the block on the same line.
    fn do_end_header_width(&mut self, block: Option<ruby_prism::Node<'_>>) -> usize {
        let Some(block) = block.and_then(|b| b.as_block_node()) else {
            return 0;
        };
        if block.opening_loc().as_slice() == b"{" {
            return 0;
        }
        let mut width = " do".len();
        if let Some(params) = explicit_parameters(block.parameters()) {
            // Bare pipe-delimited block params never wrap (see
            // fmt_block_parameters_node), so this is always a single line.
            let captured = self.node_doc(&params);
            let rendered = self.render_doc_to_string(captured);
            width += 1 + unicode_width::UnicodeWidthStr::width(rendered.as_str());
        }
        width
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

    /// Captures whatever `render` writes to `self.output` into an owned
    /// string, restoring the real `output` afterwards, and wraps the capture
    /// as an opaque `Doc::text` leaf. This is how content that hasn't been
    /// ported to build `Doc` values natively (i.e. everything, today) still
    /// renders correctly inside a `Doc`-based construct: it's rendered once,
    /// the normal way, and treated as an unsplittable block of text.
    fn capture_as_doc(&mut self, render: impl FnOnce(&mut Self)) -> Doc {
        Doc::text(self.capture_to_string(render))
    }

    /// The same as `capture_as_doc` for everything the caller sees, but
    /// `render` writes into `output` where its text will actually sit and
    /// the text is only lifted back out afterwards, instead of being
    /// rendered into a detached buffer.
    ///
    /// Use this wherever the old string emitter wrote in place and the
    /// result goes straight back at the same position. Everything `render`
    /// can look at is then what it used to see: `current_column` is the
    /// real one, `capture_depth` stays put (so a nested list still takes
    /// its comment branch, and an adjacent-literal run still wraps), and
    /// the bytes are identical. `capture_as_doc` is for the other case --
    /// a child being *measured* for a group that may still move it, where
    /// there is no position to render at yet.
    fn spliced_doc(&mut self, render: impl FnOnce(&mut Self)) -> Doc {
        let start = self.output.len();
        render(self);
        Doc::text(self.output.split_off(start))
    }

    fn capture_to_string(&mut self, render: impl FnOnce(&mut Self)) -> String {
        let saved = std::mem::take(&mut self.output);
        self.capture_depth += 1;
        render(self);
        self.capture_depth -= 1;
        std::mem::replace(&mut self.output, saved)
    }

    /// Side effects `node_doc` / `capture_as_doc` can take that a
    /// subsequent re-`visit` would take again -- comments consumed, heredoc
    /// bodies queued, indent left shifted if a visitor failed to restore it.
    fn capture_snapshot(&self) -> CaptureSnapshot {
        CaptureSnapshot {
            comment_cursor: self.comment_cursor,
            pending_heredocs_len: self.pending_heredocs.len(),
            heredoc_end_line: self.heredoc_end_line,
            indent: self.indent,
        }
    }

    fn restore_capture_snapshot(&mut self, snap: CaptureSnapshot) {
        self.comment_cursor = snap.comment_cursor;
        self.pending_heredocs.truncate(snap.pending_heredocs_len);
        self.heredoc_end_line = snap.heredoc_end_line;
        self.indent = snap.indent;
    }

    /// Parenthesized/bracketed list: expand all-or-nothing if any item
    /// already hard-breaks *or* comes back from `node_doc` multi-line (a
    /// nested array/hash/call that wrapped). The second case has to be
    /// asked about rather than left to the printer, because such an item is
    /// still a captured `Doc::text` leaf with newlines baked in: the
    /// printer measures it as one long line, so sibling items would share a
    /// line with an already-broken collection, at an indent nothing can
    /// correct. See `Doc::breaks`.
    ///
    /// `span` is the source range between the brackets (end of the opening
    /// token, start of the closing one). A comment anywhere in it forces
    /// the expanded shape too, with each comment kept next to its item --
    /// see `write_commented_bracketed_items`.
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
            && self.capture_depth == 0
            && self.has_pending_comment_in(start, end)
        {
            // The one shape still laid out imperatively: it consumes
            // comments from `comment_cursor` as it writes, and a comment
            // has no `Doc` form. Written where it will sit and lifted
            // straight back out, so it is the same bytes as writing it in
            // place -- see `spliced_doc`.
            return self.spliced_doc(|f| {
                f.write_commented_bracketed_items(open, items, close, (start, end))
            });
        }
        let snap = self.capture_snapshot();
        let leaves: Vec<Doc> = items.iter().map(|item| self.node_doc(item)).collect();
        if leaves.iter().any(Doc::breaks) {
            self.restore_capture_snapshot(snap);
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
        {
            let text = Self::format_comment(&self.comments[self.comment_cursor].text).into_owned();
            self.comment_cursor += 1;
            return Some(Doc::text(format!(" {text}")));
        }
        None
    }

    fn emit_trailing_comment_if_same_line(&mut self, line: usize) {
        if self.comment_cursor < self.comments.len()
            && self.line_of(self.comments[self.comment_cursor].start) == line
        {
            if self.output.ends_with('\n') {
                self.output.pop();
            }
            self.output.push(' ');
            self.output.push_str(&Self::format_comment(
                &self.comments[self.comment_cursor].text,
            ));
            self.output.push('\n');
            self.comment_cursor += 1;
        }
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
    fn flush_comments_before(&mut self, offset: usize, after_statements: bool) {
        self.flush_comments(offset, after_statements, true);
    }

    /// `flush_comments_before` for a position that is not the bottom of a
    /// body -- above the first `when` of a `case`, above a list item -- so
    /// there is only one level a comment can sit at: the current one.
    fn flush_leading_comments(&mut self, offset: usize, after_previous: bool) {
        self.flush_comments(offset, after_previous, false);
    }

    fn flush_comments(&mut self, offset: usize, after_statements: bool, bottom_of_body: bool) {
        if self.single_line || self.capture_depth > 0 {
            return;
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

        let mut first = true;
        while self.comment_cursor < self.comments.len()
            && self.comments[self.comment_cursor].start < offset
        {
            let start = self.comments[self.comment_cursor].start;
            let line = self.line_of(start);
            let column = start - self.line_starts[line];
            let at_keyword_level =
                closer_opens_branch && column <= closer_column && self.indent > 0;
            if (after_statements || !first) && self.source_line_is_blank(line.wrapping_sub(1)) {
                self.ensure_blank_line();
            }
            if !self.output.ends_with('\n') {
                self.output.push('\n');
            }
            if at_keyword_level {
                self.indent -= 1;
                self.write_indent();
                self.indent += 1;
            } else {
                self.write_indent();
            }
            let formatted =
                Self::format_comment(&self.comments[self.comment_cursor].text).into_owned();
            self.output.push_str(&formatted);
            self.output.push('\n');
            self.comment_cursor += 1;
            first = false;
        }
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

    fn flush_remaining_comments(&mut self) {
        while self.comment_cursor < self.comments.len() {
            self.write_indent();
            self.output.push_str(&Self::format_comment(
                &self.comments[self.comment_cursor].text,
            ));
            self.output.push('\n');
            self.comment_cursor += 1;
        }
    }

    fn ensure_blank_line(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
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

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.output
            .push_str(std::str::from_utf8(bytes).unwrap_or(""));
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
        let body = self.expanded_items_doc(items, true);
        self.indent -= 1;
        Doc::text(open)
            .append(Doc::hardline().append(body).indent(nest_width))
            .append(Doc::hardline())
            .append(Doc::text(close))
    }

    /// The expanded shape of `write_expanded_bracketed_items`, for a list
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
    fn write_commented_bracketed_items(
        &mut self,
        open: &str,
        items: &[ruby_prism::Node<'_>],
        close: &str,
        (opening_end, closing_start): (usize, usize),
    ) {
        self.output.push_str(open);
        self.output.push('\n');
        // `foo( # why`: nothing to attach it to but the list itself.
        self.emit_trailing_comment_if_same_line(self.line_of(opening_end));
        self.indent += 1;
        let last = items.len().saturating_sub(1);
        for (i, item) in items.iter().enumerate() {
            self.flush_leading_comments(item.location().start_offset(), i > 0);
            self.write_indent();
            self.inline = true;
            self.visit(item);
            while self.output.ends_with('\n') {
                self.output.pop();
            }
            if i != last {
                self.output.push(',');
            }
            self.output.push('\n');

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
            if is_trailing {
                self.emit_trailing_comment_if_same_line(item_end_line);
            }
        }
        self.flush_comments_before(closing_start, !items.is_empty());
        self.indent -= 1;
        self.write_indent();
        self.output.push_str(close);
    }

    /// `items` one per line at the current indent, comma-separated.
    /// `first_on_new_line` says whether the first item starts a line of
    /// its own (inside a bracket that has just broken) or continues the
    /// line already being written (a bare keyword-argument list, which has
    /// no bracket to sit under).
    ///
    /// Each item is rendered into `output` at the column and indent it
    /// will actually occupy and only then lifted back out, rather than
    /// captured into a detached buffer: an item that measures where it
    /// starts (an interpolated literal, a nested argument list) has to see
    /// the real column, and one that lays out its own body (a `do...end`
    /// block) has to see the real indent. The separators are rebuilt as
    /// `Doc`s, so they render back to the same bytes -- a `hardline`
    /// inside the caller's `indent` is exactly the newline plus
    /// `write_indent` this used to push by hand.
    fn expanded_items_doc(
        &mut self,
        items: &[ruby_prism::Node<'_>],
        first_on_new_line: bool,
    ) -> Doc {
        let start = self.output.len();
        let mut leaves = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            if i > 0 || first_on_new_line {
                self.output.push('\n');
                self.write_indent();
            }
            let item_start = self.output.len();
            self.inline = true;
            self.visit(item);
            while self.output.len() > item_start && self.output.ends_with('\n') {
                self.output.pop();
            }
            leaves.push(Doc::text(self.output[item_start..].to_string()));
        }
        self.output.truncate(start);
        Doc::join(leaves, Doc::text(",").append(Doc::hardline()))
    }

    /// The expanded shape of a bare command argument list: the first
    /// argument stays on the call line, every later one starts a line of
    /// its own at one indent. Ruby has no paren-less way to put the first
    /// argument on a new line, and Tuxedo does not rewrite command style
    /// into a parenthesized call.
    ///
    /// The first argument is rendered at the statement indent because it
    /// continues the call line; the later ones sit inside the `indent`, so
    /// a nested collection of theirs that wraps uses that deeper indent for
    /// its own brackets. Each is rendered where it will actually sit and
    /// lifted back out, the same bargain `expanded_items_doc` makes.
    fn expanded_bare_args_doc(&mut self, items: &[ruby_prism::Node<'_>]) -> Doc {
        let start = self.output.len();
        self.output.push(' ');
        let mut leaves = Vec::with_capacity(items.len());
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.output.push('\n');
                self.indent += 1;
                self.write_indent();
            }
            let item_start = self.output.len();
            self.inline = true;
            self.visit(item);
            while self.output.len() > item_start && self.output.ends_with('\n') {
                self.output.pop();
            }
            leaves.push(Doc::text(self.output[item_start..].to_string()));
            if i > 0 {
                self.indent -= 1;
            }
        }
        self.output.truncate(start);
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
        let snap = self.capture_snapshot();
        let leaves: Vec<Doc> = items.iter().map(|item| self.node_doc(item)).collect();
        if leaves.iter().any(Doc::breaks) {
            self.restore_capture_snapshot(snap);
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
        // A marker rather than an offset into `output`: argument lists are
        // rendered into a detached buffer by `capture_as_doc`, so an
        // offset taken there points into that temporary and not into the
        // final output. A marker travels with the text wherever it is
        // spliced.
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
    fn splice_heredoc_bodies(&mut self) {
        if self.pending_heredocs.is_empty() {
            return;
        }
        let bodies = std::mem::take(&mut self.pending_heredocs);
        let rendered = std::mem::take(&mut self.output);

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
        self.output = out;
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

    /// Copies `node`'s source text through unchanged: the fallback for
    /// every node type that has no formatter of its own yet (see
    /// `dispatch`). Unformatted is always better than wrong.
    ///
    /// Comments inside the node are part of the copied text, so the comment
    /// cursor skips past them rather than emitting them a second time.
    ///
    /// Known gap: a heredoc opened inside `node` has its body on the lines
    /// *after* the node's location, so it is not copied. The
    /// AST-equivalence check in `format_with_config` catches that and
    /// refuses the file.
    fn write_verbatim(&mut self, node: &ruby_prism::Node<'_>) {
        let text = self.verbatim_text(node);
        self.write_leaf(|f| f.output.push_str(&text));
    }

    /// `node`'s source text, with any comments inside it marked consumed so
    /// the cursor does not emit them a second time. Shared by
    /// [`Formatter::write_verbatim`] and its `Doc` form.
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
        if self.single_line || self.capture_depth > 0 {
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

    fn write_leaf(&mut self, write: impl FnOnce(&mut Self)) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        write(self);
        if !inline {
            self.output.push('\n');
        }
    }

    /// `write_leaf` for a node whose `fmt_*` returns a `Doc` instead of
    /// writing to `output`: render against the width left on the line,
    /// terminate. The indent is written by `visit` before the `fmt_*`
    /// runs, not here, so that both building the `Doc` and rendering it
    /// see the column the node actually starts at -- a literal with a
    /// `#{ }` hole in it measures that column while it builds.
    ///
    /// `inline` is passed in rather than read from `self` because building
    /// the `Doc` happens first and can clobber the flag on the way
    /// (`fmt_parentheses_node` visits a body that clears it). `write_leaf`
    /// reads the flag before running its closure, so this has to use the
    /// value from before the `fmt_*` call to lay a node out the same way.
    fn write_leaf_doc(&mut self, inline: bool, doc: Doc) {
        let rendered = self.render_doc_to_string(doc);
        self.output.push_str(&rendered);
        // A node whose last child lays out its own multi-line body (`x = if
        // a ... end`) has already ended the line; a second newline here
        // would open a blank one.
        if !inline && !rendered.ends_with('\n') {
            self.output.push('\n');
        }
    }

    /// Starts an [`InlineRun`] at the current end of `output`.
    fn begin_run(&self) -> InlineRun {
        InlineRun {
            start: self.output.len(),
            leaves: Vec::new(),
        }
    }

    /// Appends literal text to a run.
    fn run_text(&mut self, run: &mut InlineRun, text: impl Into<String>) {
        let text = text.into();
        self.output.push_str(&text);
        run.leaves.push(Doc::text(text));
    }

    /// `run_text` from source bytes, matching `write_bytes`.
    fn run_bytes(&mut self, run: &mut InlineRun, bytes: &[u8]) {
        let text = std::str::from_utf8(bytes).unwrap_or("").to_string();
        self.run_text(run, text);
    }

    /// Appends a child node to a run, inline.
    ///
    /// A node that builds a `Doc` is kept as that `Doc`, so the enclosing
    /// group lays it out for real; it is also rendered into `output` and
    /// the text thrown away, purely to carry the column forward. A node
    /// that still writes strings is rendered into `output` where it sits
    /// and that text becomes its leaf.
    fn run_node(&mut self, run: &mut InlineRun, node: &ruby_prism::Node<'_>) {
        let start = self.output.len();
        self.inline = true;
        match self.leaf_doc(node) {
            Some(doc) => {
                let rendered = self.render_doc_to_string(doc.clone());
                self.output.push_str(&rendered);
                run.leaves.push(doc);
            }
            None => {
                self.visit(node);
                run.leaves.push(Doc::text(self.output[start..].to_string()));
            }
        }
    }

    /// Ends a run: drops the text it wrote and returns what it built.
    ///
    /// Unless one of its children came back at fixed columns, in which
    /// case the text is what it built -- the run wrote every child where
    /// it will sit, and a document mixing a fixed leaf with a group the
    /// printer has yet to lay out gets that group wrong (see
    /// [`Doc::fixed_lines`]). Taking the text keeps exactly the bytes the
    /// run already produced.
    fn end_run(&mut self, run: InlineRun) -> Doc {
        let doc = Doc::concat(run.leaves);
        if doc.fixed_lines() {
            return Doc::text(self.output.split_off(run.start));
        }
        self.output.truncate(run.start);
        doc
    }

    /// [`Formatter::node_doc`] for a child the caller will place after a
    /// line point of its own -- an `&&`'s operand, an infix operator's
    /// right-hand side.
    ///
    /// A child at fixed columns (see [`Doc::fixed_lines`]) cannot go
    /// there: the indent would move the break in front of it and leave the
    /// newlines inside it where they were. So it is asked for again with
    /// `capture_depth` raised, which is what holds a `{ }` body flat --
    /// one line always nests. Asking first rather than raising the depth
    /// up front is what keeps a child that wraps through the printer (an
    /// adjacent-literal concatenation's `\` breaks) free to do so: those
    /// line points are the printer's, and they indent like any other.
    fn nested_node_doc(&mut self, node: &ruby_prism::Node<'_>) -> Doc {
        let snap = self.capture_snapshot();
        let doc = self.node_doc(node);
        if !doc.fixed_lines() {
            return doc;
        }
        self.restore_capture_snapshot(snap);
        self.capture_depth += 1;
        let doc = self.node_doc(node);
        self.capture_depth -= 1;
        doc
    }

    /// `head` and `tail` one after the other, laid out in place when either
    /// of them already carries lines of its own.
    ///
    /// The printer measures a leaf with newlines in it as one long line
    /// rather than as the column its last line really ends at (see
    /// [`Doc::fixed_lines`]). A group after one is laid out against a width
    /// that has nothing to do with where it sits -- an argument list after
    /// a `do...end` receiver expanded every time -- and a group before one
    /// is measured as though the whole body had to fit on its line.
    /// Rendering both here, where they go, is what the string emitter did,
    /// and the pair comes back as one leaf.
    fn join_after_head(&mut self, head: Doc, tail: Doc) -> Doc {
        if !head.fixed_lines() && !tail.fixed_lines() {
            return head.append(tail);
        }
        self.spliced_doc(|f| {
            let rendered = f.render_doc_to_string(head);
            f.output.push_str(&rendered);
            let rendered = f.render_doc_to_string(tail);
            f.output.push_str(&rendered);
        })
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
    fn infix_chain_doc(&self, leaves: Vec<Doc>, op: &str) -> Doc {
        let mut leaves = leaves.into_iter();
        let first = leaves.next().unwrap_or_else(Doc::nil);
        let rest = Doc::concat(
            leaves.map(|leaf| Doc::text(format!(" {op}")).append(Doc::line()).append(leaf)),
        );
        first.append(rest.indent(self.indent_str.len())).group()
    }

    /// The same chain on one line, each operand written where it will sit:
    /// the shape for a spine with an operand that lays out its own
    /// multi-line body, which no line point of a group could move.
    fn flat_infix_doc(&mut self, operands: &[ruby_prism::Node<'_>], op: &str) -> Doc {
        let mut run = self.begin_run();
        for (i, operand) in operands.iter().enumerate() {
            if i > 0 {
                self.run_text(&mut run, format!(" {op} "));
            }
            self.run_node(&mut run, operand);
        }
        self.end_run(run)
    }

    /// The `Doc` for `node` rendered inline, wherever a construct needs one
    /// of its children as a value rather than as output.
    ///
    /// For a node whose `fmt_*` already returns a `Doc` this is that `Doc`,
    /// composable and measured by the enclosing group. For every other node
    /// it is still the node's string output wrapped as one opaque leaf, so
    /// this is the seam the rest of the migration moves nodes across.
    fn node_doc(&mut self, node: &ruby_prism::Node<'_>) -> Doc {
        // Set before either branch, not just inside the capture: today's
        // callers leave `inline` true afterwards, and something downstream
        // reads it (`write_leaf` never clears it). Keeping the flag's
        // behaviour identical is what makes moving a node type onto the
        // `Doc` path a no-op for output.
        self.inline = true;
        if let Some(doc) = self.leaf_doc(node) {
            return doc;
        }
        self.capture_as_doc(|f| f.visit(node))
    }
}

/// `Doc::text` from source bytes, matching `write_bytes`: text that isn't
/// valid UTF-8 renders as nothing rather than as replacement characters,
/// which the AST check would then flag.
fn bytes_doc(bytes: &[u8]) -> Doc {
    Doc::text(std::str::from_utf8(bytes).unwrap_or("").to_string())
}
