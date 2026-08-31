use crate::config::FormatConfig;
use ruby_prism::Visit;

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

    fn emit_leading_comments_before(&mut self, line: usize) {
        while self.comment_cursor < self.comments.len()
            && self.line_of(self.comments[self.comment_cursor].start) < line
        {
            self.write_indent();
            self.output
                .push_str(&self.comments[self.comment_cursor].text);
            self.output.push('\n');
            self.comment_cursor += 1;
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
            self.output
                .push_str(&self.comments[self.comment_cursor].text);
            self.output.push('\n');
            self.comment_cursor += 1;
        }
    }

    fn flush_remaining_comments(&mut self) {
        while self.comment_cursor < self.comments.len() {
            self.write_indent();
            self.output
                .push_str(&self.comments[self.comment_cursor].text);
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
        write_target(self);
        self.output.push(' ');
        self.output.push_str(op);
        self.output.push(' ');
        self.inline = true;
        self.visit(value);
        if !inline {
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

fn is_block_statement(node: &ruby_prism::Node) -> bool {
    matches!(
        node,
        ruby_prism::Node::ClassNode { .. }
            | ruby_prism::Node::DefNode { .. }
            | ruby_prism::Node::IfNode { .. }
            | ruby_prism::Node::UnlessNode { .. }
            | ruby_prism::Node::CaseNode { .. }
            | ruby_prism::Node::WhileNode { .. }
            | ruby_prism::Node::UntilNode { .. }
            | ruby_prism::Node::ForNode { .. }
    )
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

        for (i, stmt) in stmts.iter().enumerate() {
            let start_line = self.line_of(stmt.location().start_offset());
            self.emit_leading_comments_before(start_line);

            if i > 0 && self.indent == 0 && is_block_statement(&stmt) {
                self.ensure_blank_line();
            }
            self.inline = false;
            self.visit(&stmt);

            let end_line = self.line_of(stmt.location().end_offset().saturating_sub(1));
            self.emit_trailing_comment_if_same_line(end_line);
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
            self.write_call_args(node, false);
            self.write_call_block(node);
            if !inline {
                self.output.push('\n');
            }
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

    fn visit_parameters_node(&mut self, node: &ruby_prism::ParametersNode<'_>) {
        let requireds = node.requireds();
        for (i, param) in requireds.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.visit(&param);
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

    fn visit_local_variable_read_node(&mut self, node: &ruby_prism::LocalVariableReadNode<'_>) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    fn visit_global_variable_read_node(&mut self, node: &ruby_prism::GlobalVariableReadNode<'_>) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    fn visit_instance_variable_read_node(
        &mut self,
        node: &ruby_prism::InstanceVariableReadNode<'_>,
    ) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    fn visit_class_variable_read_node(&mut self, node: &ruby_prism::ClassVariableReadNode<'_>) {
        let bytes = node.name().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    fn visit_self_node(&mut self, _node: &ruby_prism::SelfNode<'_>) {
        self.write_leaf(|f| f.output.push_str("self"));
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
        let bracketed = node.opening_loc().is_some();
        if bracketed {
            self.output.push('[');
        }
        for (i, element) in node.elements().iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&element);
        }
        if bracketed {
            self.output.push(']');
        }
    }

    fn visit_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) {
        self.write_indent();
        self.output.push_str("class ");
        self.inline = true;
        self.visit(&node.constant_path());

        if let Some(superclass) = node.superclass() {
            self.output.push_str(" < ");
            self.visit(&superclass);
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
        let is_elsif = keyword == Some(b"elsif");

        if !is_elsif {
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
        self.write_indent();
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
        self.write_indent();
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
        self.write_indent();
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
        self.write_indent();
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
        self.write_indent();
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
        self.inline = true;
        self.visit(&node.left());
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("&&");
        self.output.push(' ');
        self.output.push_str(op_str);
        self.output.push(' ');
        self.inline = true;
        self.visit(&node.right());
    }

    fn visit_or_node(&mut self, node: &ruby_prism::OrNode<'_>) {
        self.inline = true;
        self.visit(&node.left());
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("||");
        self.output.push(' ');
        self.output.push_str(op_str);
        self.output.push(' ');
        self.inline = true;
        self.visit(&node.right());
    }

    fn visit_true_node(&mut self, _node: &ruby_prism::TrueNode<'_>) {
        self.write_leaf(|f| f.output.push_str("true"));
    }

    fn visit_false_node(&mut self, _node: &ruby_prism::FalseNode<'_>) {
        self.write_leaf(|f| f.output.push_str("false"));
    }

    fn visit_nil_node(&mut self, _node: &ruby_prism::NilNode<'_>) {
        self.write_leaf(|f| f.output.push_str("nil"));
    }

    fn visit_integer_node(&mut self, node: &ruby_prism::IntegerNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    fn visit_string_node(&mut self, node: &ruby_prism::StringNode<'_>) {
        let unescaped = node.unescaped();
        self.write_leaf(|f| {
            f.output.push('"');
            f.write_bytes(unescaped);
            f.output.push('"');
        });
    }

    fn visit_symbol_node(&mut self, node: &ruby_prism::SymbolNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
    }

    fn visit_float_node(&mut self, node: &ruby_prism::FloatNode<'_>) {
        let bytes = node.location().as_slice();
        self.write_leaf(|f| f.write_bytes(bytes));
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
        if node.elements().iter().next().is_none() {
            self.output.push_str("{}");
            return;
        }
        self.output.push_str("{ ");
        for (i, element) in node.elements().iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&element);
        }
        self.output.push_str(" }");
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
