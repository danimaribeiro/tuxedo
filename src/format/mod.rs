use crate::config::FormatConfig;
use ruby_prism::Visit;

mod assignment;
mod calls;
mod classes;
mod collections;
mod control_flow;
mod defs;
mod literals;
mod statements;
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
    #[allow(dead_code)] // wired up by the array-literal Doc pilot, next commit
    line_length: usize,
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
            line_length: config.line_length,
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

    /// The display width of the current line of `output` rendered so far,
    /// i.e. where the next character would land. Derived on demand (rather
    /// than tracked incrementally) so there's no counter that can drift out
    /// of sync with `output`.
    #[allow(dead_code)] // wired up by the array-literal Doc pilot, next commit
    fn current_column(&self) -> usize {
        let line_start = self.output.rfind('\n').map_or(0, |i| i + 1);
        unicode_width::UnicodeWidthStr::width(&self.output[line_start..])
    }

    /// Renders a `Doc` value to a string, using the width still available on
    /// the current output line before `line_length` is hit.
    #[allow(dead_code)] // wired up by the array-literal Doc pilot, next commit
    fn render_doc_to_string(&self, doc: pretty::RcDoc<'static, ()>) -> String {
        let width = self.line_length.saturating_sub(self.current_column());
        doc.pretty(width).to_string()
    }

    /// Captures whatever `render` writes to `self.output` into an owned
    /// string, restoring the real `output` afterwards, and wraps the capture
    /// as an opaque `Doc::text` leaf. This is how content that hasn't been
    /// ported to build `Doc` values natively (i.e. everything, today) still
    /// renders correctly inside a `Doc`-based construct: it's rendered once,
    /// the normal way, and treated as an unsplittable block of text.
    #[allow(dead_code)] // wired up by the array-literal Doc pilot, next commit
    fn capture_as_doc(&mut self, render: impl FnOnce(&mut Self)) -> pretty::RcDoc<'static, ()> {
        let saved = std::mem::take(&mut self.output);
        render(self);
        let captured = std::mem::replace(&mut self.output, saved);
        pretty::RcDoc::text(captured)
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
        self.fmt_statements_node(node);
    }

    fn visit_call_node(&mut self, node: &ruby_prism::CallNode<'_>) {
        self.fmt_call_node(node);
    }

    fn visit_super_node(&mut self, node: &ruby_prism::SuperNode<'_>) {
        self.fmt_super_node(node);
    }

    fn visit_forwarding_super_node(&mut self, node: &ruby_prism::ForwardingSuperNode<'_>) {
        self.fmt_forwarding_super_node(node);
    }

    fn visit_def_node(&mut self, node: &ruby_prism::DefNode<'_>) {
        self.fmt_def_node(node);
    }

    fn visit_required_parameter_node(&mut self, node: &ruby_prism::RequiredParameterNode<'_>) {
        self.fmt_required_parameter_node(node);
    }

    fn visit_optional_parameter_node(&mut self, node: &ruby_prism::OptionalParameterNode<'_>) {
        self.fmt_optional_parameter_node(node);
    }

    fn visit_rest_parameter_node(&mut self, node: &ruby_prism::RestParameterNode<'_>) {
        self.fmt_rest_parameter_node(node);
    }

    fn visit_implicit_rest_node(&mut self, node: &ruby_prism::ImplicitRestNode<'_>) {
        self.fmt_implicit_rest_node(node);
    }

    fn visit_required_keyword_parameter_node(
        &mut self,
        node: &ruby_prism::RequiredKeywordParameterNode<'_>,
    ) {
        self.fmt_required_keyword_parameter_node(node);
    }

    fn visit_optional_keyword_parameter_node(
        &mut self,
        node: &ruby_prism::OptionalKeywordParameterNode<'_>,
    ) {
        self.fmt_optional_keyword_parameter_node(node);
    }

    fn visit_keyword_rest_parameter_node(
        &mut self,
        node: &ruby_prism::KeywordRestParameterNode<'_>,
    ) {
        self.fmt_keyword_rest_parameter_node(node);
    }

    fn visit_no_keywords_parameter_node(&mut self, node: &ruby_prism::NoKeywordsParameterNode<'_>) {
        self.fmt_no_keywords_parameter_node(node);
    }

    fn visit_forwarding_parameter_node(&mut self, node: &ruby_prism::ForwardingParameterNode<'_>) {
        self.fmt_forwarding_parameter_node(node);
    }

    fn visit_block_parameter_node(&mut self, node: &ruby_prism::BlockParameterNode<'_>) {
        self.fmt_block_parameter_node(node);
    }

    fn visit_parameters_node(&mut self, node: &ruby_prism::ParametersNode<'_>) {
        self.fmt_parameters_node(node);
    }

    fn visit_block_parameters_node(&mut self, node: &ruby_prism::BlockParametersNode<'_>) {
        self.fmt_block_parameters_node(node);
    }

    fn visit_block_node(&mut self, node: &ruby_prism::BlockNode<'_>) {
        self.fmt_block_node(node);
    }

    fn visit_yield_node(&mut self, node: &ruby_prism::YieldNode<'_>) {
        self.fmt_yield_node(node);
    }

    fn visit_return_node(&mut self, node: &ruby_prism::ReturnNode<'_>) {
        self.fmt_return_node(node);
    }

    fn visit_break_node(&mut self, node: &ruby_prism::BreakNode<'_>) {
        self.fmt_break_node(node);
    }

    fn visit_next_node(&mut self, node: &ruby_prism::NextNode<'_>) {
        self.fmt_next_node(node);
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
        self.fmt_local_variable_write_node(node);
    }

    fn visit_local_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOperatorWriteNode<'_>,
    ) {
        self.fmt_local_variable_operator_write_node(node);
    }

    fn visit_local_variable_or_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOrWriteNode<'_>,
    ) {
        self.fmt_local_variable_or_write_node(node);
    }

    fn visit_local_variable_and_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableAndWriteNode<'_>,
    ) {
        self.fmt_local_variable_and_write_node(node);
    }

    fn visit_instance_variable_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableWriteNode<'_>,
    ) {
        self.fmt_instance_variable_write_node(node);
    }

    fn visit_instance_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOperatorWriteNode<'_>,
    ) {
        self.fmt_instance_variable_operator_write_node(node);
    }

    fn visit_instance_variable_or_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOrWriteNode<'_>,
    ) {
        self.fmt_instance_variable_or_write_node(node);
    }

    fn visit_instance_variable_and_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableAndWriteNode<'_>,
    ) {
        self.fmt_instance_variable_and_write_node(node);
    }

    fn visit_constant_write_node(&mut self, node: &ruby_prism::ConstantWriteNode<'_>) {
        self.fmt_constant_write_node(node);
    }

    fn visit_constant_operator_write_node(
        &mut self,
        node: &ruby_prism::ConstantOperatorWriteNode<'_>,
    ) {
        self.fmt_constant_operator_write_node(node);
    }

    fn visit_constant_or_write_node(&mut self, node: &ruby_prism::ConstantOrWriteNode<'_>) {
        self.fmt_constant_or_write_node(node);
    }

    fn visit_constant_and_write_node(&mut self, node: &ruby_prism::ConstantAndWriteNode<'_>) {
        self.fmt_constant_and_write_node(node);
    }

    fn visit_global_variable_write_node(&mut self, node: &ruby_prism::GlobalVariableWriteNode<'_>) {
        self.fmt_global_variable_write_node(node);
    }

    fn visit_global_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableOperatorWriteNode<'_>,
    ) {
        self.fmt_global_variable_operator_write_node(node);
    }

    fn visit_global_variable_or_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableOrWriteNode<'_>,
    ) {
        self.fmt_global_variable_or_write_node(node);
    }

    fn visit_global_variable_and_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableAndWriteNode<'_>,
    ) {
        self.fmt_global_variable_and_write_node(node);
    }

    fn visit_class_variable_write_node(&mut self, node: &ruby_prism::ClassVariableWriteNode<'_>) {
        self.fmt_class_variable_write_node(node);
    }

    fn visit_class_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableOperatorWriteNode<'_>,
    ) {
        self.fmt_class_variable_operator_write_node(node);
    }

    fn visit_class_variable_or_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableOrWriteNode<'_>,
    ) {
        self.fmt_class_variable_or_write_node(node);
    }

    fn visit_class_variable_and_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableAndWriteNode<'_>,
    ) {
        self.fmt_class_variable_and_write_node(node);
    }

    fn visit_constant_path_write_node(&mut self, node: &ruby_prism::ConstantPathWriteNode<'_>) {
        self.fmt_constant_path_write_node(node);
    }

    fn visit_constant_path_operator_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathOperatorWriteNode<'_>,
    ) {
        self.fmt_constant_path_operator_write_node(node);
    }

    fn visit_constant_path_or_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathOrWriteNode<'_>,
    ) {
        self.fmt_constant_path_or_write_node(node);
    }

    fn visit_constant_path_and_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathAndWriteNode<'_>,
    ) {
        self.fmt_constant_path_and_write_node(node);
    }

    fn visit_multi_write_node(&mut self, node: &ruby_prism::MultiWriteNode<'_>) {
        self.fmt_multi_write_node(node);
    }

    fn visit_local_variable_target_node(&mut self, node: &ruby_prism::LocalVariableTargetNode<'_>) {
        self.fmt_local_variable_target_node(node);
    }

    fn visit_splat_node(&mut self, node: &ruby_prism::SplatNode<'_>) {
        self.fmt_splat_node(node);
    }

    fn visit_block_argument_node(&mut self, node: &ruby_prism::BlockArgumentNode<'_>) {
        self.fmt_block_argument_node(node);
    }

    fn visit_array_node(&mut self, node: &ruby_prism::ArrayNode<'_>) {
        self.fmt_array_node(node);
    }

    fn visit_class_node(&mut self, node: &ruby_prism::ClassNode<'_>) {
        self.fmt_class_node(node);
    }

    fn visit_module_node(&mut self, node: &ruby_prism::ModuleNode<'_>) {
        self.fmt_module_node(node);
    }

    fn visit_constant_path_node(&mut self, node: &ruby_prism::ConstantPathNode<'_>) {
        self.fmt_constant_path_node(node);
    }

    fn visit_constant_read_node(&mut self, node: &ruby_prism::ConstantReadNode<'_>) {
        self.fmt_constant_read_node(node);
    }

    fn visit_if_node(&mut self, node: &ruby_prism::IfNode<'_>) {
        self.fmt_if_node(node);
    }

    fn visit_else_node(&mut self, node: &ruby_prism::ElseNode<'_>) {
        self.fmt_else_node(node);
    }

    fn visit_unless_node(&mut self, node: &ruby_prism::UnlessNode<'_>) {
        self.fmt_unless_node(node);
    }

    fn visit_while_node(&mut self, node: &ruby_prism::WhileNode<'_>) {
        self.fmt_while_node(node);
    }

    fn visit_until_node(&mut self, node: &ruby_prism::UntilNode<'_>) {
        self.fmt_until_node(node);
    }

    fn visit_for_node(&mut self, node: &ruby_prism::ForNode<'_>) {
        self.fmt_for_node(node);
    }

    fn visit_case_node(&mut self, node: &ruby_prism::CaseNode<'_>) {
        self.fmt_case_node(node);
    }

    fn visit_when_node(&mut self, node: &ruby_prism::WhenNode<'_>) {
        self.fmt_when_node(node);
    }

    fn visit_and_node(&mut self, node: &ruby_prism::AndNode<'_>) {
        self.fmt_and_node(node);
    }

    fn visit_or_node(&mut self, node: &ruby_prism::OrNode<'_>) {
        self.fmt_or_node(node);
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
        self.fmt_range_node(node);
    }

    fn visit_hash_node(&mut self, node: &ruby_prism::HashNode<'_>) {
        self.fmt_hash_node(node);
    }

    fn visit_assoc_node(&mut self, node: &ruby_prism::AssocNode<'_>) {
        self.fmt_assoc_node(node);
    }
}
