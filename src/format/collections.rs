use super::doc::Doc;
use super::{Formatter, bytes_doc, is_indivisible};
use crate::config::HashBraceSpace;

impl Formatter {
    pub(super) fn fmt_splat_node(&mut self, node: &ruby_prism::SplatNode<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, "*");
        if let Some(expression) = node.expression() {
            self.run_node(&mut run, &expression);
        }
        self.end_run(run)
    }

    pub(super) fn fmt_block_argument_node(
        &mut self,
        node: &ruby_prism::BlockArgumentNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, "&");
        if let Some(expression) = node.expression() {
            self.run_node(&mut run, &expression);
        }
        self.end_run(run)
    }

    pub(super) fn fmt_array_node(&mut self, node: &ruby_prism::ArrayNode<'_>) -> Doc {
        let opening = node.opening_loc().map(|loc| loc.as_slice());
        let elements: Vec<_> = node.elements().iter().collect();

        if opening.is_some_and(|o| o.len() >= 2 && matches!(&o[..2], b"%w" | b"%i" | b"%W" | b"%I"))
        {
            return self.fmt_percent_array(node, &elements);
        }

        let bracketed = opening.is_some();

        // An empty bracketed array has nothing to expand.
        if bracketed && elements.is_empty() {
            return Doc::text("[]");
        }

        // An array with no brackets of its own -- the right-hand side of
        // `a, b = 1, 2` -- has no bracket to expand *into*. When the last
        // value has somewhere to break inside itself (a call, a hash, a
        // collection), that is where the line's own room to give is, and
        // it stays where it is and breaks in place, the way any assigned
        // value does. Only when the last value is indivisible -- nothing
        // in it for a break to land on, the way a bare string or number
        // never breaks -- is the comma-joined list itself given somewhere
        // to break: the same all-or-nothing group a bare command argument
        // list already gets (`bare_args_doc`), first value on the line it
        // started, every later one at one indent.
        if !bracketed {
            let last = elements.len().saturating_sub(1);
            if elements.len() > 1
                && !self.single_line
                && elements.last().is_some_and(is_indivisible)
            {
                return self.bare_args_doc(&elements);
            }
            let mut run = self.begin_run();
            for (i, element) in elements.iter().enumerate() {
                if i > 0 {
                    self.run_text(&mut run, ", ");
                }
                // A value that isn't the last shares its line with
                // whatever follows it, and that tail is the comma-joined
                // run's to shorten, not this element's -- so a break
                // inside a non-last value never buys the line anything,
                // only a needless reflow of an otherwise-fine expression.
                let doc = self.node_doc(element);
                let doc = if i < last { doc.flat() } else { doc };
                run.leaves.push(doc);
            }
            return self.end_run(run);
        }

        let span = node
            .opening_loc()
            .zip(node.closing_loc())
            .map(|(open, close)| (open.end_offset(), close.start_offset()));
        let was = self.array_keyword_hash;
        self.array_keyword_hash = elements
            .last()
            .filter(|last| last.as_keyword_hash_node().is_some())
            .map(|last| last.location().start_offset());
        let doc =
            self.bracketed_items_doc("[", Doc::softline(), &elements, Doc::softline(), "]", span);
        self.array_keyword_hash = was;
        doc
    }

    /// Renders a `%w()`/`%i()`/`%W()`/`%I()` word/symbol list, preserving
    /// its original percent-literal syntax (and whichever delimiter
    /// characters it used) rather than expanding it into a bracketed,
    /// comma-separated array of quoted strings/symbols.
    fn fmt_percent_array(
        &mut self,
        node: &ruby_prism::ArrayNode<'_>,
        elements: &[ruby_prism::Node<'_>],
    ) -> Doc {
        let opening = node.opening_loc().unwrap().as_slice().to_vec();
        let closing = node.closing_loc().unwrap().as_slice().to_vec();

        if elements.is_empty() {
            return bytes_doc(&opening).append(bytes_doc(&closing));
        }

        let leaves: Vec<_> = elements
            .iter()
            .map(|element| self.fmt_percent_array_word(element))
            .collect();

        let opening = String::from_utf8_lossy(&opening).into_owned();
        let closing = String::from_utf8_lossy(&closing).into_owned();
        let body = Doc::join(leaves, Doc::line());
        self.bracketed_group_doc(&opening, Doc::softline(), body, Doc::softline(), &closing)
    }

    /// Renders one element of a `%w`/`%i`/`%W`/`%I` list as its bare word,
    /// with no quotes/colon (those delimiters don't exist for an
    /// individual word in this syntax — only the list as a whole has
    /// opening/closing punctuation).
    fn fmt_percent_array_word(&mut self, element: &ruby_prism::Node<'_>) -> Doc {
        if let Some(interpolated) = element.as_interpolated_string_node() {
            let parts = interpolated.parts();
            self.interpolated_literal_doc(None, &parts, None)
        } else if let Some(interpolated) = element.as_interpolated_symbol_node() {
            let parts = interpolated.parts();
            self.interpolated_literal_doc(None, &parts, None)
        } else {
            // A plain word (StringNode for %w/%W, SymbolNode for %i/%I) has
            // no delimiters in source at all -- its raw location is
            // already exactly the bare word. Copying it verbatim (rather
            // than through `unescaped()`) also preserves an escaped space
            // like `\ ` as-is instead of resolving it to an actual space,
            // which would silently merge two words back into one.
            bytes_doc(element.location().as_slice())
        }
    }

    pub(super) fn fmt_range_node(&mut self, node: &ruby_prism::RangeNode<'_>) -> Doc {
        let mut run = self.begin_run();
        if let Some(left) = node.left() {
            self.run_node(&mut run, &left);
        }
        self.run_bytes(&mut run, node.operator_loc().as_slice());
        if let Some(right) = node.right() {
            self.run_node(&mut run, &right);
        }
        self.end_run(run)
    }

    pub(super) fn fmt_hash_node(&mut self, node: &ruby_prism::HashNode<'_>) -> Doc {
        let elements: Vec<_> = node.elements().iter().collect();
        let braces = (
            node.opening_loc().end_offset(),
            node.closing_loc().start_offset(),
        );
        self.fmt_hash_elements(&elements, Some(braces))
    }

    /// Bare keyword arguments at a call site (`foo(a, k: 1, j: 2)`). Prism
    /// parses these as a `KeywordHashNode`, *not* a `HashNode` — they are
    /// the same comma-joined pairs but carry no braces of their own.
    pub(super) fn fmt_keyword_hash_node(&mut self, node: &ruby_prism::KeywordHashNode<'_>) -> Doc {
        let elements: Vec<_> = node.elements().iter().collect();
        if self.array_keyword_hash == Some(node.location().start_offset()) {
            return self.array_element_pairs_doc(&elements);
        }
        self.fmt_hash_elements(&elements, None)
    }

    /// The pairs of a bare keyword hash written inside `[ ]`: one array
    /// element made of several pairs, laid out with no indent of its own so
    /// that when the array expands they all sit at the element indent
    /// instead of the first one alone.
    fn array_element_pairs_doc(&mut self, elements: &[ruby_prism::Node<'_>]) -> Doc {
        let snap = self.render_snapshot();
        let leaves: Vec<Doc> = elements.iter().map(|e| self.node_doc(e)).collect();
        if leaves.iter().any(Doc::breaks) {
            self.restore_render_snapshot(snap);
            return self.expanded_items_doc(elements);
        }
        Doc::join(leaves, Doc::text(",").append(Doc::line())).group()
    }

    /// Renders a run of `key => value` / `key: value` pairs, optionally
    /// wrapped in `{ }`. Shared by braced hash literals and bare keyword
    /// argument lists so the two can never drift apart on separators.
    ///
    /// `braces` is the source range between the braces of a braced hash
    /// (`None` for bare keyword arguments).
    fn fmt_hash_elements(
        &mut self,
        elements: &[ruby_prism::Node<'_>],
        braces: Option<(usize, usize)>,
    ) -> Doc {
        let braced = braces.is_some();
        if elements.is_empty() {
            return if braced { Doc::text("{}") } else { Doc::nil() };
        }

        if braced {
            let inner = match self.hash_brace_space {
                HashBraceSpace::Space => Doc::line,
                HashBraceSpace::NoSpace => Doc::softline,
            };
            return self.bracketed_items_doc("{", inner(), elements, inner(), "}", braces);
        }

        // Bare keyword args at a call site with no braces of their own.
        // Call args flatten KeywordHashNode before wrapping, so this is
        // the fallback if a keyword hash is visited directly. With no
        // bracket to break after, the first pair stays on the line the
        // call is already on.
        let snap = self.render_snapshot();
        let leaves: Vec<Doc> = elements.iter().map(|e| self.node_doc(e)).collect();
        if leaves.iter().any(Doc::breaks) {
            self.restore_render_snapshot(snap);
            return self.expanded_items_doc(elements);
        }
        let body = Doc::join(leaves, Doc::text(",").append(Doc::line()));
        body.indent(self.indent_str.len()).group()
    }

    /// A double-splat entry inside a hash literal or keyword argument list
    /// (`{ a: 1, **rest }`, `foo(**opts)`). The bare `**` form used to
    /// forward an anonymous kwrest has no value.
    pub(super) fn fmt_assoc_splat_node(&mut self, node: &ruby_prism::AssocSplatNode<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_bytes(&mut run, node.operator_loc().as_slice());
        if let Some(value) = node.value() {
            self.run_node(&mut run, &value);
        }
        self.end_run(run)
    }

    pub(super) fn fmt_assoc_node(&mut self, node: &ruby_prism::AssocNode<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, &node.key());
        let value = node.value();
        // Shorthand `key:` (3.1+ hash-literal value-omission, or a pattern
        // -match bind) has an `ImplicitNode` standing in for the omitted
        // value -- the key's own trailing `:` already says everything, so
        // there's nothing more to print. Printing it anyway would silently
        // rewrite `{ x: }` into `{ x: x }`, or worse, turn a pattern-match
        // bind into a match against an existing variable's current value.
        if matches!(value, ruby_prism::Node::ImplicitNode { .. }) {
            return self.end_run(run);
        }
        // `key => value` keeps whichever operator the source wrote;
        // `key: value` has none of its own -- the `:` is part of the key.
        self.run_text(&mut run, " ");
        if let Some(op) = node.operator_loc() {
            self.run_bytes(&mut run, op.as_slice());
            self.run_text(&mut run, " ");
        }
        self.run_node(&mut run, &value);
        self.end_run(run)
    }

    /// `{ key: value, ... }` / `ClassName[key: value, ...]` as a pattern
    /// (as opposed to `fmt_hash_node`'s hash *literal*) -- e.g. the
    /// right-hand side of `expr => { key: value }` or `expr in
    /// ClassName[key:]`. Kept as a flat one-line shape rather than a
    /// breakable group like the literal: patterns are short in practice
    /// and rarely need to wrap.
    pub(super) fn fmt_hash_pattern_node(&mut self, node: &ruby_prism::HashPatternNode<'_>) -> Doc {
        let mut run = self.begin_run();
        if let Some(constant) = node.constant() {
            self.run_node(&mut run, &constant);
        }

        let opening = node
            .opening_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"{".to_vec());
        let closing = node
            .closing_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"}".to_vec());
        let braced = opening == b"{";

        self.run_bytes(&mut run, &opening);
        let elements: Vec<_> = node.elements().iter().collect();
        let has_content = !elements.is_empty() || node.rest().is_some();
        if braced && has_content {
            self.run_text(&mut run, " ");
        }
        for (i, element) in elements.iter().enumerate() {
            if i > 0 {
                self.run_text(&mut run, ", ");
            }
            self.run_node(&mut run, element);
        }
        if let Some(rest) = node.rest() {
            if !elements.is_empty() {
                self.run_text(&mut run, ", ");
            }
            self.run_node(&mut run, &rest);
        }
        if braced && has_content {
            self.run_text(&mut run, " ");
        }
        self.run_bytes(&mut run, &closing);
        self.end_run(run)
    }

    /// `[a, b, *rest, c]` / `ClassName[a, b]` as a pattern (as opposed to
    /// `fmt_array_node`'s array *literal*). Same rationale as
    /// `fmt_hash_pattern_node` for staying one flat line.
    pub(super) fn fmt_array_pattern_node(
        &mut self,
        node: &ruby_prism::ArrayPatternNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        if let Some(constant) = node.constant() {
            self.run_node(&mut run, &constant);
        }

        let opening = node
            .opening_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"[".to_vec());
        let closing = node
            .closing_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"]".to_vec());
        self.run_bytes(&mut run, &opening);

        let bare_comma = node
            .rest()
            .is_some_and(|rest| rest.as_implicit_rest_node().is_some());
        let items: Vec<_> = node
            .requireds()
            .iter()
            .chain(node.rest().filter(|_| !bare_comma))
            .chain(node.posts().iter())
            .collect();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.run_text(&mut run, ", ");
            }
            self.run_node(&mut run, item);
        }
        // `in [0, 1,]` discards the rest with a comma and nothing after it.
        if bare_comma {
            self.run_text(&mut run, ",");
        }

        self.run_bytes(&mut run, &closing);
        self.end_run(run)
    }

    /// The omitted value of a shorthand `key:` entry (hash-literal
    /// value-omission or a pattern-match bind) -- see the comment in
    /// `fmt_assoc_node`, which returns before ever visiting this node in
    /// practice. Renders nothing; kept only so an unexpected direct visit
    /// doesn't fall through to prism's default (which would print the
    /// implied variable's read/target, defeating the point).
    pub(super) fn fmt_implicit_node(&mut self, _node: &ruby_prism::ImplicitNode<'_>) -> Doc {
        Doc::nil()
    }
}
