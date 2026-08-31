use crate::config::FormatConfig;
use ruby_prism::Visit;

pub fn format(source: &str) -> String {
    format_with_config(source, &FormatConfig::default())
}

pub fn format_with_config(source: &str, config: &FormatConfig) -> String {
    let result = ruby_prism::parse(source.as_bytes());
    let node = result.node();

    let mut formatter = Formatter::new(config);
    formatter.visit(&node);
    let trimmed = formatter.output.trim_end_matches('\n');
    let mut result = trimmed.to_string();
    result.push('\n');
    result
}

struct Formatter {
    output: String,
    indent: usize,
    inline: bool,
    indent_str: String,
}

impl Formatter {
    fn new(config: &FormatConfig) -> Self {
        Self {
            output: String::new(),
            indent: 0,
            inline: false,
            indent_str: " ".repeat(config.indent_size),
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str(&self.indent_str);
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
        self.inline = false;
        if !inline {
            self.write_indent();
        }
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
}

fn is_block_statement(node: &ruby_prism::Node) -> bool {
    matches!(
        node,
        ruby_prism::Node::ClassNode { .. }
            | ruby_prism::Node::DefNode { .. }
            | ruby_prism::Node::IfNode { .. }
            | ruby_prism::Node::UnlessNode { .. }
    )
}

impl Visit<'_> for Formatter {
    fn visit_statements_node(&mut self, node: &ruby_prism::StatementsNode<'_>) {
        let stmts = node.body();
        for (i, stmt) in stmts.iter().enumerate() {
            if i > 0 && self.indent == 0 && is_block_statement(&stmt) {
                self.ensure_blank_line();
            }
            self.inline = false;
            self.visit(&stmt);
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
            if name_str == "!" && node.arguments().is_none() {
                self.output.push('!');
                self.inline = true;
                self.visit(&receiver);
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
            if !inline {
                self.output.push('\n');
            }
        } else {
            if !inline {
                self.write_indent();
            }
            self.output.push_str(name_str);

            if let Some(args) = node.arguments() {
                self.output.push(' ');
                let arguments = args.arguments();
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.visit(&arg);
                }
            }

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

    fn visit_local_variable_read_node(&mut self, node: &ruby_prism::LocalVariableReadNode<'_>) {
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
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
        let name = node.name();
        self.output
            .push_str(std::str::from_utf8(name.as_slice()).unwrap_or(""));
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
        self.output.push_str("true");
    }

    fn visit_false_node(&mut self, _node: &ruby_prism::FalseNode<'_>) {
        self.output.push_str("false");
    }

    fn visit_nil_node(&mut self, _node: &ruby_prism::NilNode<'_>) {
        self.output.push_str("nil");
    }

    fn visit_integer_node(&mut self, node: &ruby_prism::IntegerNode<'_>) {
        let loc = node.location();
        let src = std::str::from_utf8(loc.as_slice()).unwrap_or("0");
        self.output.push_str(src);
    }

    fn visit_string_node(&mut self, node: &ruby_prism::StringNode<'_>) {
        self.output.push('"');
        self.output
            .push_str(std::str::from_utf8(node.unescaped()).unwrap_or(""));
        self.output.push('"');
    }
}
