use super::Formatter;
use pretty::RcDoc as Doc;
use ruby_prism::Visit;

impl Formatter {
    pub(super) fn fmt_def_node(&mut self, node: &ruby_prism::DefNode<'_>) {
        let inline = self.inline;
        // Set by fmt_block_node when rendering a brace block's body flat
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
        let name = node.name();
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
            self.write_bracketed_parameters("(", &params, ")");
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
    ) {
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
    }

    pub(super) fn fmt_optional_parameter_node(
        &mut self,
        node: &ruby_prism::OptionalParameterNode<'_>,
    ) {
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        self.output.push_str(" = ");
        self.inline = true;
        self.visit(&node.value());
        self.inline = false;
    }

    pub(super) fn fmt_rest_parameter_node(&mut self, node: &ruby_prism::RestParameterNode<'_>) {
        self.output.push('*');
        if let Some(name) = node.name() {
            self.output
                .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        }
    }

    pub(super) fn fmt_implicit_rest_node(&mut self, _node: &ruby_prism::ImplicitRestNode<'_>) {
        self.output.push('*');
    }

    pub(super) fn fmt_required_keyword_parameter_node(
        &mut self,
        node: &ruby_prism::RequiredKeywordParameterNode<'_>,
    ) {
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        self.output.push(':');
    }

    pub(super) fn fmt_optional_keyword_parameter_node(
        &mut self,
        node: &ruby_prism::OptionalKeywordParameterNode<'_>,
    ) {
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        self.output.push_str(": ");
        self.inline = true;
        self.visit(&node.value());
        self.inline = false;
    }

    pub(super) fn fmt_keyword_rest_parameter_node(
        &mut self,
        node: &ruby_prism::KeywordRestParameterNode<'_>,
    ) {
        self.output.push_str("**");
        if let Some(name) = node.name() {
            self.output
                .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        }
    }

    pub(super) fn fmt_no_keywords_parameter_node(
        &mut self,
        _node: &ruby_prism::NoKeywordsParameterNode<'_>,
    ) {
        self.output.push_str("**nil");
    }

    pub(super) fn fmt_forwarding_parameter_node(
        &mut self,
        _node: &ruby_prism::ForwardingParameterNode<'_>,
    ) {
        self.output.push_str("...");
    }

    pub(super) fn fmt_block_parameter_node(&mut self, node: &ruby_prism::BlockParameterNode<'_>) {
        self.output.push('&');
        if let Some(name) = node.name() {
            self.output
                .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        }
    }

    /// Collects a parameter list's entries as a `Doc` leaf per parameter, in
    /// declaration order (requireds, optionals, rest, posts, keywords,
    /// keyword-rest, block) -- shared by `fmt_parameters_node` (which joins
    /// them as flat text, for contexts that never wrap) and
    /// `write_bracketed_parameters` (which wraps them all-or-nothing, block
    /// indented, like a call's argument list).
    fn parameter_leaves(&mut self, node: &ruby_prism::ParametersNode<'_>) -> Vec<Doc<'static, ()>> {
        let mut leaves = Vec::new();
        let mut push = |formatter: &mut Self, visit: &dyn Fn(&mut Self)| {
            leaves.push(formatter.capture_as_doc(|f| {
                f.inline = true;
                visit(f);
            }));
        };

        for param in node.requireds().iter() {
            push(self, &|f| f.visit(&param));
        }
        for param in node.optionals().iter() {
            push(self, &|f| f.visit(&param));
        }
        if let Some(rest) = node.rest() {
            push(self, &|f| f.visit(&rest));
        }
        for param in node.posts().iter() {
            push(self, &|f| f.visit(&param));
        }
        for param in node.keywords().iter() {
            push(self, &|f| f.visit(&param));
        }
        if let Some(keyword_rest) = node.keyword_rest() {
            push(self, &|f| f.visit(&keyword_rest));
        }
        if let Some(block) = node.block() {
            push(self, &|f| f.fmt_block_parameter_node(&block));
        }
        leaves
    }

    pub(super) fn fmt_parameters_node(&mut self, node: &ruby_prism::ParametersNode<'_>) {
        let leaves = self.parameter_leaves(node);
        for (i, leaf) in leaves.into_iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(&self.render_doc_to_string(leaf));
        }
    }

    /// Renders a parameter list wrapped in `open`/`close` (a def's parens or
    /// a lambda's), all-or-nothing and block-indented like a call's
    /// argument list, so an overlong signature breaks one parameter per
    /// line instead of running past the line length.
    fn write_bracketed_parameters(
        &mut self,
        open: &str,
        node: &ruby_prism::ParametersNode<'_>,
        close: &str,
    ) {
        let leaves = self.parameter_leaves(node);
        let body = Doc::intersperse(leaves, Doc::text(",").append(Doc::line()));
        let rendered = self.wrap_bracketed_group(open, Doc::line_(), body, Doc::line_(), close);
        self.output.push_str(&rendered);
    }

    pub(super) fn fmt_block_parameters_node(&mut self, node: &ruby_prism::BlockParametersNode<'_>) {
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

        match node.parameters() {
            // Only a lambda's real parens are worth all-or-nothing wrapping
            // like a call's argument list. A block's `|params|` is never
            // meant to span multiple lines -- forcing a wrap there splits
            // the pipes across lines and leaves the block's body dangling
            // after the closing pipe on the same line as its last param.
            Some(params) if opening == "(" => {
                self.write_bracketed_parameters(&opening, &params, &closing)
            }
            Some(params) => {
                self.output.push_str(&opening);
                self.fmt_parameters_node(&params);
                self.output.push_str(&closing);
            }
            None => {
                self.output.push_str(&opening);
                self.output.push_str(&closing);
            }
        }
    }

    /// A lambda literal (`->(x) { x + 1 }`, `-> { 42 }`, `->(x) do ... end`).
    ///
    /// Structurally a block with an arrow in front, so the body is rendered
    /// the same way `fmt_block_node` does it — but it is its own node type,
    /// and without this it fell through to Prism's default visitor, which
    /// emitted the parameters in block form and dropped the `->`, the
    /// parens and the braces entirely: `->(_) { block.call }` came out as
    /// `|_| block.call`.
    pub(super) fn fmt_lambda_node(&mut self, node: &ruby_prism::LambdaNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }

        let operator = node.operator_loc().as_slice().to_vec();
        self.write_bytes(&operator);
        let brace = node.opening_loc().as_slice() == b"{";
        if let Some(parameters) = node.parameters() {
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

        if brace {
            self.write_brace_block_body(node.body());
        } else {
            self.output.push('\n');
            self.indent += 1;
            if let Some(body) = node.body() {
                self.inline = false;
                self.visit(&body);
            }
            self.indent -= 1;
            self.write_indent();
            self.output.push_str("end");
        }

        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_block_node(&mut self, node: &ruby_prism::BlockNode<'_>) {
        let brace = node.opening_loc().as_slice() == b"{";

        self.output.push(' ');
        self.output.push_str(if brace { "{" } else { "do" });

        if let Some(params) = node.parameters() {
            self.output.push(' ');
            self.inline = true;
            self.visit(&params);
        }

        if brace {
            self.write_brace_block_body(node.body());
        } else {
            self.output.push('\n');
            self.indent += 1;
            if let Some(body) = node.body() {
                self.visit(&body);
            }
            self.indent -= 1;
            self.write_indent();
            self.output.push_str("end");
        }
    }

    /// Brace-block / brace-lambda body: stay ` { stmt }` when that fits
    /// on the current line, otherwise the same all-or-nothing expanded
    /// shape as `do...end` (body at +1 indent, `}` on its own line). A
    /// `{ }` nested inside another still-flat brace body stays flat.
    fn write_brace_block_body(&mut self, body: Option<ruby_prism::Node<'_>>) {
        let Some(body) = body else {
            self.output.push('}');
            return;
        };

        if self.single_line {
            self.output.push(' ');
            self.inline = true;
            self.visit(&body);
            self.output.push_str(" }");
            return;
        }

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
        self.output.push('\n');
        self.indent += 1;
        self.inline = false;
        self.visit(&body);
        self.indent -= 1;
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.write_indent();
        self.output.push('}');
    }
}
