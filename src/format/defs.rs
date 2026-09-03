use super::Formatter;
use pretty::RcDoc as Doc;
use ruby_prism::Visit;

impl Formatter {
    pub(super) fn fmt_def_node(&mut self, node: &ruby_prism::DefNode<'_>) {
        let name = node.name();
        let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");

        self.write_indent();
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
            self.output.push('(');
            self.fmt_parameters_node(&params);
            self.output.push(')');
        }

        self.output.push('\n');
        let header_line = self.line_of(node.location().start_offset());
        self.emit_trailing_comment_if_same_line(header_line);

        self.indent += 1;
        if let Some(body) = node.body() {
            self.visit(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
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

    pub(super) fn fmt_parameters_node(&mut self, node: &ruby_prism::ParametersNode<'_>) {
        let mut wrote_any = false;
        let mut sep = |formatter: &mut Self| {
            if wrote_any {
                formatter.output.push_str(", ");
            }
            wrote_any = true;
        };

        for param in node.requireds().iter() {
            sep(self);
            self.visit(&param);
        }
        for param in node.optionals().iter() {
            sep(self);
            self.visit(&param);
        }
        if let Some(rest) = node.rest() {
            sep(self);
            self.visit(&rest);
        }
        for param in node.posts().iter() {
            sep(self);
            self.visit(&param);
        }
        for param in node.keywords().iter() {
            sep(self);
            self.visit(&param);
        }
        if let Some(keyword_rest) = node.keyword_rest() {
            sep(self);
            self.visit(&keyword_rest);
        }
        if let Some(block) = node.block() {
            sep(self);
            self.fmt_block_parameter_node(&block);
        }
    }

    pub(super) fn fmt_block_parameters_node(&mut self, node: &ruby_prism::BlockParametersNode<'_>) {
        // A lambda's parameters are parenthesized (`->(x) { }`) while a
        // block's are pipe-delimited (`{ |x| }`), and both are this node
        // type — so take the delimiters from source rather than assuming
        // pipes.
        let opening = node.opening_loc().map(|loc| loc.as_slice().to_vec());
        let closing = node.closing_loc().map(|loc| loc.as_slice().to_vec());

        match &opening {
            Some(bytes) => self.write_bytes(bytes),
            None => self.output.push('|'),
        }
        if let Some(params) = node.parameters() {
            self.fmt_parameters_node(&params);
        }
        match &closing {
            Some(bytes) => self.write_bytes(bytes),
            None => self.output.push('|'),
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
        if let Some(parameters) = node.parameters() {
            self.inline = true;
            self.visit(&parameters);
        }

        let brace = node.opening_loc().as_slice() == b"{";
        self.output.push(' ');
        self.output.push_str(if brace { "{" } else { "do" });

        if brace {
            let doc = match node.body() {
                Some(body) => {
                    let was_single_line = self.single_line;
                    self.single_line = true;
                    let body_doc = self.capture_as_doc(|f| {
                        f.inline = true;
                        f.visit(&body);
                    });
                    self.single_line = was_single_line;
                    Doc::text(" ").append(body_doc).append(Doc::text(" }"))
                }
                None => Doc::text("}"),
            };
            let rendered = self.render_doc_to_string(doc);
            self.output.push_str(&rendered);
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
            let doc = match node.body() {
                Some(body) => {
                    let was_single_line = self.single_line;
                    self.single_line = true;
                    let body_doc = self.capture_as_doc(|f| {
                        f.inline = true;
                        f.visit(&body);
                    });
                    self.single_line = was_single_line;
                    Doc::text(" ").append(body_doc).append(Doc::text(" }"))
                }
                None => Doc::text("}"),
            };
            let rendered = self.render_doc_to_string(doc);
            self.output.push_str(&rendered);
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
}
