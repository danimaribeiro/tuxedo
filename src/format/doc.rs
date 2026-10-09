//! The document IR the layout engine works in, and the only module in the
//! crate that names `pretty`.
//!
//! A `Doc` describes *possible* layouts rather than one fixed string: the
//! breaks in it are decided when the document is rendered against a width.
//! That is what buys Tuxedo's all-or-nothing rule (see the spec in
//! CLAUDE.md) — a `group` either fits flat or breaks at every one of its
//! own line points, never at some of them, so adding an element to a list
//! can only ever expand the whole list, not reflow its neighbours.
//!
//! The vocabulary is Prettier's, not `pretty`'s:
//!
//! | here                | flat            | broken          |
//! |---------------------|-----------------|-----------------|
//! | [`Doc::line`]       | `" "`           | newline         |
//! | [`Doc::softline`]   | `""`            | newline         |
//! | [`Doc::hardline`]   | newline (and forces every enclosing group to break) |
//! | [`Doc::if_break`]   | its flat arm    | its broken arm  |
//!
//! Wrapping `pretty` rather than using it directly is what makes the
//! printer replaceable: callers name none of its types, so swapping the
//! body of this module for our own printer is a change to one file. It
//! also keeps the surface honest — everything the formatter is allowed to
//! express is listed here.

use pretty::RcDoc;
use std::cell::Cell;
use unicode_width::UnicodeWidthStr;

thread_local! {
    /// The width the document being rendered is laid out against, for
    /// [`Doc::fits_or`], which has to ask the question while it renders.
    static RENDER_WIDTH: Cell<usize> = const { Cell::new(UNBOUNDED_WIDTH) };
}

/// The width a document is laid out against when nothing in it is allowed
/// to break (see [`Doc::flat`]). Not `usize::MAX`: the printer adds the
/// column the document starts at to it.
const UNBOUNDED_WIDTH: usize = usize::MAX / 2;

/// Marks a line that exists only to be blank. The printer writes the
/// indent in force at every break, including the one that opens an empty
/// line, so a blank line between two statements would come out as a line
/// of spaces. [`Doc::render`] strips this marker and the indent in front
/// of it.
///
/// A marker rather than trimming every line: a string literal and a
/// heredoc body are content, and trailing spaces inside them are the
/// program's, not the formatter's.
const BLANK_MARK: char = '\u{E002}';

/// A document: a tree of text and line points that renders to a string
/// once a width is chosen.
#[derive(Clone)]
pub(super) struct Doc {
    doc: RcDoc<'static, ()>,
    /// Whether this document is multi-line however much room it is given
    /// -- see [`Doc::breaks`].
    breaks: bool,
}

impl Doc {
    fn new(doc: RcDoc<'static, ()>, breaks: bool) -> Self {
        Doc { doc, breaks }
    }

    /// Whether this document occupies more than one line whatever width it
    /// is rendered at: a [`hardline`](Doc::hardline) sits somewhere inside
    /// it.
    ///
    /// The printer does not need to be told -- a hardline forces every
    /// group around it open by itself. What needs to be told is the
    /// formatter, where a construct picks between two *different*
    /// documents rather than between two layouts of one: a bracketed list
    /// with an already-broken item is built in its expanded shape, and a
    /// `{ }` block whose body breaks is built as `do...end` would be.
    pub(super) fn breaks(&self) -> bool {
        self.breaks
    }

    /// The empty document.
    pub(super) fn nil() -> Self {
        Doc::new(RcDoc::nil(), false)
    }

    /// Literal text, measured by display width (not bytes) — so a wide
    /// CJK character or an emoji counts for the columns it actually takes
    /// up on screen.
    ///
    /// Text is one line. A `\n` inside it still renders, but it is
    /// measured as if it were one long line and the printer's indentation
    /// does not apply to it, so a group containing one can be laid out as
    /// though it fit. The formatter puts newlines in text in exactly two
    /// places, both of them content rather than layout: a heredoc body and
    /// a `__END__` section. [`Doc::breaks`] reports them anyway, since a
    /// list holding one has no flat shape either way.
    pub(super) fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        let breaks = text.contains('\n');
        Doc::new(RcDoc::text(text), breaks)
    }

    /// The document laid out now, on one line, and handed back as a single
    /// `text` leaf.
    ///
    /// For the two places where a break would be legal Ruby but would land
    /// inside something whose line structure is not the printer's to
    /// decide: the content of a `#{}` hole, and a body being joined with
    /// `; ` inside `( )`.
    pub(super) fn flat(self) -> Self {
        Doc::text(self.render(UNBOUNDED_WIDTH))
    }

    /// A break, an empty line, and a second break: what goes between two
    /// statements the author left a gap between, or that a
    /// Layout/EmptyLine* cop wants separated.
    pub(super) fn blank_line() -> Self {
        Doc::hardline()
            .append(Doc::text(BLANK_MARK.to_string()))
            .append(Doc::hardline())
    }

    /// A line point that is a single space when its group fits flat.
    pub(super) fn line() -> Self {
        Doc::new(RcDoc::line(), false)
    }

    /// A line point that is nothing at all when its group fits flat —
    /// where a break is allowed but no separator is wanted, such as just
    /// inside a bracket.
    pub(super) fn softline() -> Self {
        Doc::new(RcDoc::line_(), false)
    }

    /// An unconditional line break, which also forces every group
    /// containing it to break. This is how a construct that is always
    /// multi-line -- an expanded collection -- reports that fact to
    /// whatever it sits inside, instead of the caller testing for it up
    /// front.
    pub(super) fn hardline() -> Self {
        Doc::new(RcDoc::hardline(), true)
    }

    /// `broken` when the enclosing group breaks, `flat` when it fits —
    /// Prettier's `ifBreak`, and the general form `line` and `softline`
    /// are special cases of. The trailing `\` an adjacent-string-literal
    /// concatenation needs only in its wrapped shape is the case that uses
    /// it (see `implicit_string_concat_doc`); a trailing comma would be
    /// the other usual one, but Tuxedo does not emit those.
    pub(super) fn if_break(broken: Doc, flat: Doc) -> Self {
        Doc::new(broken.doc.flat_alt(flat.doc), broken.breaks && flat.breaks)
    }

    /// `fits` if `head` -- the text that would follow on the line this
    /// document starts at -- still ends within the width, `otherwise` if
    /// not.
    ///
    /// For a choice between two different documents that a group cannot
    /// make, because the part of the document that differs contains a
    /// hardline and a group holding one never fits: `head` is the
    /// hardline-free prefix, and the question asked is the one the group
    /// would have asked of it.
    pub(super) fn fits_or(head: &str, fits: Doc, otherwise: Doc) -> Self {
        let head_width = head.width();
        let breaks = fits.breaks || otherwise.breaks;
        Doc::new(
            RcDoc::column(move |pos| {
                if pos + head_width <= RENDER_WIDTH.with(Cell::get) {
                    fits.doc.clone()
                } else {
                    otherwise.doc.clone()
                }
            }),
            breaks,
        )
    }

    /// `stay` while `head` -- the text that would follow on the line this
    /// document starts at -- fits within the width, `moved` when it does not.
    ///
    /// [`Doc::fits_or`] for a `moved` shape that starts with a hardline and
    /// is only ever chosen on a line that is too long: it must not make the
    /// document report [`Doc::breaks`], which would tell every enclosing
    /// construct there is no flat shape when there is one.
    pub(super) fn stay_or_move(head: &str, stay: Doc, moved: Doc) -> Self {
        let head_width = head.width();
        let breaks = stay.breaks;
        Doc::new(
            RcDoc::column(move |pos| {
                if pos + head_width <= RENDER_WIDTH.with(Cell::get) {
                    stay.doc.clone()
                } else {
                    moved.doc.clone()
                }
            }),
            breaks,
        )
    }

    /// `fits` if `tail` fits on a line that starts at the indent in force
    /// where this document begins, `otherwise` if not.
    ///
    /// For text that will follow a closing bracket when the bracketed
    /// list before it expands: the closer goes back to that indent, so
    /// whether the text fits after it does not depend on how long the
    /// list's elements are.
    pub(super) fn fits_at_indent_or(tail: &str, fits: Doc, otherwise: Doc) -> Self {
        let tail_width = tail.width();
        let breaks = fits.breaks || otherwise.breaks;
        Doc::new(
            RcDoc::nesting(move |indent| {
                if indent + tail_width <= RENDER_WIDTH.with(Cell::get) {
                    fits.doc.clone()
                } else {
                    otherwise.doc.clone()
                }
            }),
            breaks,
        )
    }

    /// The documents one after another.
    pub(super) fn concat(docs: impl IntoIterator<Item = Doc>) -> Self {
        docs.into_iter().fold(Doc::nil(), Doc::append)
    }

    /// The documents one after another with `separator` between each pair.
    pub(super) fn join(docs: impl IntoIterator<Item = Doc>, separator: Doc) -> Self {
        let mut out = Doc::nil();
        for (i, doc) in docs.into_iter().enumerate() {
            if i > 0 {
                out = out.append(separator.clone());
            }
            out = out.append(doc);
        }
        out
    }

    /// `other` directly after `self`.
    pub(super) fn append(self, other: Doc) -> Self {
        Doc::new(self.doc.append(other.doc), self.breaks || other.breaks)
    }

    /// Marks `self` as a unit that is laid out flat if it fits on the rest
    /// of the line, and otherwise breaks at *every* line point directly
    /// inside it. Line points inside a nested group are decided
    /// separately, by that group.
    pub(super) fn group(self) -> Self {
        Doc::new(self.doc.group(), self.breaks)
    }

    /// Indents the lines that `self`'s own line points introduce by
    /// `spaces` columns. Only breaks *inside* `self` are affected, and the
    /// indent is relative to the one already in effect.
    pub(super) fn indent(self, spaces: usize) -> Self {
        Doc::new(self.doc.nest(spaces as isize), self.breaks)
    }

    /// Outdents the lines that `self`'s own line points introduce by
    /// `spaces` columns -- the inverse of [`Doc::indent`], for the one
    /// thing that sits a level out from the body it is written inside: a
    /// comment the author aligned with the `else`/`when`/`rescue` below
    /// it.
    pub(super) fn dedent(self, spaces: usize) -> Self {
        Doc::new(self.doc.nest(-(spaces as isize)), self.breaks)
    }

    /// Lays the document out for a line `width` columns wide.
    pub(super) fn render(&self, width: usize) -> String {
        let outer = RENDER_WIDTH.with(|w| w.replace(width));
        let rendered = self.doc.pretty(width).to_string();
        RENDER_WIDTH.with(|w| w.set(outer));
        if !rendered.contains(BLANK_MARK) {
            return rendered;
        }
        let mut out = String::with_capacity(rendered.len());
        for line in rendered.split_inclusive('\n') {
            match line.find(BLANK_MARK) {
                // Everything before the marker on that line is indent the
                // printer wrote into an otherwise empty line.
                Some(_) => out.push_str(line.trim_start_matches([' ', '\t', BLANK_MARK])),
                None => out.push_str(line),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::Doc;

    /// `[a, b]` as the formatter builds a list: flat if it fits, otherwise
    /// one element per line with the brackets alone on theirs.
    fn list(items: &[&str]) -> Doc {
        let body = Doc::join(
            items.iter().map(|i| Doc::text(*i)),
            Doc::text(",").append(Doc::line()),
        );
        Doc::text("[")
            .append(Doc::softline().append(body).indent(2))
            .append(Doc::softline().append(Doc::text("]")))
            .group()
    }

    #[test]
    fn a_group_that_fits_stays_flat() {
        assert_eq!(list(&["1", "2"]).render(80), "[1, 2]");
    }

    #[test]
    fn a_group_that_does_not_fit_breaks_at_every_line_point() {
        assert_eq!(list(&["1", "2"]).render(4), "[\n  1,\n  2\n]");
    }

    #[test]
    fn a_hardline_breaks_the_enclosing_group_even_when_it_would_fit() {
        let doc = Doc::text("[")
            .append(Doc::softline().append(Doc::text("x")).indent(2))
            .append(Doc::hardline())
            .append(Doc::text("]"))
            .group();
        assert_eq!(doc.render(80), "[\n  x\n]");
    }

    #[test]
    fn if_break_picks_its_arm_from_the_enclosing_group() {
        let doc = |width| {
            Doc::text("(")
                .append(Doc::softline().append(Doc::text("x")).indent(2))
                .append(Doc::if_break(Doc::text(","), Doc::nil()))
                .append(Doc::softline().append(Doc::text(")")))
                .group()
                .render(width)
        };
        assert_eq!(doc(80), "(x)");
        assert_eq!(doc(2), "(\n  x,\n)");
    }

    /// `日本語 x` is 8 columns on screen and 11 bytes. Measuring it by
    /// byte length would break it at a width it actually fits in.
    #[test]
    fn text_is_measured_by_display_width_not_bytes() {
        let doc = |width| {
            Doc::text("日本語")
                .append(Doc::line())
                .append(Doc::text("x"))
                .group()
                .render(width)
        };
        assert_eq!(doc(8), "日本語 x");
        assert_eq!(doc(7), "日本語\nx");
    }

    #[test]
    fn breaks_reports_a_hardline_anywhere_inside() {
        assert!(!list(&["1", "2"]).breaks());
        assert!(
            Doc::text("a")
                .append(Doc::line())
                .append(Doc::hardline())
                .group()
                .indent(2)
                .breaks()
        );
    }

    /// A heredoc body is content, not layout: it arrives as one `text`
    /// leaf with newlines in it, the printer measures it as a single long
    /// line, and `breaks` is what tells a list holding one that it has no
    /// flat shape.
    #[test]
    fn breaks_reports_a_newline_baked_into_a_text_leaf() {
        let body = || Doc::text("  SELECT 1\nSQL\n");
        assert!(body().breaks());
        assert!(list(&["1", "2"]).append(body()).breaks());
    }

    /// `flat` renders a document once, at whatever width it needs, and
    /// hands it back as a leaf -- for a `#{}` hole, whose enclosing
    /// literal owns the line structure.
    #[test]
    fn flat_lays_a_document_out_on_one_line() {
        assert_eq!(list(&["1", "2"]).flat().render(4), "[1, 2]");
    }

    #[test]
    fn dedent_puts_a_line_back_a_level() {
        let doc = Doc::text("body")
            .append(
                Doc::hardline()
                    .append(Doc::text("# about the else"))
                    .dedent(2),
            )
            .append(Doc::hardline())
            .append(Doc::text("else"))
            .indent(2);
        assert_eq!(doc.render(80), "body\n# about the else\n  else");
    }

    #[test]
    fn indent_is_relative_to_the_indent_already_in_effect() {
        let inner = Doc::text("a")
            .append(Doc::hardline())
            .append(Doc::text("b"));
        let doc = Doc::text("outer")
            .append(Doc::hardline().append(inner).indent(2))
            .indent(4);
        assert_eq!(doc.render(80), "outer\n      a\n      b");
    }
}
