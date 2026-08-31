use crate::config::FormatConfig;
use ruby_prism::Visit;

mod literals;
mod variables;

pub fn format(source: &str) -> String {
    format_with_config(source, &FormatConfig::default())
}

pub fn format_with_config(source: &str, config: &FormatConfig) -> String {
    let parsed = ruby_prism::parse(source.as_bytes());
    let node = parsed.node();

    let mut formatter = Formatter::new(source, &parsed, config);
    formatter.visit(&node);
    formatter.flush_remaining_comments();
    let trimmed = formatter.output.trim_end_matches('\n');
    let mut result = trimmed.to_string();
    result.push('\n');
    result
}

struct CommentSpan {
    start: usize,
    text: String,
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
    comments: Vec<CommentSpan>,
    comment_cursor: usize,
    line_starts: Vec<usize>,
}

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
            comments,
            comment_cursor: 0,
            line_starts: line_starts(source),
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

        let extra_indent = prefix_len / self.indent_str.len();
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

        if positional.is_empty() && block_pass.is_none() && !parenthesized {
            return;
        }

        self.output.push(if parenthesized { '(' } else { ' ' });
        for (i, arg) in positional.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(arg);
        }
        if let Some(block_pass) = &block_pass {
            if !positional.is_empty() {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(block_pass);
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

impl Visit<'_> for Formatter {
    fn visit_statements_node(&mut self, node: &ruby_prism::StatementsNode<'_>) {
        let stmts = node.body();

        if self.single_line {
            for (i, stmt) in stmts.iter().enumerate() {
                if i > 0 {
                    self.output.push_str("; ");
                }
                self.inline = true;
                self.visit(&stmt);
            }
            return;
        }

        let mut prev_end_line: Option<usize> = None;

        for stmt in stmts.iter() {
            let start_line = self.line_of(stmt.location().start_offset());

            while self.comment_cursor < self.comments.len() {
                let comment_line = self.line_of(self.comments[self.comment_cursor].start);
                if comment_line >= start_line {
                    break;
                }
                if prev_end_line.is_some_and(|prev| comment_line > prev + 1) {
                    self.ensure_blank_line();
                }
                self.write_indent();
                let formatted =
                    Self::format_comment(&self.comments[self.comment_cursor].text).into_owned();
                self.output.push_str(&formatted);
                self.output.push('\n');
                prev_end_line = Some(comment_line);
                self.comment_cursor += 1;
            }

            if prev_end_line.is_some_and(|prev| start_line > prev + 1) {
                self.ensure_blank_line();
            }

            self.inline = false;
            self.visit(&stmt);

            let end_line = self.line_of(stmt.location().end_offset().saturating_sub(1));
            self.emit_trailing_comment_if_same_line(end_line);
            prev_end_line = Some(end_line);
        }
    }

    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'_>) {
        let name = node.name();
        let name_str = std::str::from_utf8(name.as_slice()).unwrap_or("");
        let inline = self.inline;
        self.inline = false;

        if let Some(receiver) = node.receiver() {
            if !inline {
                self.write_indent();
            }

            let is_index = name_str == "[]" || name_str == "[]=";
            let is_identifier_call = name_str
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');

            if is_index {
                self.inline = true;
                self.visit(&receiver);
                self.output.push('[');
                let mut args: Vec<_> = node
                    .arguments()
                    .map(|a| a.arguments().iter().collect())
                    .unwrap_or_default();
                let value = if name_str == "[]=" { args.pop() } else { None };
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.inline = true;
                    self.visit(arg);
                }
                self.output.push(']');
                if let Some(value) = value {
                    self.output.push_str(" = ");
                    self.inline = true;
                    self.visit(&value);
                }
            } else if name_str == "!" && node.arguments().is_none() {
                self.output.push('!');
                self.inline = true;
                self.visit(&receiver);
            } else if is_identifier_call {
                self.inline = true;
                self.visit(&receiver);
                let op = node
                    .call_operator_loc()
                    .and_then(|l| std::str::from_utf8(l.as_slice()).ok())
                    .unwrap_or(".");
                self.output.push_str(op);

                if node.is_attribute_write() {
                    self.output
                        .push_str(name_str.strip_suffix('=').unwrap_or(name_str));
                    self.output.push_str(" = ");
                    if let Some(value) = node.arguments().and_then(|a| a.arguments().iter().next())
                    {
                        self.inline = true;
                        self.visit(&value);
                    }
                } else {
                    self.output.push_str(name_str);
                    let parenthesized = node.opening_loc().is_some();
                    self.write_call_args(node, parenthesized);
                }
            } else {
                self.inline = true;
                self.visit(&receiver);
                self.output.push(' ');
                self.output.push_str(name_str);
                if let Some(args) = node.arguments() {
                    self.output.push(' ');
                    let arguments = args.arguments();
                    for (i, arg) in arguments.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.inline = true;
                        self.visit(&arg);
                    }
                }
            }
            self.write_call_block(node);
            if !inline {
                self.output.push('\n');
            }
        } else {
            if !inline {
                self.write_indent();
            }
            self.output.push_str(name_str);
            let parenthesized = node.opening_loc().is_some();
            self.write_call_args(node, parenthesized);
            self.write_call_block(node);
            if !inline {
                self.output.push('\n');
            }
        }
    }

    fn visit_super_node(&mut self, node: &ruby_prism::SuperNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("super");

        let parenthesized = node.lparen_loc().is_some();
        let positional: Vec<_> = node
            .arguments()
            .map(|a| a.arguments().iter().collect())
            .unwrap_or_default();
        let block_pass = match node.block() {
            Some(b @ ruby_prism::Node::BlockArgumentNode { .. }) => Some(b),
            _ => None,
        };

        if parenthesized || !positional.is_empty() || block_pass.is_some() {
            self.output.push(if parenthesized { '(' } else { ' ' });
            for (i, arg) in positional.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(arg);
            }
            if let Some(block_pass) = &block_pass {
                if !positional.is_empty() {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(block_pass);
            }
            if parenthesized {
                self.output.push(')');
            }
        }

        if let Some(block @ ruby_prism::Node::BlockNode { .. }) = node.block() {
            self.visit(&block);
        }

        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_forwarding_super_node(&mut self, node: &ruby_prism::ForwardingSuperNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("super");
        if let Some(block) = node.block() {
            self.visit_block_node(&block);
        }
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_def_node(&mut self, node: &ruby_prism::DefNode<'_>) {
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
            self.visit_parameters_node(&params);
            self.output.push(')');
        }

        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.body() {
            self.visit(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    fn visit_required_parameter_node(&mut self, node: &ruby_prism::RequiredParameterNode<'_>) {
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
    }

    fn visit_optional_parameter_node(&mut self, node: &ruby_prism::OptionalParameterNode<'_>) {
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        self.output.push_str(" = ");
        self.inline = true;
        self.visit(&node.value());
        self.inline = false;
    }

    fn visit_rest_parameter_node(&mut self, node: &ruby_prism::RestParameterNode<'_>) {
        self.output.push('*');
        if let Some(name) = node.name() {
            self.output
                .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        }
    }

    fn visit_implicit_rest_node(&mut self, _node: &ruby_prism::ImplicitRestNode<'_>) {
        self.output.push('*');
    }

    fn visit_required_keyword_parameter_node(
        &mut self,
        node: &ruby_prism::RequiredKeywordParameterNode<'_>,
    ) {
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        self.output.push(':');
    }

    fn visit_optional_keyword_parameter_node(
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

    fn visit_keyword_rest_parameter_node(
        &mut self,
        node: &ruby_prism::KeywordRestParameterNode<'_>,
    ) {
        self.output.push_str("**");
        if let Some(name) = node.name() {
            self.output
                .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        }
    }

    fn visit_no_keywords_parameter_node(
        &mut self,
        _node: &ruby_prism::NoKeywordsParameterNode<'_>,
    ) {
        self.output.push_str("**nil");
    }

    fn visit_forwarding_parameter_node(&mut self, _node: &ruby_prism::ForwardingParameterNode<'_>) {
        self.output.push_str("...");
    }

    fn visit_block_parameter_node(&mut self, node: &ruby_prism::BlockParameterNode<'_>) {
        self.output.push('&');
        if let Some(name) = node.name() {
            self.output
                .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
        }
    }

    fn visit_parameters_node(&mut self, node: &ruby_prism::ParametersNode<'_>) {
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
            self.visit_block_parameter_node(&block);
        }
    }

    fn visit_block_parameters_node(&mut self, node: &ruby_prism::BlockParametersNode<'_>) {
        self.output.push('|');
        if let Some(params) = node.parameters() {
            self.visit_parameters_node(&params);
        }
        self.output.push('|');
    }

    fn visit_block_node(&mut self, node: &ruby_prism::BlockNode<'_>) {
        let brace = node.opening_loc().as_slice() == b"{";

        self.output.push(' ');
        self.output.push_str(if brace { "{" } else { "do" });

        if let Some(params) = node.parameters() {
            self.output.push(' ');
            self.inline = true;
            self.visit(&params);
        }

        if brace {
            if let Some(body) = node.body() {
                self.output.push(' ');
                let was_single_line = self.single_line;
                self.single_line = true;
                self.inline = true;
                self.visit(&body);
                self.single_line = was_single_line;
            }
            self.output.push_str(" }");
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

    fn visit_yield_node(&mut self, node: &ruby_prism::YieldNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("yield");
        if let Some(args) = node.arguments() {
            self.output.push(' ');
            for (i, arg) in args.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
        }
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_return_node(&mut self, node: &ruby_prism::ReturnNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("return");
        if let Some(args) = node.arguments() {
            self.output.push(' ');
            for (i, arg) in args.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
        }
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_break_node(&mut self, node: &ruby_prism::BreakNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("break");
        if let Some(args) = node.arguments() {
            self.output.push(' ');
            for (i, arg) in args.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
        }
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_next_node(&mut self, node: &ruby_prism::NextNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("next");
        if let Some(args) = node.arguments() {
            self.output.push(' ');
            for (i, arg) in args.arguments().iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.inline = true;
                self.visit(&arg);
            }
        }
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_local_variable_read_node(&mut self, node: &ruby_prism::LocalVariableReadNode<'_>) {
        self.fmt_local_variable_read_node(node);
    }

    fn visit_global_variable_read_node(&mut self, node: &ruby_prism::GlobalVariableReadNode<'_>) {
        self.fmt_global_variable_read_node(node);
    }

    fn visit_instance_variable_read_node(
        &mut self,
        node: &ruby_prism::InstanceVariableReadNode<'_>,
    ) {
        self.fmt_instance_variable_read_node(node);
    }

    fn visit_class_variable_read_node(&mut self, node: &ruby_prism::ClassVariableReadNode<'_>) {
        self.fmt_class_variable_read_node(node);
    }

    fn visit_self_node(&mut self, node: &ruby_prism::SelfNode<'_>) {
        self.fmt_self_node(node);
    }

    fn visit_local_variable_write_node(&mut self, node: &ruby_prism::LocalVariableWriteNode<'_>) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    fn visit_local_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    fn visit_local_variable_or_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    fn visit_local_variable_and_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    fn visit_instance_variable_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    fn visit_instance_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    fn visit_instance_variable_or_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    fn visit_instance_variable_and_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    fn visit_constant_write_node(&mut self, node: &ruby_prism::ConstantWriteNode<'_>) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    fn visit_constant_operator_write_node(
        &mut self,
        node: &ruby_prism::ConstantOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    fn visit_constant_or_write_node(&mut self, node: &ruby_prism::ConstantOrWriteNode<'_>) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    fn visit_constant_and_write_node(&mut self, node: &ruby_prism::ConstantAndWriteNode<'_>) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    fn visit_global_variable_write_node(&mut self, node: &ruby_prism::GlobalVariableWriteNode<'_>) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    fn visit_global_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    fn visit_global_variable_or_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    fn visit_global_variable_and_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    fn visit_class_variable_write_node(&mut self, node: &ruby_prism::ClassVariableWriteNode<'_>) {
        self.write_assignment(node.name().as_slice(), "=", &node.value());
    }

    fn visit_class_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableOperatorWriteNode<'_>,
    ) {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment(node.name().as_slice(), op, &node.value());
    }

    fn visit_class_variable_or_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableOrWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "||=", &node.value());
    }

    fn visit_class_variable_and_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableAndWriteNode<'_>,
    ) {
        self.write_assignment(node.name().as_slice(), "&&=", &node.value());
    }

    fn visit_constant_path_write_node(&mut self, node: &ruby_prism::ConstantPathWriteNode<'_>) {
        let target = node.target();
        self.write_assignment_to(|f| f.visit(&target.as_node()), "=", &node.value());
    }

    fn visit_constant_path_operator_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathOperatorWriteNode<'_>,
    ) {
        let target = node.target();
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.write_assignment_to(|f| f.visit(&target.as_node()), op, &node.value());
    }

    fn visit_constant_path_or_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathOrWriteNode<'_>,
    ) {
        let target = node.target();
        self.write_assignment_to(|f| f.visit(&target.as_node()), "||=", &node.value());
    }

    fn visit_constant_path_and_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathAndWriteNode<'_>,
    ) {
        let target = node.target();
        self.write_assignment_to(|f| f.visit(&target.as_node()), "&&=", &node.value());
    }

    fn visit_multi_write_node(&mut self, node: &ruby_prism::MultiWriteNode<'_>) {
        let inline = self.inline;
        self.inline = false;
        if !inline {
            self.write_indent();
        }

        let lefts = node.lefts();
        let rights = node.rights();
        let mut first = true;
        for target in lefts.iter() {
            if !first {
                self.output.push_str(", ");
            }
            first = false;
            self.visit(&target);
        }
        if let Some(rest) = node.rest() {
            if !first {
                self.output.push_str(", ");
            }
            first = false;
            self.visit(&rest);
        }
        for target in rights.iter() {
            if !first {
                self.output.push_str(", ");
            }
            first = false;
            self.visit(&target);
        }

        self.output.push_str(" = ");
        self.inline = true;
        self.visit(&node.value());
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_local_variable_target_node(&mut self, node: &ruby_prism::LocalVariableTargetNode<'_>) {
        self.write_bytes(node.name().as_slice());
    }

    fn visit_splat_node(&mut self, node: &ruby_prism::SplatNode<'_>) {
        self.output.push('*');
        if let Some(expression) = node.expression() {
            self.inline = true;
            self.visit(&expression);
        }
    }

    fn visit_block_argument_node(&mut self, node: &ruby_prism::BlockArgumentNode<'_>) {
        self.output.push('&');
        if let Some(expression) = node.expression() {
            self.inline = true;
            self.visit(&expression);
        }
    }

    fn visit_array_node(&mut self, node: &ruby_prism::ArrayNode<'_>) {
        self.write_leaf(|f| {
            let bracketed = node.opening_loc().is_some();
            if bracketed {
                f.output.push('[');
            }
            for (i, element) in node.elements().iter().enumerate() {
                if i > 0 {
                    f.output.push_str(", ");
                }
                f.inline = true;
                f.visit(&element);
            }
            if bracketed {
                f.output.push(']');
            }
        });
    }

    fn visit_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) {
        let class_line = self.line_of(node.location().start_offset());
        self.write_indent();
        self.output.push_str("class ");
        self.inline = true;
        self.visit(&node.constant_path());

        if let Some(superclass) = node.superclass() {
            self.output.push_str(" < ");
            self.visit(&superclass);
        }

        self.output.push('\n');
        self.emit_trailing_comment_if_same_line(class_line);

        self.indent += 1;
        if let Some(body) = node.body() {
            self.visit(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    fn visit_module_node(&mut self, node: &ruby_prism::ModuleNode<'_>) {
        self.write_indent();
        self.output.push_str("module ");
        self.inline = true;
        self.visit(&node.constant_path());
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.body() {
            self.visit(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    fn visit_constant_path_node(&mut self, node: &ruby_prism::ConstantPathNode<'_>) {
        if let Some(parent) = node.parent() {
            self.visit(&parent);
            self.output.push_str("::");
        }
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.unwrap().as_slice()).unwrap_or(""));
    }

    fn visit_constant_read_node(&mut self, node: &ruby_prism::ConstantReadNode<'_>) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    fn visit_if_node(&mut self, node: &ruby_prism::IfNode<'_>) {
        let keyword = node.if_keyword_loc().map(|k| k.as_slice());
        let is_ternary = keyword.is_none();
        let is_elsif = keyword == Some(b"elsif");
        let is_modifier = !is_ternary && !is_elsif && node.end_keyword_loc().is_none();

        if is_ternary {
            // Ternary: predicate ? true_branch : false_branch
            self.inline = true;
            self.visit(&node.predicate());
            self.output.push_str(" ? ");
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" : ");
            if let Some(consequent) = node.subsequent()
                && let Some(else_node) = consequent.as_else_node()
                && let Some(body) = else_node.statements()
            {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            return;
        }

        if is_modifier {
            // Modifier: body if condition
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" if ");
            self.inline = true;
            self.visit(&node.predicate());
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        // Block if / elsif
        let inline = self.inline;
        if !is_elsif && !inline {
            self.write_indent();
        }
        self.output
            .push_str(if is_elsif { "elsif " } else { "if " });
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.visit_statements_node(&body);
        }
        self.indent -= 1;

        if let Some(consequent) = node.subsequent() {
            self.write_indent();
            self.visit(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end\n");
        }
    }

    fn visit_else_node(&mut self, node: &ruby_prism::ElseNode<'_>) {
        self.output.push_str("else\n");

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.visit_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    fn visit_unless_node(&mut self, node: &ruby_prism::UnlessNode<'_>) {
        let is_modifier = node.end_keyword_loc().is_none();

        if is_modifier {
            // Modifier: body unless condition
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" unless ");
            self.inline = true;
            self.visit(&node.predicate());
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        // Block unless
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("unless ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.visit_statements_node(&body);
        }
        self.indent -= 1;

        if let Some(consequent) = node.else_clause() {
            self.write_indent();
            self.visit_else_node(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end\n");
        }
    }

    fn visit_while_node(&mut self, node: &ruby_prism::WhileNode<'_>) {
        let is_modifier = node.closing_loc().is_none();

        if is_modifier {
            // Modifier: body while condition
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" while ");
            self.inline = true;
            self.visit(&node.predicate());
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        // Block while
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("while ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.visit_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    fn visit_until_node(&mut self, node: &ruby_prism::UntilNode<'_>) {
        let is_modifier = node.closing_loc().is_none();

        if is_modifier {
            // Modifier: body until condition
            let inline = self.inline;
            if !inline {
                self.write_indent();
            }
            if let Some(body) = node.statements() {
                for (i, stmt) in body.body().iter().enumerate() {
                    if i > 0 {
                        self.output.push_str("; ");
                    }
                    self.inline = true;
                    self.visit(&stmt);
                }
            }
            self.output.push_str(" until ");
            self.inline = true;
            self.visit(&node.predicate());
            if !inline {
                self.output.push('\n');
            }
            return;
        }

        // Block until
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("until ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.visit_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    fn visit_for_node(&mut self, node: &ruby_prism::ForNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("for ");
        self.inline = true;
        self.visit(&node.index());
        self.output.push_str(" in ");
        self.inline = true;
        self.visit(&node.collection());
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.visit_statements_node(&body);
        }
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    fn visit_case_node(&mut self, node: &ruby_prism::CaseNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("case");
        if let Some(predicate) = node.predicate() {
            self.output.push(' ');
            self.inline = true;
            self.visit(&predicate);
            self.inline = false;
        }
        self.output.push('\n');

        for when in node.conditions().iter() {
            self.visit(&when);
        }

        if let Some(consequent) = node.else_clause() {
            self.write_indent();
            self.visit_else_node(&consequent);
        } else {
            self.write_indent();
            self.output.push_str("end\n");
        }
    }

    fn visit_when_node(&mut self, node: &ruby_prism::WhenNode<'_>) {
        self.write_indent();
        self.output.push_str("when ");
        for (i, condition) in node.conditions().iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&condition);
        }
        self.inline = false;
        self.output.push('\n');

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.visit_statements_node(&body);
        }
        self.indent -= 1;
    }

    fn visit_and_node(&mut self, node: &ruby_prism::AndNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.inline = true;
        self.visit(&node.left());
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("&&");
        self.output.push(' ');
        self.output.push_str(op_str);
        self.output.push(' ');
        self.inline = true;
        self.visit(&node.right());
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_or_node(&mut self, node: &ruby_prism::OrNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.inline = true;
        self.visit(&node.left());
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("||");
        self.output.push(' ');
        self.output.push_str(op_str);
        self.output.push(' ');
        self.inline = true;
        self.visit(&node.right());
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_true_node(&mut self, node: &ruby_prism::TrueNode<'_>) {
        self.fmt_true_node(node);
    }

    fn visit_false_node(&mut self, node: &ruby_prism::FalseNode<'_>) {
        self.fmt_false_node(node);
    }

    fn visit_nil_node(&mut self, node: &ruby_prism::NilNode<'_>) {
        self.fmt_nil_node(node);
    }

    fn visit_integer_node(&mut self, node: &ruby_prism::IntegerNode<'_>) {
        self.fmt_integer_node(node);
    }

    fn visit_string_node(&mut self, node: &ruby_prism::StringNode<'_>) {
        self.fmt_string_node(node);
    }

    fn visit_symbol_node(&mut self, node: &ruby_prism::SymbolNode<'_>) {
        self.fmt_symbol_node(node);
    }

    fn visit_float_node(&mut self, node: &ruby_prism::FloatNode<'_>) {
        self.fmt_float_node(node);
    }

    fn visit_range_node(&mut self, node: &ruby_prism::RangeNode<'_>) {
        if let Some(left) = node.left() {
            self.inline = true;
            self.visit(&left);
        }
        let op = std::str::from_utf8(node.operator_loc().as_slice()).unwrap_or("..");
        self.output.push_str(op);
        if let Some(right) = node.right() {
            self.inline = true;
            self.visit(&right);
        }
    }

    fn visit_hash_node(&mut self, node: &ruby_prism::HashNode<'_>) {
        let elements = node.elements();
        if elements.iter().next().is_none() {
            self.write_leaf(|f| f.output.push_str("{}"));
            return;
        }
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("{ ");
        for (i, element) in elements.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&element);
        }
        self.output.push_str(" }");
        if !inline {
            self.output.push('\n');
        }
    }

    fn visit_assoc_node(&mut self, node: &ruby_prism::AssocNode<'_>) {
        self.inline = true;
        self.visit(&node.key());
        if let Some(op) = node.operator_loc() {
            self.output.push(' ');
            self.output
                .push_str(std::str::from_utf8(op.as_slice()).unwrap_or("=>"));
            self.output.push(' ');
        } else {
            self.output.push(' ');
        }
        self.inline = true;
        self.visit(&node.value());
    }
}
