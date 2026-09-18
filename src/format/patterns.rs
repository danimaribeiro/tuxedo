//! Pattern-matching nodes that only occur inside `in`/`=>` patterns, plus
//! the handful of keyword constructs (`defined?`, `undef`, `BEGIN`/`END`,
//! flip-flop) that carry a keyword of their own and nothing else.
//!
//! `fmt_array_pattern_node`/`fmt_hash_pattern_node` live in `collections`
//! next to the literals they mirror.
//!
//! All of these but `BEGIN`/`END` are a keyword or operator with one or
//! two children joined by a fixed separator, so each is an [`InlineRun`]
//! over `run_node` of its children and carries no line point of its own.
//! What the `Doc` buys them is their *children*: one that carries line
//! points (an adjacent-string-literal concatenation, say) is now laid out
//! by the enclosing group, which can see what precedes it on the line,
//! instead of being rendered on its own and pasted in.
//!
//! [`InlineRun`]: super::InlineRun

use super::Formatter;
use super::doc::Doc;

impl Formatter {
    /// `Integer | Float`
    pub(super) fn fmt_alternation_pattern_node(
        &mut self,
        node: &ruby_prism::AlternationPatternNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, &node.left());
        self.run_text(&mut run, " | ");
        self.run_node(&mut run, &node.right());
        self.end_run(run)
    }

    /// `Integer => n`
    pub(super) fn fmt_capture_pattern_node(
        &mut self,
        node: &ruby_prism::CapturePatternNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, &node.value());
        self.run_text(&mut run, " => ");
        self.run_node(&mut run, &node.target().as_node());
        self.end_run(run)
    }

    /// `^var`
    pub(super) fn fmt_pinned_variable_node(
        &mut self,
        node: &ruby_prism::PinnedVariableNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, "^");
        self.run_node(&mut run, &node.variable());
        self.end_run(run)
    }

    /// `^(expression)`
    pub(super) fn fmt_pinned_expression_node(
        &mut self,
        node: &ruby_prism::PinnedExpressionNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, "^(");
        self.run_node(&mut run, &node.expression());
        self.run_text(&mut run, ")");
        self.end_run(run)
    }

    /// `[*, needle, *post]` / `Const[*, needle, *]` / `Const(*, needle, *)`
    pub(super) fn fmt_find_pattern_node(&mut self, node: &ruby_prism::FindPatternNode<'_>) -> Doc {
        let opening = node
            .opening_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"[".to_vec());
        let closing = node
            .closing_loc()
            .map(|l| l.as_slice().to_vec())
            .unwrap_or_else(|| b"]".to_vec());

        let mut run = self.begin_run();
        if let Some(constant) = node.constant() {
            self.run_node(&mut run, &constant);
        }
        self.run_bytes(&mut run, &opening);
        self.run_node(&mut run, &node.left().as_node());
        for required in node.requireds().iter() {
            self.run_text(&mut run, ", ");
            self.run_node(&mut run, &required);
        }
        self.run_text(&mut run, ", ");
        self.run_node(&mut run, &node.right());
        self.run_bytes(&mut run, &closing);
        self.end_run(run)
    }

    /// `(a..b)` as a condition: Ruby's flip-flop. Same shape as a range.
    pub(super) fn fmt_flip_flop_node(&mut self, node: &ruby_prism::FlipFlopNode<'_>) -> Doc {
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

    /// `defined?(x)` / `defined? x`. Parentheses are kept as written: they
    /// are not optional in general (`defined? a && b` asks about `a && b`,
    /// `defined?(a) && b` does not).
    pub(super) fn fmt_defined_node(&mut self, node: &ruby_prism::DefinedNode<'_>) -> Doc {
        let parenthesized = node.lparen_loc().is_some();
        let mut run = self.begin_run();
        self.run_text(
            &mut run,
            if parenthesized {
                "defined?("
            } else {
                "defined? "
            },
        );
        self.run_node(&mut run, &node.value());
        if parenthesized {
            self.run_text(&mut run, ")");
        }
        self.end_run(run)
    }

    /// `undef foo, :bar`
    pub(super) fn fmt_undef_node(&mut self, node: &ruby_prism::UndefNode<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_text(&mut run, "undef ");
        for (i, name) in node.names().iter().enumerate() {
            if i > 0 {
                self.run_text(&mut run, ", ");
            }
            self.run_node(&mut run, &name);
        }
        self.end_run(run)
    }

    /// `BEGIN { ... }`
    pub(super) fn fmt_pre_execution_node(
        &mut self,
        node: &ruby_prism::PreExecutionNode<'_>,
    ) -> Doc {
        self.spliced_doc(|f| f.write_execution_block("BEGIN", node.statements()))
    }

    /// `END { ... }`
    pub(super) fn fmt_post_execution_node(
        &mut self,
        node: &ruby_prism::PostExecutionNode<'_>,
    ) -> Doc {
        self.spliced_doc(|f| f.write_execution_block("END", node.statements()))
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
            self.write_statements_node(&statements);
            self.single_line = single_line;
            self.output.push_str(" }");
            return;
        }

        self.write_indent();
        self.output.push_str(keyword);
        self.output.push_str(" {\n");
        self.indent += 1;
        self.write_statements_node(&statements);
        self.indent -= 1;
        self.write_indent();
        self.output.push_str("}\n");
    }

    /// A constant assignment under a `# shareable_constant_value:` magic
    /// comment. The wrapper has no text of its own.
    pub(super) fn fmt_shareable_constant_node(
        &mut self,
        node: &ruby_prism::ShareableConstantNode<'_>,
    ) -> Doc {
        self.node_doc(&node.write())
    }
}
