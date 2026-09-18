use super::Formatter;
use super::doc::Doc;

impl Formatter {
    fn write_def_node(&mut self, node: &ruby_prism::DefNode<'_>) {
        let inline = self.inline;
        // Set by write_brace_block_body when rendering a brace block's body flat
        // (`{ ... }`). A hardcoded '\n' after the signature (below) broke
        // that flatness regardless of indent bookkeeping, and whether "end"
        // ended up separated from the body at all then depended on
        // whatever self.indent's numeric value happened to be (0 at true
        // top level renders no separating spaces at all) -- `Class.new {
        // def self.a; self; end }` could come out as literal `selfend }`,
        // a syntax error. Joining with "; " instead, like
        // fmt_statements_node/fmt_begin_node already do in this mode, is
        // correct regardless of indent depth.
        let single_line = self.single_line;
        // The name as written, not `node.name()`: `def ~@` and `def !@`
        // are interned as `~` and `!`, and there is no reason to respell
        // what the author wrote.
        let name = node.name_loc();
        let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");

        if !inline {
            self.write_indent();
        }
        self.output.push_str("def ");
        if let Some(receiver) = node.receiver() {
            self.inline = true;
            self.visit(&receiver);
            let op = node
                .operator_loc()
                .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
                .unwrap_or(".");
            self.output.push_str(op);
        }
        self.output.push_str(name_str);

        if let Some(params) = node.parameters() {
            let doc = self.bracketed_parameters_doc("(", &params, ")");
            let rendered = self.render_doc_to_string(doc);
            self.output.push_str(&rendered);
        }

        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            // A trailing comment sits after wherever the signature actually
            // ends -- the closing paren when params wrapped onto their own
            // lines, not the `def` keyword's line, which may be several
            // lines earlier by then.
            let header_end = node
                .rparen_loc()
                .map(|l| l.start_offset())
                .unwrap_or_else(|| node.location().start_offset());
            let header_line = self.line_of(header_end);
            self.emit_trailing_comment_if_same_line(header_line);
            self.indent += 1;
        }

        if let Some(body) = node.body() {
            self.visit(&body);
        }

        if single_line {
            self.output.push_str("; end");
        } else {
            if let Some(end_keyword) = node.end_keyword_loc() {
                self.flush_comments_before(end_keyword.start_offset(), node.body().is_some());
            }
            self.indent -= 1;
            self.write_indent();
            self.output.push_str("end");
        }
        if !inline {
            self.output.push('\n');
        }
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

    pub(super) fn fmt_implicit_rest_node(
        &mut self,
        _node: &ruby_prism::ImplicitRestNode<'_>,
    ) -> Doc {
        Doc::text("*")
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
            leaves.push(self.node_doc(&rest));
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
    /// Structurally a block with an arrow in front, so the body is rendered
    /// the same way `write_block_node` does it — but it is its own node type,
    /// and without this it fell through to Prism's default visitor, which
    /// emitted the parameters in block form and dropped the `->`, the
    /// parens and the braces entirely: `->(_) { block.call }` came out as
    /// `|_| block.call`.
    fn write_lambda_node(&mut self, node: &ruby_prism::LambdaNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }

        let operator = node.operator_loc().as_slice().to_vec();
        self.write_bytes(&operator);
        let brace = node.opening_loc().as_slice() == b"{";
        if let Some(parameters) = super::explicit_parameters(node.parameters()) {
            self.inline = true;
            // " {"/" do" follows right after the parameter list on the
            // same line -- reserve its width so an overlong param list
            // wraps instead of pushing the whole line past line_length
            // (render_doc_to_string can't see what gets appended after it
            // renders). Brace-block bodies overflow their own line
            // independently and aren't reserved for here.
            let reserve = if brace { 0 } else { " do".len() };
            self.trailing_reserve += reserve;
            self.visit(&parameters);
            self.trailing_reserve -= reserve;
        }

        self.output.push(' ');
        self.output.push_str(if brace { "{" } else { "do" });

        let (opening_end, closing_start) = (
            node.opening_loc().end_offset(),
            node.closing_loc().start_offset(),
        );
        if brace {
            self.write_brace_block_body(node.body(), opening_end, closing_start);
        } else {
            self.output.push('\n');
            self.emit_trailing_comment_if_same_line(self.line_of(opening_end));
            self.indent += 1;
            if let Some(body) = node.body() {
                self.inline = false;
                self.visit(&body);
            }
            self.flush_comments_before(closing_start, node.body().is_some());
            self.indent -= 1;
            self.write_indent();
            self.output.push_str("end");
        }

        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn write_block_node(&mut self, node: &ruby_prism::BlockNode<'_>) {
        let brace = node.opening_loc().as_slice() == b"{";

        self.output.push(' ');
        self.output.push_str(if brace { "{" } else { "do" });

        if let Some(params) = super::explicit_parameters(node.parameters()) {
            self.output.push(' ');
            self.inline = true;
            self.visit(&params);
        }

        let (opening_end, closing_start) = (
            node.opening_loc().end_offset(),
            node.closing_loc().start_offset(),
        );
        if brace {
            self.write_brace_block_body(node.body(), opening_end, closing_start);
        } else {
            self.output.push('\n');
            // `do |x| # why`: the header ends at the closing pipe when
            // there are parameters, at `do` otherwise.
            let header_end = node
                .parameters()
                .map_or(opening_end, |p| p.location().end_offset().max(opening_end));
            self.emit_trailing_comment_if_same_line(self.line_of(header_end));
            self.indent += 1;
            if let Some(body) = node.body() {
                self.visit(&body);
            }
            self.flush_comments_before(closing_start, node.body().is_some());
            self.indent -= 1;
            self.write_indent();
            self.output.push_str("end");
        }
    }

    /// Brace-block / brace-lambda body: stay ` { stmt }` when there's a
    /// single statement and it fits on the current line, otherwise the
    /// same all-or-nothing expanded shape as `do...end` (body at +1
    /// indent, `}` on its own line). A body with more than one statement
    /// always expands, regardless of width. A `{ }` nested inside another
    /// still-flat brace body stays flat.
    ///
    /// A comment anywhere between the braces also forces the expanded
    /// shape: `{ # why` has no flat form that keeps the comment in place.
    fn write_brace_block_body(
        &mut self,
        body: Option<ruby_prism::Node<'_>>,
        opening_end: usize,
        closing_start: usize,
    ) {
        let has_comment = self.has_pending_comment_in(opening_end, closing_start)
            && !self.single_line
            && self.capture_depth == 0;
        let Some(body) = body else {
            if has_comment {
                self.output.push('\n');
                self.emit_trailing_comment_if_same_line(self.line_of(opening_end));
                self.indent += 1;
                self.flush_comments_before(closing_start, false);
                self.indent -= 1;
                self.write_indent();
            }
            self.output.push('}');
            return;
        };

        if self.single_line || self.capture_depth > 0 {
            self.output.push(' ');
            let was_single_line = self.single_line;
            self.single_line = true;
            self.inline = true;
            self.visit(&body);
            self.single_line = was_single_line;
            self.output.push_str(" }");
            return;
        }

        // A body with more than one statement never collapses onto one
        // line, even when the flat form would fit -- joining statements
        // with "; " inside `{ }` is what RuboCop's Style/Semicolon flags,
        // and it silently collapses structure the source author chose to
        // keep multi-line (e.g. a lambda body split across several lines).
        // Confirmed against rails's action_controller/metal.rb, where a
        // 4-line lambda body fit under line_length flat and so got
        // squashed to one line before this check.
        let multiline_body = body
            .as_statements_node()
            .is_some_and(|s| s.body().iter().count() > 1);

        if !multiline_body && !has_comment {
            let snap = self.capture_snapshot();
            let was_single_line = self.single_line;
            self.single_line = true;
            let captured = self.capture_to_string(|f| {
                f.inline = true;
                f.visit(&body);
            });
            self.single_line = was_single_line;
            let flat_width =
                " ".len() + unicode_width::UnicodeWidthStr::width(captured.as_str()) + " }".len();
            if !captured.contains('\n')
                && self
                    .current_column()
                    .saturating_add(flat_width)
                    .saturating_add(self.trailing_reserve)
                    <= self.line_length
            {
                self.output.push(' ');
                self.output.push_str(&captured);
                self.output.push_str(" }");
                return;
            }
            self.restore_capture_snapshot(snap);
        }
        self.output.push('\n');
        self.emit_trailing_comment_if_same_line(self.line_of(opening_end));
        self.indent += 1;
        self.inline = false;
        self.visit(&body);
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.flush_comments_before(closing_start, true);
        self.indent -= 1;
        self.write_indent();
        self.output.push('}');
    }

    // A `def`, a lambda and a block lay out their own multi-line body with
    // `self.indent` and real newlines. As `Doc`s they are rendered where
    // they will sit and lifted straight back out, so the bytes are what the
    // string emitter produced and every width decision inside is made
    // against the real column -- the same bargain `statements_doc` makes.
    //
    // All three are listed under `self_indented` in the dispatch: a `def`
    // writes its own leading indent (or none, when it is a sub-expression),
    // and a block writes no indent at all because it continues the line its
    // call ends on.
    pub(super) fn fmt_def_node(&mut self, node: &ruby_prism::DefNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_def_node(node);
        })
    }

    pub(super) fn fmt_lambda_node(&mut self, node: &ruby_prism::LambdaNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_lambda_node(node);
        })
    }

    pub(super) fn fmt_block_node(&mut self, node: &ruby_prism::BlockNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_block_node(node);
        })
    }
}
