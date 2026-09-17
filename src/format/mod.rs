use crate::config::FormatConfig;
mod assignment;
mod calls;
mod classes;
mod collections;
mod control_flow;
mod defs;
mod dispatch;
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
/// `fmt_block_node`'s `self.single_line = true`), so it carries no raw `\n`
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

/// True if rendering `node` inline would emit a raw newline -- either
/// because `node` itself manages its own indentation, or because a nested
/// collection/call/operator contains one that does. Used to decide that a
/// parenthesized list can't stay on one line and must expand
/// all-or-nothing rather than going through `capture_as_doc` (which would
/// bake those newlines in and then misalign them under a Doc nest).
///
/// Does not walk into `{ }` / parenthesized bodies: those force
/// `single_line` and join inner `if`/`def`/… with `; `, so they stay one
/// physical line.
pub(super) fn forces_list_break(node: &ruby_prism::Node<'_>) -> bool {
    if manages_own_indentation(node) || assoc_forces_list_break(node) {
        return true;
    }
    if let Some(array) = node.as_array_node() {
        return array.elements().iter().any(|e| forces_list_break(&e));
    }
    if let Some(hash) = node.as_hash_node() {
        return hash.elements().iter().any(|e| assoc_forces_list_break(&e));
    }
    if let Some(kw) = node.as_keyword_hash_node() {
        return kw.elements().iter().any(|e| assoc_forces_list_break(&e));
    }
    if let Some(splat) = node.as_splat_node() {
        return splat.expression().is_some_and(|e| forces_list_break(&e));
    }
    if let Some(call) = node.as_call_node() {
        return call
            .arguments()
            .is_some_and(|a| a.arguments().iter().any(|arg| forces_list_break(&arg)));
    }
    if let Some(super_node) = node.as_super_node() {
        return super_node
            .arguments()
            .is_some_and(|a| a.arguments().iter().any(|arg| forces_list_break(&arg)));
    }
    if let Some(if_node) = node.as_if_node() {
        return forces_list_break(&if_node.predicate())
            || statements_force_list_break(if_node.statements())
            || if_node.subsequent().is_some_and(|s| forces_list_break(&s));
    }
    if let Some(unless_node) = node.as_unless_node() {
        return forces_list_break(&unless_node.predicate())
            || statements_force_list_break(unless_node.statements())
            || unless_node
                .else_clause()
                .is_some_and(|e| statements_force_list_break(e.statements()));
    }
    if let Some(else_node) = node.as_else_node() {
        return statements_force_list_break(else_node.statements());
    }
    if let Some(and_node) = node.as_and_node() {
        return forces_list_break(&and_node.left()) || forces_list_break(&and_node.right());
    }
    if let Some(or_node) = node.as_or_node() {
        return forces_list_break(&or_node.left()) || forces_list_break(&or_node.right());
    }
    false
}

fn statements_force_list_break(stmts: Option<ruby_prism::StatementsNode<'_>>) -> bool {
    stmts.is_some_and(|s| s.body().iter().any(|n| forces_list_break(&n)))
}

/// True for an `IfNode` that's actually a ternary (`predicate ? a : b`) --
/// Prism represents both with the same node type, distinguished only by the
/// absence of an `if`/`elsif` keyword location.
fn is_ternary_if(node: &ruby_prism::Node<'_>) -> bool {
    node.as_if_node()
        .is_some_and(|n| n.if_keyword_loc().is_none())
}

/// Like `forces_list_break`, but for a hash element (`AssocNode`/
/// `AssocSplatNode`): looks at the value being assigned rather than the
/// element itself, since a hash element is never itself a node
/// `forces_list_break` would match directly.
pub(super) fn assoc_forces_list_break(node: &ruby_prism::Node<'_>) -> bool {
    if let Some(assoc) = node.as_assoc_node() {
        return forces_list_break(&assoc.value());
    }
    if let Some(splat) = node.as_assoc_splat_node() {
        return splat.value().is_some_and(|value| forces_list_break(&value));
    }
    false
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
    /// `output` after each opening tag. See `write_heredoc` and
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
    /// How many `capture_to_string` frames are active. A `{ }` body or
    /// implicit string-concat wrap that injects raw newlines while the
    /// result is being captured as `Doc::text` would bake those breaks in
    /// so a surrounding Doc nest (e.g. `a || b { ... }`) only shifts the
    /// first line. Stay flat while capturing.
    capture_depth: usize,
}

/// Delimiters of the placeholder that stands in for a heredoc body between
/// `write_heredoc` and `splice_heredoc_bodies`. Unicode private-use
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
    /// spaces `write_indent` would emit right now. A `Doc` rendered via
    /// `render_doc_to_string` is laid out as if starting fresh at column 0,
    /// so any construct that nests its wrapped continuation lines needs to
    /// add this in as the baseline — otherwise a wrapped Doc's continuation
    /// lines would ignore how deeply the surrounding code is already
    /// indented and dedent back toward the left margin.
    fn indent_width(&self) -> usize {
        self.indent * self.indent_str.len()
    }

    /// Renders a `Doc` value to a string, using the width still available on
    /// the current output line before `line_length` is hit.
    fn render_doc_to_string(&self, doc: pretty::RcDoc<'static, ()>) -> String {
        let width = self
            .line_length
            .saturating_sub(self.current_column())
            .saturating_sub(self.trailing_reserve);
        doc.pretty(width).to_string()
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
            let captured = self.capture_as_doc(|f| {
                f.inline = true;
                f.visit(&params);
            });
            let rendered = self.render_doc_to_string(captured);
            width += 1 + unicode_width::UnicodeWidthStr::width(rendered.as_str());
        }
        width
    }

    /// Renders a bracketed, comma-or-other-separated group (array literal,
    /// hash literal, parenthesized call argument list, ...) as a single Doc
    /// group implementing Tuxedo's block-indentation spec: either the whole
    /// thing fits on one line, or the opening bracket sits alone, every
    /// element gets a fixed indent one level deeper than the current
    /// statement, and the closing bracket sits alone back at the statement's
    /// own indent -- never aligned under the opening bracket, so renaming
    /// the assignment target can't reindent the body.
    ///
    /// `open_sep`/`close_sep` are `Doc::line()` (renders as a space when the
    /// group stays flat -- hash braces: `{ a: 1 }`) or `Doc::line_()`
    /// (renders as nothing when flat -- arrays/call args: `[1, 2]`,
    /// `(1, 2)`), passed in so callers control the flat-mode spacing while
    /// sharing the same broken-mode layout.
    fn wrap_bracketed_group(
        &mut self,
        open: &str,
        open_sep: pretty::RcDoc<'static, ()>,
        body: pretty::RcDoc<'static, ()>,
        close_sep: pretty::RcDoc<'static, ()>,
        close: &str,
    ) -> String {
        let indent_width = self.indent_width();
        let nest_width = indent_width + self.indent_str.len();
        // The closing token must be nested *together with* close_sep, not
        // appended after the nest ends -- `pretty`'s nest only applies the
        // indent to a line break whose subsequent content is still inside
        // the same nested subtree.
        let doc = pretty::RcDoc::text(open.to_string())
            .append(open_sep.append(body).nest(nest_width as isize))
            .append(
                close_sep
                    .append(pretty::RcDoc::text(close.to_string()))
                    .nest(indent_width as isize),
            )
            .group();
        self.render_doc_to_string(doc)
    }

    /// Captures whatever `render` writes to `self.output` into an owned
    /// string, restoring the real `output` afterwards, and wraps the capture
    /// as an opaque `Doc::text` leaf. This is how content that hasn't been
    /// ported to build `Doc` values natively (i.e. everything, today) still
    /// renders correctly inside a `Doc`-based construct: it's rendered once,
    /// the normal way, and treated as an unsplittable block of text.
    fn capture_as_doc(&mut self, render: impl FnOnce(&mut Self)) -> pretty::RcDoc<'static, ()> {
        pretty::RcDoc::text(self.capture_to_string(render))
    }

    fn capture_to_string(&mut self, render: impl FnOnce(&mut Self)) -> String {
        let saved = std::mem::take(&mut self.output);
        self.capture_depth += 1;
        render(self);
        self.capture_depth -= 1;
        std::mem::replace(&mut self.output, saved)
    }

    fn capture_inline(&mut self, node: &ruby_prism::Node<'_>) -> String {
        self.capture_to_string(|f| {
            f.inline = true;
            f.visit(node);
        })
    }

    /// Side effects `capture_inline` / `capture_as_doc` can take that a
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
    /// already hard-breaks *or* captures as multi-line (a nested array/
    /// hash/call that wrapped during capture). Captured newlines can't go
    /// through `wrap_bracketed_group` -- `Doc::text` with a raw `\n` stays
    /// "flat" in the outer group, so sibling items share a line with the
    /// broken nested collection, and the nest can't reindent the baked-in
    /// continuation lines.
    fn write_bracketed_items(
        &mut self,
        open: &str,
        open_sep: pretty::RcDoc<'static, ()>,
        items: &[ruby_prism::Node<'_>],
        close_sep: pretty::RcDoc<'static, ()>,
        close: &str,
    ) {
        if items.iter().any(forces_list_break) {
            self.write_expanded_bracketed_items(open, items, close);
            return;
        }
        let snap = self.capture_snapshot();
        let captured: Vec<String> = items.iter().map(|item| self.capture_inline(item)).collect();
        if captured.iter().any(|s| s.contains('\n')) {
            self.restore_capture_snapshot(snap);
            self.write_expanded_bracketed_items(open, items, close);
            return;
        }
        let leaves = captured.into_iter().map(pretty::RcDoc::text);
        let body = pretty::RcDoc::intersperse(
            leaves,
            pretty::RcDoc::text(",").append(pretty::RcDoc::line()),
        );
        let rendered = self.wrap_bracketed_group(open, open_sep, body, close_sep, close);
        self.output.push_str(&rendered);
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

    fn write_assignment(&mut self, name: &[u8], op: &str, value: &ruby_prism::Node<'_>) {
        self.write_assignment_to(|f| f.write_bytes(name), op, value);
    }

    fn write_assignment_to(
        &mut self,
        write_target: impl FnOnce(&mut Self),
        op: &str,
        value: &ruby_prism::Node<'_>,
    ) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.inline = true;

        let start_len = self.output.len();
        write_target(self);
        self.output.push(' ');
        self.output.push_str(op);
        self.output.push(' ');
        let prefix_len = self.output.len() - start_len;

        // An `if`/`case` value should have its continuation lines/`end`
        // align under the `=` (rubocop's Layout/EndAlignment stock default
        // is `keyword`: align `end` with the keyword that opens the value,
        // not with the assignment target -- confirmed against
        // scripts/format-verify.rubocop.yml, which deliberately does not
        // override it, as opposed to a project's own .rubocop.yml, which
        // may). Everything else -- a wrapped array/hash/call-args literal
        // (block-indented at a fixed depth regardless of the assignment
        // target's length), adjacent string literals (`"foo" "bar"`), a
        // `do...end`/`{ }` block, a lambda literal, or an implicit/explicit
        // `begin` -- manages its own body indentation relative to the
        // statement's own indent instead.
        let skip_alignment = matches!(
            value,
            ruby_prism::Node::BeginNode { .. }
                | ruby_prism::Node::LambdaNode { .. }
                | ruby_prism::Node::ArrayNode { .. }
                | ruby_prism::Node::HashNode { .. }
                | ruby_prism::Node::CallNode { .. }
                | ruby_prism::Node::InterpolatedStringNode { .. }
        ) || has_real_block(value)
            || is_ternary_if(value);
        let extra_indent = if skip_alignment {
            0
        } else {
            prefix_len / self.indent_str.len()
        };
        self.indent += extra_indent;

        self.inline = true;
        self.visit(value);

        self.indent -= extra_indent;

        if !inline && !self.output.ends_with('\n') {
            self.output.push('\n');
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.output
            .push_str(std::str::from_utf8(bytes).unwrap_or(""));
    }

    /// Writes a must-break bracketed list: opening token alone, every item
    /// on its own line at indent+1, closing token alone at the current
    /// indent. Used when at least one item already hard-breaks (see
    /// `forces_list_break`), so a Doc group can't choose "fits on one
    /// line" -- it already doesn't -- and capturing those items as
    /// `Doc::text` would bake in newlines the nest can't shift.
    fn write_expanded_bracketed_items(
        &mut self,
        open: &str,
        items: &[ruby_prism::Node<'_>],
        close: &str,
    ) {
        self.output.push_str(open);
        self.output.push('\n');
        self.indent += 1;
        self.write_indent();
        self.write_expanded_items(items);
        self.output.push('\n');
        self.indent -= 1;
        self.write_indent();
        self.output.push_str(close);
    }

    /// Writes `items` one-per-line at the current indent, comma-separated,
    /// with the first item continuing on the current line. Caller is
    /// responsible for any leading indent (so this works both as the body
    /// of `write_expanded_bracketed_items` and for a bare keyword-arg list
    /// that's already sitting at the right column).
    fn write_expanded_items(&mut self, items: &[ruby_prism::Node<'_>]) {
        let last = items.len().saturating_sub(1);
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.output.push('\n');
                self.write_indent();
            }
            self.inline = true;
            self.visit(item);
            while self.output.ends_with('\n') {
                self.output.pop();
            }
            if i != last {
                self.output.push(',');
            }
        }
    }

    /// Bare command args that already span lines, or whose flat form
    /// would overflow `line_length`: first argument stays on the call
    /// line, every later argument starts on its own line at one indent.
    /// Ruby has no paren-less way to put the first argument on a new
    /// line, and Tuxedo does not rewrite command style into a
    /// parenthesized call.
    ///
    /// Subsequent arguments are visited at +1 indent so a nested
    /// collection that wraps uses that indent for its own brackets;
    /// the first argument is visited at the statement indent because it
    /// continues the call line.
    fn write_expanded_bare_args(&mut self, items: &[ruby_prism::Node<'_>]) {
        self.output.push(' ');
        let last = items.len().saturating_sub(1);
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.output.push('\n');
                self.indent += 1;
                self.write_indent();
            }
            self.inline = true;
            self.visit(item);
            while self.output.ends_with('\n') {
                self.output.pop();
            }
            if i != last {
                self.output.push(',');
            }
            if i > 0 {
                self.indent -= 1;
            }
        }
    }

    /// True if emitting `captured` as ` a, b, c` on the current line
    /// would pass `line_length` (including any `trailing_reserve` for a
    /// following `do |params|` header).
    fn bare_args_overflow(&self, captured: &[String]) -> bool {
        let args_width: usize = captured
            .iter()
            .map(|s| unicode_width::UnicodeWidthStr::width(s.as_str()))
            .sum();
        let seps = captured.len().saturating_sub(1) * ", ".len();
        let flat_width = " ".len() + args_width + seps;
        self.current_column()
            .saturating_add(flat_width)
            .saturating_add(self.trailing_reserve)
            > self.line_length
    }

    /// Renders a call's argument list, including a block-pass argument
    /// (`&:sym` / `&block`) if present. Prism stores a block-pass in the
    /// same `block()` slot as a real `{ }`/`do...end` block, but unlike a
    /// real block it belongs inside the argument list (comma-joined with
    /// any positional args), not after the closing paren.
    fn write_call_args(&mut self, node: &ruby_prism::CallNode<'_>, parenthesized: bool) {
        let positional: Vec<_> = node
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let block_pass = match node.block() {
            Some(b @ ruby_prism::Node::BlockArgumentNode { .. }) => Some(b),
            _ => None,
        };
        self.write_arg_list(parenthesized, positional, block_pass);
    }

    /// Shared argument-list renderer for calls, `super`, and `yield`.
    ///
    /// Parenthesized lists expand all-or-nothing when any argument
    /// hard-breaks *or* captures as multi-line (a nested array/hash/call
    /// that wrapped internally). Capturing those as `Doc::text` would leave
    /// sibling arguments on the same line as the broken nested collection.
    /// Keyword pairs are flattened out of Prism's `KeywordHashNode` so
    /// `foo(a, k: 1, j: 2)` wraps one pair per line, not as one opaque
    /// leaf. Parenthesized lists that still fit go through the Doc wrap
    /// path.
    ///
    /// Bare command calls stay unparenthesized. When any argument is
    /// already multi-line, or the flat `a, b, c` form would pass
    /// `line_length`, siblings move to their own line at one indent with
    /// the first argument remaining on the call line. That covers
    /// Layout/MultilineMethodArgumentLineBreaks and Layout/LineLength
    /// for command style without adding parentheses (a single argument
    /// that itself exceeds the limit still cannot wrap).
    fn write_arg_list<'a>(
        &mut self,
        parenthesized: bool,
        mut items: Vec<ruby_prism::Node<'a>>,
        block_pass: Option<ruby_prism::Node<'a>>,
    ) {
        if items.is_empty() && block_pass.is_none() && !parenthesized {
            return;
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

        let has_args = !items.is_empty();
        if parenthesized && has_args {
            self.write_bracketed_items(
                "(",
                pretty::RcDoc::line_(),
                &items,
                pretty::RcDoc::line_(),
                ")",
            );
            return;
        }

        if !parenthesized && items.len() > 1 && !self.single_line {
            let snap = self.capture_snapshot();
            let captured: Vec<String> =
                items.iter().map(|item| self.capture_inline(item)).collect();
            if captured.iter().any(|s| s.contains('\n')) || self.bare_args_overflow(&captured) {
                self.restore_capture_snapshot(snap);
                self.write_expanded_bare_args(&items);
                return;
            }
            self.output.push(' ');
            for (i, arg) in captured.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.output.push_str(arg);
            }
            return;
        }

        self.output.push(if parenthesized { '(' } else { ' ' });
        for (i, arg) in items.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(arg);
        }
        if parenthesized {
            self.output.push(')');
        }
    }

    /// Renders a call's real block (`{ }` / `do...end`), if it has one —
    /// as opposed to a block-pass argument, which write_call_args handles.
    fn write_call_block(&mut self, node: &ruby_prism::CallNode<'_>) {
        if let Some(block @ ruby_prism::Node::BlockNode { .. }) = node.block() {
            self.visit(&block);
        }
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

    /// Emits a heredoc: the opening tag goes here, inline, but the body is
    /// deferred.
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
    fn write_heredoc(&mut self, opening: &[u8], body: String, body_end: usize) {
        let terminator_line = self.line_of(body_end.saturating_sub(1));
        self.heredoc_end_line = Some(
            self.heredoc_end_line
                .map_or(terminator_line, |seen| seen.max(terminator_line)),
        );
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.write_bytes(opening);
        let index = self.pending_heredocs.len();
        self.pending_heredocs.push(body);
        // A marker rather than an offset into `output`: argument lists are
        // rendered into a detached buffer by `capture_as_doc`, so an
        // offset taken there points into that temporary and not into the
        // final output. A marker travels with the text wherever it is
        // spliced.
        self.output.push(HEREDOC_MARK_START);
        self.output.push_str(&index.to_string());
        self.output.push(HEREDOC_MARK_END);
        if !inline {
            self.output.push('\n');
        }
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
        let location = node.location();
        let (start, end) = (location.start_offset(), location.end_offset());
        let text = self.source.get(start..end).unwrap_or_default().to_string();
        while self.comment_cursor < self.comments.len()
            && (start..end).contains(&self.comments[self.comment_cursor].start)
        {
            self.comment_cursor += 1;
        }
        self.write_leaf(|f| f.output.push_str(&text));
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
}
