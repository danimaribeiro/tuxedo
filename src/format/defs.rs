//! Method definitions, parameters, blocks and lambdas.
//!
//! A `def` with a body is always written out on its own lines, even a
//! one-liner; an endless `def x = y` stays endless while it fits and
//! expands to the block form when it does not. A `{ }` block stays on one
//! line while it fits and holds a single statement with no construct of
//! its own (`holds_block_construct`); otherwise it expands the way
//! `do...end` does. Parameter lists follow the all-or-nothing rule of every
//! bracketed list.

use super::Formatter;
use super::doc::Doc;
use ruby_prism::Visit;

/// Whether `node` holds a construct that has no one-line spelling without
/// semicolons: an `if`/`case`/`while`/`begin`/`def`/`class`, or a sequence
/// of statements. Flat, they come out as `if a; b; else; c; end`, which
/// RuboCop flags (`Style/IfWithSemicolon`) and which hides structure the
/// author gave lines to. A brace block holding one expands like
/// `do...end` instead.
///
/// Modifier forms (`a if b`) and ternaries are one-line spellings already
/// and do not count. A `def` does, endless or not: an endless one that does
/// not fit is written out with its `end`.
pub(super) fn holds_block_construct(node: &ruby_prism::Node<'_>) -> bool {
    struct Finder(bool);

    impl<'pr> Visit<'pr> for Finder {
        fn visit_if_node(&mut self, node: &ruby_prism::IfNode<'pr>) {
            self.0 |= node.end_keyword_loc().is_some();
            ruby_prism::visit_if_node(self, node);
        }
        fn visit_unless_node(&mut self, node: &ruby_prism::UnlessNode<'pr>) {
            self.0 |= node.end_keyword_loc().is_some();
            ruby_prism::visit_unless_node(self, node);
        }
        fn visit_while_node(&mut self, node: &ruby_prism::WhileNode<'pr>) {
            self.0 |= node.closing_loc().is_some();
            ruby_prism::visit_while_node(self, node);
        }
        fn visit_until_node(&mut self, node: &ruby_prism::UntilNode<'pr>) {
            self.0 |= node.closing_loc().is_some();
            ruby_prism::visit_until_node(self, node);
        }
        fn visit_begin_node(&mut self, node: &ruby_prism::BeginNode<'pr>) {
            self.0 |= node.end_keyword_loc().is_some();
            ruby_prism::visit_begin_node(self, node);
        }
        fn visit_def_node(&mut self, node: &ruby_prism::DefNode<'pr>) {
            self.0 = true;
            ruby_prism::visit_def_node(self, node);
        }
        fn visit_case_node(&mut self, node: &ruby_prism::CaseNode<'pr>) {
            self.0 = true;
            ruby_prism::visit_case_node(self, node);
        }
        fn visit_case_match_node(&mut self, node: &ruby_prism::CaseMatchNode<'pr>) {
            self.0 = true;
            ruby_prism::visit_case_match_node(self, node);
        }
        fn visit_for_node(&mut self, node: &ruby_prism::ForNode<'pr>) {
            self.0 = true;
            ruby_prism::visit_for_node(self, node);
        }
        fn visit_class_node(&mut self, node: &ruby_prism::ClassNode<'pr>) {
            self.0 = true;
            ruby_prism::visit_class_node(self, node);
        }
        fn visit_module_node(&mut self, node: &ruby_prism::ModuleNode<'pr>) {
            self.0 = true;
            ruby_prism::visit_module_node(self, node);
        }
        fn visit_singleton_class_node(&mut self, node: &ruby_prism::SingletonClassNode<'pr>) {
            self.0 = true;
            ruby_prism::visit_singleton_class_node(self, node);
        }
        fn visit_statements_node(&mut self, node: &ruby_prism::StatementsNode<'pr>) {
            self.0 |= node.body().iter().count() > 1;
            ruby_prism::visit_statements_node(self, node);
        }
    }

    let mut finder = Finder(false);
    finder.visit(node);
    finder.0
}

/// True if a `{ }` block inside `node` holds a block construct, and so
/// expands (see [`holds_block_construct`]) instead of joining its body
/// with semicolons.
pub(super) fn holds_expanding_brace_block(node: &ruby_prism::Node<'_>) -> bool {
    struct Finder(bool);

    impl<'pr> Visit<'pr> for Finder {
        fn visit_block_node(&mut self, node: &ruby_prism::BlockNode<'pr>) {
            self.0 |= node.opening_loc().as_slice() == b"{"
                && node.body().is_some_and(|b| holds_block_construct(&b));
            ruby_prism::visit_block_node(self, node);
        }
    }

    let mut finder = Finder(false);
    finder.visit(node);
    finder.0
}

impl Formatter {
    /// `def name(params)`, the body one level in, `end` back at the
    /// `def`'s level.
    ///
    /// Tuxedo always expands a method body, even a one-liner's: `def x; y;
    /// end` becomes three lines. An endless definition (`def x = y`) is the
    /// author's choice of construct and stays one when it fits; it expands
    /// to the block form when it does not. A block `def` is never made
    /// endless. The exception is `single_line`, where an
    /// enclosing construct owns the line and the whole definition has to
    /// fit on it -- `Class.new { def self.a; self; end }`.
    pub(super) fn fmt_def_node(&mut self, node: &ruby_prism::DefNode<'_>) -> Doc {
        // The name as written, not `node.name()`: `def ~@` and `def !@`
        // are interned as `~` and `!`, and there is no reason to respell
        // what the author wrote.
        let name = node.name_loc();
        let name_str = std::str::from_utf8(name.as_slice())
            .unwrap_or("")
            .to_string();

        let mut header = Doc::text("def ");
        if let Some(receiver) = node.receiver() {
            let op = node
                .operator_loc()
                .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
                .unwrap_or(".")
                .to_string();
            header = header
                .append(self.node_doc(&receiver))
                .append(Doc::text(op));
        }
        header = header.append(Doc::text(name_str));
        if let Some(params) = node.parameters() {
            let params = self.bracketed_parameters_doc("(", &params, ")");
            header = header.append(params);
        }

        if self.single_line {
            return match node.body() {
                Some(body) => header
                    .append(Doc::text("; "))
                    .append(self.node_doc(&body))
                    .append(Doc::text("; end")),
                None => header.append(Doc::text("; end")),
            };
        }

        // A trailing comment sits after wherever the signature actually
        // ends -- the closing paren when params wrapped onto their own
        // lines, not the `def` keyword's line, which may be several
        // lines earlier by then.
        let header_end = node
            .rparen_loc()
            .map(|l| l.start_offset())
            .unwrap_or_else(|| node.location().start_offset());
        let header_line = self.line_of(header_end);
        let end_offset = node.end_keyword_loc().map(|l| l.start_offset());

        if node.equal_loc().is_some()
            && let Some(body) = node.body()
            && !self.has_pending_comment_in(
                body.location().start_offset(),
                self.end_of_line_after(node.location().end_offset()),
            )
        {
            // Both shapes are built and the printer picks by width: the
            // endless one when it fits, the block form when it does not.
            // `breaks` on the flat build says there is no flat shape at all.
            let snap = self.render_snapshot();
            let was_single_line = std::mem::replace(&mut self.single_line, true);
            let flat = self.node_doc(&body);
            self.single_line = was_single_line;
            if !flat.breaks() {
                self.rewind_render_snapshot(snap);
                let block = self.keyword_body_doc(header.clone(), header_line, Some(body), None);
                let endless = header.append(Doc::text(" = ")).append(flat);
                return Doc::if_break(block, endless).group();
            }
            self.restore_render_snapshot(snap);
        }
        self.keyword_body_doc(header, header_line, node.body(), end_offset)
    }

    pub(super) fn fmt_required_parameter_node(
        &mut self,
        node: &ruby_prism::RequiredParameterNode<'_>,
    ) -> Doc {
        super::bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_optional_parameter_node(
        &mut self,
        node: &ruby_prism::OptionalParameterNode<'_>,
    ) -> Doc {
        let name = node.name();
        self.parameter_default_doc(name.as_slice(), " = ", &node.value())
    }

    pub(super) fn fmt_rest_parameter_node(
        &mut self,
        node: &ruby_prism::RestParameterNode<'_>,
    ) -> Doc {
        Self::sigil_parameter_doc("*", node.name())
    }

    /// The bare trailing comma of `|k,|` and `in [0, 1,]`: "discard the
    /// rest" with nothing to write for it. The list that owns it puts the
    /// comma after its last element, where a `", "` join could not.
    pub(super) fn fmt_implicit_rest_node(
        &mut self,
        _node: &ruby_prism::ImplicitRestNode<'_>,
    ) -> Doc {
        Doc::nil()
    }

    pub(super) fn fmt_required_keyword_parameter_node(
        &mut self,
        node: &ruby_prism::RequiredKeywordParameterNode<'_>,
    ) -> Doc {
        super::bytes_doc(node.name().as_slice()).append(Doc::text(":"))
    }

    pub(super) fn fmt_optional_keyword_parameter_node(
        &mut self,
        node: &ruby_prism::OptionalKeywordParameterNode<'_>,
    ) -> Doc {
        let name = node.name();
        self.parameter_default_doc(name.as_slice(), ": ", &node.value())
    }

    pub(super) fn fmt_keyword_rest_parameter_node(
        &mut self,
        node: &ruby_prism::KeywordRestParameterNode<'_>,
    ) -> Doc {
        Self::sigil_parameter_doc("**", node.name())
    }

    pub(super) fn fmt_no_keywords_parameter_node(
        &mut self,
        _node: &ruby_prism::NoKeywordsParameterNode<'_>,
    ) -> Doc {
        Doc::text("**nil")
    }

    pub(super) fn fmt_forwarding_parameter_node(
        &mut self,
        _node: &ruby_prism::ForwardingParameterNode<'_>,
    ) -> Doc {
        Doc::text("...")
    }

    pub(super) fn fmt_block_parameter_node(
        &mut self,
        node: &ruby_prism::BlockParameterNode<'_>,
    ) -> Doc {
        Self::sigil_parameter_doc("&", node.name())
    }

    /// `*rest`, `**opts`, `&block` -- a sigil and an optional name, which
    /// is absent for the anonymous forms (`def f(*)`, `def g(&)`).
    fn sigil_parameter_doc(sigil: &str, name: Option<ruby_prism::ConstantId<'_>>) -> Doc {
        let doc = Doc::text(sigil.to_string());
        match name {
            Some(name) => doc.append(super::bytes_doc(name.as_slice())),
            None => doc,
        }
    }

    /// `name = value` / `name: value` -- a parameter with a default. The
    /// value goes through a run so it measures the column its own name
    /// leaves it at, not the column the parameter list starts at.
    fn parameter_default_doc(
        &mut self,
        name: &[u8],
        operator: &str,
        value: &ruby_prism::Node<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_bytes(&mut run, name);
        self.run_text(&mut run, operator);
        self.run_node(&mut run, value);
        self.end_run(run)
    }

    /// Collects a parameter list's entries as a `Doc` per parameter, in
    /// declaration order (requireds, optionals, rest, posts, keywords,
    /// keyword-rest, block) -- shared by `fmt_parameters_node` (which joins
    /// them with no line point at all, for contexts that never wrap) and
    /// `bracketed_parameters_doc` (which wraps them all-or-nothing, block
    /// indented, like a call's argument list).
    fn parameter_leaves(&mut self, node: &ruby_prism::ParametersNode<'_>) -> Vec<Doc> {
        let mut leaves = Vec::new();
        for param in node.requireds().iter() {
            leaves.push(self.node_doc(&param));
        }
        for param in node.optionals().iter() {
            leaves.push(self.node_doc(&param));
        }
        if let Some(rest) = node.rest() {
            if rest.as_implicit_rest_node().is_some() {
                if let Some(last) = leaves.pop() {
                    leaves.push(last.append(Doc::text(",")));
                }
            } else {
                leaves.push(self.node_doc(&rest));
            }
        }
        for param in node.posts().iter() {
            leaves.push(self.node_doc(&param));
        }
        for param in node.keywords().iter() {
            leaves.push(self.node_doc(&param));
        }
        if let Some(keyword_rest) = node.keyword_rest() {
            leaves.push(self.node_doc(&keyword_rest));
        }
        if let Some(block) = node.block() {
            leaves.push(self.fmt_block_parameter_node(&block));
        }
        leaves
    }

    /// A parameter list with nothing around it, comma-joined and with no
    /// line point of its own: the pipe-delimited form (`|a, b|`), which is
    /// never worth wrapping, and the part before a block's locals.
    pub(super) fn fmt_parameters_node(&mut self, node: &ruby_prism::ParametersNode<'_>) -> Doc {
        Doc::join(self.parameter_leaves(node), Doc::text(", "))
    }

    /// A parameter list wrapped in `open`/`close` (a def's parens or a
    /// lambda's), all-or-nothing and block-indented like a call's argument
    /// list, so an overlong signature breaks one parameter per line instead
    /// of running past the line length.
    fn bracketed_parameters_doc(
        &mut self,
        open: &str,
        node: &ruby_prism::ParametersNode<'_>,
        close: &str,
    ) -> Doc {
        let leaves = self.parameter_leaves(node);
        let body = Doc::join(leaves, Doc::text(",").append(Doc::line()));
        self.bracketed_group_doc(open, Doc::softline(), body, Doc::softline(), close)
    }

    pub(super) fn fmt_block_parameters_node(
        &mut self,
        node: &ruby_prism::BlockParametersNode<'_>,
    ) -> Doc {
        // A lambda's parameters are parenthesized (`->(x) { }`) while a
        // block's are pipe-delimited (`{ |x| }`), and both are this node
        // type — so take the delimiters from source rather than assuming
        // pipes. A block's parameter list always has an explicit `|...|`
        // when it has any parameters at all -- `opening_loc`/`closing_loc`
        // come back `None` here only for a lambda's single bare parameter
        // written with no delimiter (`-> message do ... end`, valid Ruby
        // shorthand for `->(message) do ... end`), so parens is the only
        // correct default, not pipes.
        let opening = node
            .opening_loc()
            .map(|loc| String::from_utf8_lossy(loc.as_slice()).into_owned())
            .unwrap_or_else(|| "(".to_string());
        let closing = node
            .closing_loc()
            .map(|loc| String::from_utf8_lossy(loc.as_slice()).into_owned())
            .unwrap_or_else(|| ")".to_string());

        // Block-local variables (`|a; b, c|`): declared after a `;`, they
        // are part of the parameter list's text but not of `parameters()`.
        let locals: Vec<String> = node
            .locals()
            .iter()
            .map(|local| String::from_utf8_lossy(local.location().as_slice()).into_owned())
            .collect();
        if !locals.is_empty() {
            let params = match node.parameters() {
                Some(params) => self.fmt_parameters_node(&params),
                None => Doc::nil(),
            };
            return Doc::text(opening)
                .append(params)
                .append(Doc::text(format!("; {}{closing}", locals.join(", "))));
        }

        match node.parameters() {
            // Only a lambda's real parens are worth all-or-nothing wrapping
            // like a call's argument list. A block's `|params|` is never
            // meant to span multiple lines -- forcing a wrap there splits
            // the pipes across lines and leaves the block's body dangling
            // after the closing pipe on the same line as its last param.
            Some(params) if opening == "(" => {
                self.bracketed_parameters_doc(&opening, &params, &closing)
            }
            Some(params) => {
                let params = self.fmt_parameters_node(&params);
                Doc::text(opening).append(params).append(Doc::text(closing))
            }
            None => Doc::text(format!("{opening}{closing}")),
        }
    }

    /// A lambda literal (`->(x) { x + 1 }`, `-> { 42 }`, `->(x) do ... end`).
    ///
    /// Structurally a block with an arrow in front, so the body is built
    /// the same way `fmt_block_node` does it, but it is its own node type:
    /// the `->`, the parameter parentheses and the braces are all written
    /// here.
    pub(super) fn fmt_lambda_node(&mut self, node: &ruby_prism::LambdaNode<'_>) -> Doc {
        let brace = node.opening_loc().as_slice() == b"{";
        let opening = Doc::text(if brace { " {" } else { " do" });
        // The opening follows right after the parameter list on the
        // same line, so the two render as one document: an overlong
        // parameter list then wraps instead of pushing the line past
        // `line_length`, which it would if the opening were appended
        // after the list had already chosen its shape.
        let header = super::bytes_doc(node.operator_loc().as_slice()).append(
            match super::explicit_parameters(node.parameters()) {
                Some(parameters) => self.node_doc(&parameters).append(opening),
                None => opening,
            },
        );

        let (opening_end, closing_start) = (
            node.opening_loc().end_offset(),
            node.closing_loc().start_offset(),
        );
        if brace {
            let body = self.brace_block_body_doc(node.body(), false, opening_end, closing_start);
            return header.append(body);
        }
        let header_line = self.line_of(opening_end);
        self.keyword_body_doc(header, header_line, node.body(), Some(closing_start))
    }

    /// The header a block continues its call's line with: ` do` or ` {`,
    /// plus ` |params|` when the block declares any.
    ///
    /// A `Doc` rather than writes, so a caller can append it to the
    /// argument list's own group and have the printer measure the two
    /// together -- the width this takes is what decides whether those
    /// arguments still fit on the line. Block parameters never wrap (see
    /// `fmt_block_parameters_node`), so it is always one line.
    pub(super) fn block_header_doc(&mut self, node: &ruby_prism::BlockNode<'_>) -> Doc {
        let brace = node.opening_loc().as_slice() == b"{";
        let doc = Doc::text(if brace { " {" } else { " do" });
        match super::explicit_parameters(node.parameters()) {
            Some(params) => doc.append(Doc::text(" ")).append(self.node_doc(&params)),
            None => doc,
        }
    }

    pub(super) fn fmt_block_node(&mut self, node: &ruby_prism::BlockNode<'_>) -> Doc {
        let header = self.block_header_doc(node);
        let body = self.block_body_doc(node);
        header.append(body)
    }

    /// Everything after the header: a `{ }` body, or the indented
    /// statements and the `end` of a `do...end`.
    pub(super) fn block_body_doc(&mut self, node: &ruby_prism::BlockNode<'_>) -> Doc {
        let brace = node.opening_loc().as_slice() == b"{";
        let (opening_end, closing_start) = (
            node.opening_loc().end_offset(),
            node.closing_loc().start_offset(),
        );
        if brace {
            let has_parameters = super::explicit_parameters(node.parameters()).is_some();
            return self.brace_block_body_doc(
                node.body(),
                has_parameters,
                opening_end,
                closing_start,
            );
        }
        // `do |x| # why`: the header ends at the closing pipe when
        // there are parameters, at `do` otherwise.
        let header_end = node
            .parameters()
            .map_or(opening_end, |p| p.location().end_offset().max(opening_end));
        let header_line = self.line_of(header_end);
        self.keyword_body_doc(Doc::nil(), header_line, node.body(), Some(closing_start))
    }

    /// Everything a brace block puts *after* its `{`, which the header
    /// the call line ends with has already contributed: ` stmt }` when it
    /// fits on the line, otherwise the same expanded shape as `do...end`
    /// (body at +1 indent, `}` on its own line).
    ///
    /// A body with more than one statement never collapses onto one line,
    /// even when the flat form would fit -- joining statements with "; "
    /// inside `{ }` is what RuboCop's Style/Semicolon flags, and it
    /// silently collapses structure the source author chose to keep
    /// multi-line (e.g. a lambda body split across several lines).
    /// Confirmed against rails's action_controller/metal.rb, where a
    /// 4-line lambda body fit under line_length flat and so got squashed
    /// to one line before this check.
    ///
    /// A comment anywhere between the braces forces the expanded shape
    /// too: `{ # why` has no flat form that keeps the comment in place.
    fn brace_block_body_doc(
        &mut self,
        body: Option<ruby_prism::Node<'_>>,
        has_parameters: bool,
        opening_end: usize,
        closing_start: usize,
    ) -> Doc {
        let nest = self.indent_str.len();
        let has_comment =
            !self.single_line && self.has_pending_comment_in(opening_end, closing_start);

        let Some(body) = body else {
            if !has_comment {
                // `{}` when nothing sits between the braces, `{ |x| }` once
                // a parameter list does: the closing pipe is not a brace
                // and `|x|}` reads as one token.
                return Doc::text(if has_parameters { " }" } else { "}" });
            }
            let opening_comment = self.trailing_comment_doc(self.line_of(opening_end));
            self.indent += 1;
            let dangling = self.comments_before_doc(closing_start, false);
            self.indent -= 1;
            return opening_comment
                .unwrap_or_else(Doc::nil)
                .append(dangling.unwrap_or_else(Doc::nil).indent(nest))
                .append(Doc::hardline())
                .append(Doc::text("}"));
        };

        if self.single_line {
            // Laid out now, whatever it costs. The caller is closing this
            // line with ` }` and more besides; a group that broke for
            // width in here would put the newline in the middle of it.
            let was_limit = self.comment_limit.replace(closing_start);
            let inner = self.node_doc(&body);
            self.comment_limit = was_limit;
            return Doc::text(" ").append(inner.flat()).append(Doc::text(" }"));
        }

        let multiline_body = body
            .as_statements_node()
            .is_some_and(|s| s.body().iter().count() > 1);

        // Nothing written after the `}` belongs to a statement inside it.
        let was_limit = self.comment_limit.replace(closing_start);

        if !multiline_body && !has_comment && !holds_block_construct(&body) {
            // The two shapes are different documents, not two layouts of
            // one: flat, anything inside that would lay out its own lines
            // (a nested `BEGIN { }`, a `begin`/`rescue`) takes its
            // one-line shape instead. So both are built and handed to the
            // printer as an `if_break`, which measures the flat one
            // against the room left on the line and picks.
            //
            // `breaks` on the flat build is what says there is no flat
            // shape at all -- a `do...end` call or an expanded collection
            // in there -- in which case only the expanded shape is built.
            let snap = self.render_snapshot();
            let was_single_line = std::mem::replace(&mut self.single_line, true);
            let flat = self.node_doc(&body);
            self.single_line = was_single_line;
            if !flat.breaks() {
                self.rewind_render_snapshot(snap);
                let expanded = self.node_doc(&body);
                self.comment_limit = was_limit;
                return Doc::if_break(
                    Doc::hardline()
                        .append(expanded)
                        .indent(nest)
                        .append(Doc::hardline())
                        .append(Doc::text("}")),
                    Doc::text(" ").append(flat).append(Doc::text(" }")),
                )
                .group();
            }
            self.restore_render_snapshot(snap);
        }

        // A comment after the `}` of a block written on one line is about
        // the whole statement, not about what follows the `{`: it stays
        // after the `}` (`# rubocop:disable` covers only its own line).
        let opening_comment = (self.line_of(opening_end) != self.line_of(closing_start))
            .then(|| self.trailing_comment_doc(self.line_of(opening_end)))
            .flatten();
        self.indent += 1;
        let mut inner = Doc::hardline().append(self.node_doc(&body));
        if let Some(dangling) = self.comments_before_doc(closing_start, true) {
            inner = inner.append(dangling);
        }
        self.indent -= 1;
        self.comment_limit = was_limit;
        opening_comment
            .unwrap_or_else(Doc::nil)
            .append(inner.indent(nest))
            .append(Doc::hardline())
            .append(Doc::text("}"))
    }
}
