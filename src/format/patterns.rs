//! Pattern-matching nodes that only occur inside `in`/`=>` patterns, plus
//! the handful of keyword constructs (`defined?`, `undef`, `BEGIN`/`END`,
//! flip-flop) that used to fall through to the verbatim fallback.
//!
//! `fmt_array_pattern_node`/`fmt_hash_pattern_node` live in `collections`
//! next to the literals they mirror.

use super::Formatter;

impl Formatter {
    /// `Integer | Float`
    pub(super) fn fmt_alternation_pattern_node(
        &mut self,
        node: &ruby_prism::AlternationPatternNode<'_>,
    ) {
        self.write_leaf(|f| {
            f.inline = true;
            f.visit(&node.left());
            f.output.push_str(" | ");
            f.inline = true;
            f.visit(&node.right());
        });
    }

    /// `Integer => n`
    pub(super) fn fmt_capture_pattern_node(&mut self, node: &ruby_prism::CapturePatternNode<'_>) {
        self.write_leaf(|f| {
            f.inline = true;
            f.visit(&node.value());
            f.output.push_str(" => ");
            f.inline = true;
            f.visit(&node.target().as_node());
        });
    }

    /// `^var`
    pub(super) fn fmt_pinned_variable_node(&mut self, node: &ruby_prism::PinnedVariableNode<'_>) {
        self.write_leaf(|f| {
            f.output.push('^');
            f.inline = true;
            f.visit(&node.variable());
        });
    }

    /// `^(expression)`
    pub(super) fn fmt_pinned_expression_node(
        &mut self,
        node: &ruby_prism::PinnedExpressionNode<'_>,
    ) {
        self.write_leaf(|f| {
            f.output.push_str("^(");
            f.inline = true;
            f.visit(&node.expression());
            f.output.push(')');
        });
    }

    /// `[*, needle, *post]` / `Const[*, needle, *]` / `Const(*, needle, *)`
    pub(super) fn fmt_find_pattern_node(&mut self, node: &ruby_prism::FindPatternNode<'_>) {
        self.write_leaf(|f| {
            if let Some(constant) = node.constant() {
                f.inline = true;
                f.visit(&constant);
            }
            let opening = node
                .opening_loc()
                .map(|l| l.as_slice().to_vec())
                .unwrap_or_else(|| b"[".to_vec());
            let closing = node
                .closing_loc()
                .map(|l| l.as_slice().to_vec())
                .unwrap_or_else(|| b"]".to_vec());
            f.write_bytes(&opening);
            f.inline = true;
            f.visit(&node.left().as_node());
            for required in node.requireds().iter() {
                f.output.push_str(", ");
                f.inline = true;
                f.visit(&required);
            }
            f.output.push_str(", ");
            f.inline = true;
            f.visit(&node.right());
            f.write_bytes(&closing);
        });
    }

    /// `(a..b)` as a condition: Ruby's flip-flop. Same shape as a range.
    pub(super) fn fmt_flip_flop_node(&mut self, node: &ruby_prism::FlipFlopNode<'_>) {
        self.write_leaf(|f| {
            if let Some(left) = node.left() {
                f.inline = true;
                f.visit(&left);
            }
            let op = node.operator_loc().as_slice().to_vec();
            f.write_bytes(&op);
            if let Some(right) = node.right() {
                f.inline = true;
                f.visit(&right);
            }
        });
    }

    /// `defined?(x)` / `defined? x`. Parentheses are kept as written: they
    /// are not optional in general (`defined? a && b` asks about `a && b`,
    /// `defined?(a) && b` does not).
    pub(super) fn fmt_defined_node(&mut self, node: &ruby_prism::DefinedNode<'_>) {
        self.write_leaf(|f| {
            let parenthesized = node.lparen_loc().is_some();
            f.output.push_str(if parenthesized {
                "defined?("
            } else {
                "defined? "
            });
            f.inline = true;
            f.visit(&node.value());
            if parenthesized {
                f.output.push(')');
            }
        });
    }

    /// `undef foo, :bar`
    pub(super) fn fmt_undef_node(&mut self, node: &ruby_prism::UndefNode<'_>) {
        self.write_leaf(|f| {
            f.output.push_str("undef ");
            for (i, name) in node.names().iter().enumerate() {
                if i > 0 {
                    f.output.push_str(", ");
                }
                f.inline = true;
                f.visit(&name);
            }
        });
    }

    /// `BEGIN { ... }`
    pub(super) fn fmt_pre_execution_node(&mut self, node: &ruby_prism::PreExecutionNode<'_>) {
        self.write_execution_block("BEGIN", node.statements());
    }

    /// `END { ... }`
    pub(super) fn fmt_post_execution_node(&mut self, node: &ruby_prism::PostExecutionNode<'_>) {
        self.write_execution_block("END", node.statements());
    }

    /// `BEGIN`/`END` only take braces (never `do...end`), and unlike a
    /// method's `{ }` block they are statements in their own right, so the
    /// body is laid out like any other block of statements.
    fn write_execution_block(
        &mut self,
        keyword: &str,
        statements: Option<ruby_prism::StatementsNode<'_>>,
    ) {
        let Some(statements) = statements else {
            self.write_leaf(|f| {
                f.output.push_str(keyword);
                f.output.push_str(" {}");
            });
            return;
        };

        if self.inline || self.single_line {
            self.output.push_str(keyword);
            self.output.push_str(" { ");
            let single_line = std::mem::replace(&mut self.single_line, true);
            self.fmt_statements_node(&statements);
            self.single_line = single_line;
            self.output.push_str(" }");
            return;
        }

        self.write_indent();
        self.output.push_str(keyword);
        self.output.push_str(" {\n");
        self.indent += 1;
        self.fmt_statements_node(&statements);
        self.indent -= 1;
        self.write_indent();
        self.output.push_str("}\n");
    }

    /// A constant assignment under a `# shareable_constant_value:` magic
    /// comment. The wrapper has no text of its own.
    pub(super) fn fmt_shareable_constant_node(
        &mut self,
        node: &ruby_prism::ShareableConstantNode<'_>,
    ) {
        self.visit(&node.write());
    }
}
