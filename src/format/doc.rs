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
//! body of this module for our own printer is a change to one file (issue
//! #24). It also keeps the surface honest — everything the formatter is
//! allowed to express is listed here.

use pretty::RcDoc;

/// A document: a tree of text and line points that renders to a string
/// once a width is chosen.
#[derive(Clone)]
pub(super) struct Doc {
    doc: RcDoc<'static, ()>,
    /// Whether this document is already multi-line no matter how much room
    /// it is given -- see [`Doc::breaks`].
    breaks: bool,
}

impl Doc {
    fn new(doc: RcDoc<'static, ()>, breaks: bool) -> Self {
        Doc { doc, breaks }
    }

    /// Whether this document occupies more than one line whatever width it
    /// is rendered at: it contains a [`hardline`](Doc::hardline), or a
    /// `text` leaf with a newline baked into it (captured output from a
    /// construct that has not moved onto `Doc` yet).
    ///
    /// A collection asks this of its items to decide between its flat and
    /// its expanded shape. It has to be asked rather than left to the
    /// printer because the two cases differ: `hardline` does force the
    /// enclosing group open, but a newline inside a `text` leaf does not --
    /// the printer measures that leaf as one long line and can lay the
    /// group out flat around content that is already broken.
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
    /// Text is meant to be a single line. A `\n` inside it still renders,
    /// but it is measured as if it were one long line and the printer's
    /// indentation does not apply to it, so a group containing one can be
    /// laid out as though it fit. Today that is how captured string output
    /// gets embedded in a `Doc`; [`Doc::breaks`] reports the case so the
    /// enclosing construct can pick its expanded shape by hand, and both go
    /// away with the last capture (issues #22, #23).
    pub(super) fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        let breaks = text.contains('\n');
        Doc::new(RcDoc::text(text), breaks)
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
    /// multi-line (a `do...end` body, a statement list) reports that fact
    /// to the collection it sits inside, instead of the caller testing for
    /// it up front.
    ///
    /// Unused so far: it is what replaces `forces_list_break` once bodies
    /// build `Doc`s of their own (issue #22).
    #[allow(dead_code)]
    pub(super) fn hardline() -> Self {
        Doc::new(RcDoc::hardline(), true)
    }

    /// `broken` when the enclosing group breaks, `flat` when it fits —
    /// Prettier's `ifBreak`. A trailing comma that only appears in the
    /// expanded form is the usual case (Tuxedo does not emit one, but the
    /// primitive is what `line`/`softline` are special cases of).
    ///
    /// Unused so far: kept because it is the general form `line` and
    /// `softline` are special cases of, and the vocabulary is the point of
    /// this module.
    #[allow(dead_code)]
    pub(super) fn if_break(broken: Doc, flat: Doc) -> Self {
        Doc::new(broken.doc.flat_alt(flat.doc), broken.breaks && flat.breaks)
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

    /// Lays the document out for a line `width` columns wide.
    pub(super) fn render(&self, width: usize) -> String {
        self.doc.pretty(width).to_string()
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

    /// Captured output from a construct that has not moved onto `Doc` yet
    /// arrives as one `text` leaf with newlines in it. The printer measures
    /// that as a single long line and lays the group out flat around it, so
    /// only `breaks` can see that the result is multi-line.
    #[test]
    fn breaks_reports_a_newline_baked_into_a_text_leaf() {
        let captured = || Doc::text("if x\n  y\nend");
        assert!(captured().breaks());
        assert!(list(&["1", "2"]).append(captured()).breaks());
        assert_eq!(
            Doc::text("[")
                .append(captured())
                .append(Doc::text("]"))
                .group()
                .render(80),
            "[if x\n  y\nend]"
        );
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
