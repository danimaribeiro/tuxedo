use super::Formatter;
use super::doc::Doc;

impl Formatter {
    /// `value => pattern` -- the "rightward assignment" one-line pattern
    /// match that raises `NoMatchingPatternError` on failure (as opposed to
    /// `MatchPredicateNode`'s `in`, which returns `false`).
    pub(super) fn fmt_match_required_node(
        &mut self,
        node: &ruby_prism::MatchRequiredNode<'_>,
    ) -> Doc {
        self.one_line_match_doc(&node.value(), " => ", &node.pattern())
    }

    /// `value in pattern` -- the boolean-returning one-line pattern match.
    pub(super) fn fmt_match_predicate_node(
        &mut self,
        node: &ruby_prism::MatchPredicateNode<'_>,
    ) -> Doc {
        self.one_line_match_doc(&node.value(), " in ", &node.pattern())
    }

    /// `value => pattern` / `value in pattern`: both are a value, a
    /// keyword and a pattern on one line, with no line point of their own.
    fn one_line_match_doc(
        &mut self,
        value: &ruby_prism::Node<'_>,
        keyword: &str,
        pattern: &ruby_prism::Node<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, value);
        self.run_text(&mut run, keyword);
        self.run_node(&mut run, pattern);
        self.end_run(run)
    }

    fn write_if_node(&mut self, node: &ruby_prism::IfNode<'_>) {
        let keyword = node.if_keyword_loc().map(|k| k.as_slice());
        let is_ternary = keyword.is_none();
        let is_elsif = keyword == Some(b"elsif");
        let is_modifier = !is_ternary && !is_elsif && node.end_keyword_loc().is_none();

        if is_ternary {
            self.fmt_ternary(node);
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
        let single_line = self.single_line;
        if !is_elsif && !inline {
            self.write_indent();
        }
        self.output
            .push_str(if is_elsif { "elsif " } else { "if " });
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            let header_line =
                self.line_of(node.predicate().location().end_offset().saturating_sub(1));
            self.emit_trailing_comment_if_same_line(header_line);
            self.indent += 1;
        }

        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        if !single_line {
            // A comment above `elsif`/`else`/`end` belongs to the branch
            // it sits at the bottom of.
            let closer = node
                .subsequent()
                .map(|s| s.location().start_offset())
                .or_else(|| node.end_keyword_loc().map(|e| e.start_offset()));
            if let Some(closer) = closer {
                self.flush_comments_before(closer, node.statements().is_some());
            }
            self.indent -= 1;
        }

        if let Some(consequent) = node.subsequent() {
            if single_line {
                self.output.push_str("; ");
            } else {
                self.write_indent();
            }
            // An elsif/else terminates the same chain this `if` opened, so
            // it needs the chain's *original* inline-ness to decide whether
            // its own trailing "end" gets a newline -- not `self.inline`,
            // which was just forced to false above for the predicate/body.
            self.inline = inline;
            self.visit(&consequent);
        } else if single_line {
            self.output.push_str("; end");
            if !inline {
                self.output.push('\n');
            }
        } else {
            self.write_indent();
            self.output.push_str("end");
            if !inline {
                self.output.push('\n');
            }
        }
    }

    fn write_else_node(&mut self, node: &ruby_prism::ElseNode<'_>) {
        // Whether the if/case chain this closes is itself being rendered
        // inline (as a sub-expression, e.g. a call argument) -- see
        // fmt_if_node, which restores this into self.inline before
        // recursing here for exactly this reason. A captured inline value
        // must not carry a hardcoded trailing newline: joining it with more
        // content on the same Doc/text line (a following `, arg)`) would
        // otherwise leave that content dangling on its own line, which
        // isn't just ugly -- for a call argument it's invalid Ruby.
        let inline = self.inline;
        let single_line = self.single_line;
        if single_line {
            self.output.push_str("else; ");
        } else {
            self.output.push_str("else\n");
            let header_line = self.line_of(node.else_keyword_loc().start_offset());
            self.emit_trailing_comment_if_same_line(header_line);
            self.indent += 1;
        }

        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }

        if single_line {
            self.output.push_str("; end");
        } else {
            if let Some(end_keyword) = node.end_keyword_loc() {
                self.flush_comments_before(end_keyword.start_offset(), node.statements().is_some());
            }
            self.indent -= 1;
            self.write_indent();
            self.output.push_str("end");
        }
        if !inline {
            self.output.push('\n');
        }
    }

    /// Renders `begin ... end`, including any attached `rescue`/`else`/
    /// `ensure` clauses. Also reached with no `begin`/`end` keywords at all:
    /// prism wraps a method/block body in a `BeginNode` whenever it has a
    /// `rescue`/`else`/`ensure` clause (e.g. `def foo; rescue; end`), in
    /// which case the enclosing `def`/block already writes its own
    /// keyword/indent/`end` and this only needs to render the clauses.
    fn write_begin_node(&mut self, node: &ruby_prism::BeginNode<'_>) {
        let has_keyword = node.begin_keyword_loc().is_some();
        let inline = self.inline;
        // Set by fmt_block_node when rendering a brace block's body flat
        // (`{ ... }` rather than `do...end`). A `begin`/`rescue`/`ensure`
        // here still has to produce *something* on that one line -- join
        // clauses with "; " instead of real indentation/newlines, the same
        // compact style fmt_statements_node already uses for its own
        // statements in this mode. Without this, a block like
        // `foo { begin; risky; rescue => e; handle; end }` rendered its
        // "rescue"/"end" with real newlines while everything around it
        // stayed on one line, corrupting the output.
        let single_line = self.single_line;

        if has_keyword {
            if !inline {
                self.write_indent();
            }
            self.output.push_str("begin");
            if single_line {
                self.output.push_str("; ");
            } else {
                self.output.push('\n');
                if let Some(begin_keyword) = node.begin_keyword_loc() {
                    self.emit_trailing_comment_if_same_line(
                        self.line_of(begin_keyword.start_offset()),
                    );
                }
                self.indent += 1;
            }
        }

        if let Some(statements) = node.statements() {
            self.fmt_statements_node(&statements);
        }

        // Where each section of the body stops: a comment above `rescue`/
        // `else`/`ensure`/`end` stays at the bottom of the section before
        // it. An implicit `begin` (a `def`/block body with a `rescue`) has
        // no `end` of its own; whoever owns that `end` flushes the last
        // section.
        let end_offset = node.end_keyword_loc().map(|l| l.start_offset());
        let ensure_offset = node
            .ensure_clause()
            .map(|e| e.location().start_offset())
            .or(end_offset);
        let else_offset = node
            .else_clause()
            .map(|e| e.location().start_offset())
            .or(ensure_offset);
        let rescue_offset = node
            .rescue_clause()
            .map(|r| r.location().start_offset())
            .or(else_offset);
        if let Some(closer) = rescue_offset {
            self.flush_comments_before(closer, node.statements().is_some());
        }

        if has_keyword && !single_line {
            self.indent -= 1;
        }

        let has_clauses = node.rescue_clause().is_some()
            || node.else_clause().is_some()
            || node.ensure_clause().is_some();
        if has_clauses {
            // rescue/else/ensure always align with the keyword that opens
            // the body they guard. For an explicit `begin` we've already
            // dedented back to its level above; for an implicit one (no
            // `begin`/`end` here at all) `self.indent` is still one level
            // deeper — whatever the enclosing def/block set it to for the
            // body — so dedent by one to reach that keyword's own level.
            let dedent = usize::from(!has_keyword);
            if !single_line {
                self.indent -= dedent;
            }

            if let Some(rescue) = node.rescue_clause() {
                if single_line {
                    self.output.push_str("; ");
                }
                self.write_rescue_clause(&rescue, else_offset);
            }
            if let Some(else_clause) = node.else_clause() {
                if single_line {
                    self.output.push_str("; else; ");
                } else {
                    self.write_indent();
                    self.output.push_str("else\n");
                    self.emit_trailing_comment_if_same_line(
                        self.line_of(else_clause.else_keyword_loc().start_offset()),
                    );
                    self.indent += 1;
                }
                if let Some(statements) = else_clause.statements() {
                    self.fmt_statements_node(&statements);
                }
                if !single_line {
                    if let Some(closer) = ensure_offset {
                        self.flush_comments_before(closer, else_clause.statements().is_some());
                    }
                    self.indent -= 1;
                }
            }
            if let Some(ensure) = node.ensure_clause() {
                if single_line {
                    self.output.push_str("; ");
                }
                self.write_ensure_node(&ensure);
            }

            if !single_line {
                self.indent += dedent;
            }
        }

        if has_keyword {
            if single_line {
                self.output.push_str("; end");
            } else {
                self.write_indent();
                self.output.push_str("end");
            }
            if !inline {
                self.output.push('\n');
            }
        }
    }

    /// `expr rescue fallback` -- the modifier form, as opposed to an
    /// explicit `begin`/`rescue` block (`fmt_rescue_node`). Prism's default
    /// visitor for this node just visits `expression()` then
    /// `rescue_expression()` back to back with no separator, which is why
    /// this needs its own handler rather than falling through: without the
    /// literal " rescue " keyword between them, a statement like
    /// `transmit(...) rescue nil` followed by more code lost the boundary
    /// between "nil" and the next line entirely.
    fn write_rescue_modifier_node(&mut self, node: &ruby_prism::RescueModifierNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }
        self.inline = true;
        self.visit(&node.expression());
        self.output.push_str(" rescue ");
        self.inline = true;
        self.visit(&node.rescue_expression());
        if !inline {
            self.output.push('\n');
        }
    }

    pub(super) fn fmt_rescue_node(&mut self, node: &ruby_prism::RescueNode<'_>) -> Doc {
        self.spliced_doc(|f| f.write_rescue_clause(node, None))
    }

    pub(super) fn fmt_ensure_node(&mut self, node: &ruby_prism::EnsureNode<'_>) -> Doc {
        self.spliced_doc(|f| f.write_ensure_node(node))
    }

    /// `closer` is where the last `rescue` of the chain stops (the start
    /// of the `else`/`ensure`/`end` that follows), if the caller knows it.
    fn write_rescue_clause(&mut self, node: &ruby_prism::RescueNode<'_>, closer: Option<usize>) {
        let single_line = self.single_line;
        if !single_line {
            self.write_indent();
        }
        self.output.push_str("rescue");

        let exceptions: Vec<_> = node.exceptions().iter().collect();
        for (i, exception) in exceptions.iter().enumerate() {
            self.output.push_str(if i == 0 { " " } else { ", " });
            self.inline = true;
            self.visit(exception);
        }

        if let Some(reference) = node.reference() {
            self.output.push_str(" => ");
            self.inline = true;
            self.visit(&reference);
        }

        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            let header_end = node
                .reference()
                .map(|r| r.location().end_offset())
                .or_else(|| {
                    node.exceptions()
                        .iter()
                        .last()
                        .map(|e| e.location().end_offset())
                })
                .unwrap_or_else(|| node.keyword_loc().end_offset());
            self.emit_trailing_comment_if_same_line(self.line_of(header_end.saturating_sub(1)));
            self.indent += 1;
        }

        if let Some(statements) = node.statements() {
            self.fmt_statements_node(&statements);
        }

        if !single_line {
            let section_end = node
                .subsequent()
                .map(|s| s.location().start_offset())
                .or(closer);
            if let Some(section_end) = section_end {
                self.flush_comments_before(section_end, node.statements().is_some());
            }
            self.indent -= 1;
        }

        if let Some(subsequent) = node.subsequent() {
            if single_line {
                self.output.push_str("; ");
            }
            self.write_rescue_clause(&subsequent, closer);
        }
    }

    fn write_ensure_node(&mut self, node: &ruby_prism::EnsureNode<'_>) {
        let single_line = self.single_line;
        if !single_line {
            self.write_indent();
        }
        self.output.push_str("ensure");
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            self.emit_trailing_comment_if_same_line(
                self.line_of(node.ensure_keyword_loc().start_offset()),
            );
            self.indent += 1;
        }

        if let Some(statements) = node.statements() {
            self.fmt_statements_node(&statements);
        }

        if !single_line {
            self.flush_comments_before(
                node.end_keyword_loc().start_offset(),
                node.statements().is_some(),
            );
            self.indent -= 1;
        }
    }

    fn write_unless_node(&mut self, node: &ruby_prism::UnlessNode<'_>) {
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
        let single_line = self.single_line;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("unless ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            let header_line =
                self.line_of(node.predicate().location().end_offset().saturating_sub(1));
            self.emit_trailing_comment_if_same_line(header_line);
            self.indent += 1;
        }

        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        if !single_line {
            let closer = node
                .else_clause()
                .map(|e| e.location().start_offset())
                .or_else(|| node.end_keyword_loc().map(|e| e.start_offset()));
            if let Some(closer) = closer {
                self.flush_comments_before(closer, node.statements().is_some());
            }
            self.indent -= 1;
        }

        if let Some(consequent) = node.else_clause() {
            if single_line {
                self.output.push_str("; ");
            } else {
                self.write_indent();
            }
            self.inline = inline;
            self.write_else_node(&consequent);
        } else if single_line {
            self.output.push_str("; end");
            if !inline {
                self.output.push('\n');
            }
        } else {
            self.write_indent();
            self.output.push_str("end");
            if !inline {
                self.output.push('\n');
            }
        }
    }

    fn write_while_node(&mut self, node: &ruby_prism::WhileNode<'_>) {
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
        let single_line = self.single_line;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("while ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            let header_line =
                self.line_of(node.predicate().location().end_offset().saturating_sub(1));
            self.emit_trailing_comment_if_same_line(header_line);
            self.indent += 1;
        }

        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }

        if single_line {
            self.output.push_str("; end");
        } else {
            if let Some(closing) = node.closing_loc() {
                self.flush_comments_before(closing.start_offset(), node.statements().is_some());
            }
            self.indent -= 1;
            self.write_indent();
            self.output.push_str("end");
        }
        if !inline {
            self.output.push('\n');
        }
    }

    fn write_until_node(&mut self, node: &ruby_prism::UntilNode<'_>) {
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
        let single_line = self.single_line;
        if !inline {
            self.write_indent();
        }
        self.output.push_str("until ");
        self.inline = true;
        self.visit(&node.predicate());
        self.inline = false;
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            let header_line =
                self.line_of(node.predicate().location().end_offset().saturating_sub(1));
            self.emit_trailing_comment_if_same_line(header_line);
            self.indent += 1;
        }

        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }

        if single_line {
            self.output.push_str("; end");
        } else {
            if let Some(closing) = node.closing_loc() {
                self.flush_comments_before(closing.start_offset(), node.statements().is_some());
            }
            self.indent -= 1;
            self.write_indent();
            self.output.push_str("end");
        }
        if !inline {
            self.output.push('\n');
        }
    }

    fn write_for_node(&mut self, node: &ruby_prism::ForNode<'_>) {
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
        let header_line = self.line_of(node.collection().location().end_offset().saturating_sub(1));
        self.emit_trailing_comment_if_same_line(header_line);

        self.indent += 1;
        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }
        self.flush_comments_before(
            node.end_keyword_loc().start_offset(),
            node.statements().is_some(),
        );
        self.indent -= 1;

        self.write_indent();
        self.output.push_str("end\n");
    }

    fn write_case_node(&mut self, node: &ruby_prism::CaseNode<'_>) {
        let inline = self.inline;
        let single_line = self.single_line;
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
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
        }

        self.write_case_clauses(
            node.conditions().iter().collect(),
            node.else_clause().map(|e| e.location().start_offset()),
            node.end_keyword_loc().start_offset(),
        );

        if let Some(consequent) = node.else_clause() {
            if !single_line {
                self.write_indent();
            }
            self.inline = inline;
            self.write_else_node(&consequent);
        } else {
            if !single_line {
                self.write_indent();
            }
            self.output.push_str("end");
            if !inline {
                self.output.push('\n');
            }
        }
    }

    /// Renders the `when`/`in` clauses of a `case`, telling each one where
    /// it stops so it can keep a comment at the bottom of its body. A
    /// comment between `case x` and the first clause goes above that
    /// clause.
    fn write_case_clauses(
        &mut self,
        clauses: Vec<ruby_prism::Node<'_>>,
        else_offset: Option<usize>,
        end_offset: usize,
    ) {
        for (i, clause) in clauses.iter().enumerate() {
            if i == 0 {
                self.flush_leading_comments(clause.location().start_offset(), false);
            }
            let closer = clauses
                .get(i + 1)
                .map(|next| next.location().start_offset())
                .or(else_offset)
                .unwrap_or(end_offset);
            self.clause_closer = Some(closer);
            // A clause writes its own indent (keyed on `single_line`, not
            // `inline`) and leaves the separator for whatever follows --
            // `; ` in single-line mode, a newline otherwise. `inline` says
            // the `leaf` path must add neither.
            self.inline = true;
            self.visit(clause);
            self.clause_closer = None;
        }
    }

    /// A `when` clause always ends by leaving a separator in place for
    /// whatever comes next (another `when`, an `else`, or `end`) --
    /// `; ` in single-line mode (see fmt_block_node), a real newline
    /// otherwise -- so callers never need to add one of their own.
    fn write_when_node(&mut self, node: &ruby_prism::WhenNode<'_>) {
        let single_line = self.single_line;
        let closer = self.clause_closer.take();
        if !single_line {
            self.write_indent();
        }
        self.output.push_str("when ");
        for (i, condition) in node.conditions().iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.inline = true;
            self.visit(&condition);
        }
        self.inline = false;
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            let header_end = node
                .conditions()
                .iter()
                .last()
                .map_or(node.keyword_loc().end_offset(), |c| {
                    c.location().end_offset()
                });
            self.emit_trailing_comment_if_same_line(self.line_of(header_end.saturating_sub(1)));
            self.indent += 1;
        }

        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }

        if single_line {
            self.output.push_str("; ");
        } else {
            if let Some(closer) = closer {
                self.flush_comments_before(closer, node.statements().is_some());
            }
            self.indent -= 1;
        }
    }

    /// `case pred; in pattern; ...; end` -- pattern-matching's `case/in`,
    /// as opposed to `fmt_case_node`'s `case/when`. Structurally identical
    /// to it (predicate, a list of clauses, an optional else), just a
    /// distinct node type with `in` clauses (`InNode`) instead of `when`
    /// (`WhenNode`).
    fn write_case_match_node(&mut self, node: &ruby_prism::CaseMatchNode<'_>) {
        let inline = self.inline;
        let single_line = self.single_line;
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
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
        }

        self.write_case_clauses(
            node.conditions().iter().collect(),
            node.else_clause().map(|e| e.location().start_offset()),
            node.end_keyword_loc().start_offset(),
        );

        if let Some(consequent) = node.else_clause() {
            if !single_line {
                self.write_indent();
            }
            self.inline = inline;
            self.write_else_node(&consequent);
        } else {
            if !single_line {
                self.write_indent();
            }
            self.output.push_str("end");
            if !inline {
                self.output.push('\n');
            }
        }
    }

    /// One `in pattern` clause of a `case/in`. No guard (`in pattern if
    /// cond`) support yet -- not observed in the wild so far; `InNode`
    /// doesn't expose a separate guard accessor, so that would need its own
    /// investigation into how prism represents it. Like fmt_when_node,
    /// always leaves a separator in place for whatever comes next.
    fn write_in_node(&mut self, node: &ruby_prism::InNode<'_>) {
        let single_line = self.single_line;
        let closer = self.clause_closer.take();
        if !single_line {
            self.write_indent();
        }
        self.output.push_str("in ");
        self.inline = true;
        self.visit(&node.pattern());
        self.inline = false;
        if single_line {
            self.output.push_str("; ");
        } else {
            self.output.push('\n');
            let header_end = node.pattern().location().end_offset();
            self.emit_trailing_comment_if_same_line(self.line_of(header_end.saturating_sub(1)));
            self.indent += 1;
        }

        if let Some(body) = node.statements() {
            self.fmt_statements_node(&body);
        }

        if single_line {
            self.output.push_str("; ");
        } else {
            if let Some(closer) = closer {
                self.flush_comments_before(closer, node.statements().is_some());
            }
            self.indent -= 1;
        }
    }

    /// `predicate ? true_branch : false_branch`. Wraps as a Doc group so an
    /// overlong ternary breaks after `?` with both branches at a fixed
    /// indent from the statement -- the same fixed-indent choice
    /// `fmt_binary_bool_node` makes, not aligned under `?`/`:`, so renaming
    /// the predicate can't reindent the branches. Falls back to the old flat
    /// rendering when the predicate or either branch manages its own
    /// multi-line body, for the same reason `fmt_binary_bool_node` does.
    fn fmt_ternary(&mut self, node: &ruby_prism::IfNode<'_>) {
        let inline = self.inline;
        if !inline {
            self.write_indent();
        }

        let predicate = node.predicate();
        let true_branch: Vec<_> = node
            .statements()
            .map(|b| b.body().iter().collect())
            .unwrap_or_default();
        let false_branch: Vec<_> = node
            .subsequent()
            .and_then(|c| c.as_else_node())
            .and_then(|e| e.statements())
            .map(|b| b.body().iter().collect())
            .unwrap_or_default();

        let manages_own = super::manages_own_indentation(&predicate)
            || true_branch.iter().any(super::manages_own_indentation)
            || false_branch.iter().any(super::manages_own_indentation);

        if manages_own {
            self.inline = true;
            self.visit(&predicate);
            self.output.push_str(" ? ");
            self.write_ternary_branch_flat(&true_branch);
            self.output.push_str(" : ");
            self.write_ternary_branch_flat(&false_branch);
        } else {
            let predicate_doc = self.node_doc(&predicate);
            let true_doc = self.capture_ternary_branch_as_doc(&true_branch);
            let false_doc = self.capture_ternary_branch_as_doc(&false_branch);
            let nest_width = self.indent_str.len();
            let doc = predicate_doc.append(Doc::text(" ?")).append(
                Doc::line()
                    .append(true_doc)
                    .append(Doc::text(" :"))
                    .append(Doc::line())
                    .append(false_doc)
                    .indent(nest_width)
                    .group(),
            );
            let rendered = self.render_doc_to_string(doc);
            self.output.push_str(&rendered);
        }

        if !inline {
            self.output.push('\n');
        }
    }

    fn write_ternary_branch_flat(&mut self, stmts: &[ruby_prism::Node<'_>]) {
        for (i, stmt) in stmts.iter().enumerate() {
            if i > 0 {
                self.output.push_str("; ");
            }
            self.inline = true;
            self.visit(stmt);
        }
    }

    fn capture_ternary_branch_as_doc(&mut self, stmts: &[ruby_prism::Node<'_>]) -> Doc {
        self.capture_as_doc(|f| {
            f.write_ternary_branch_flat(stmts);
        })
    }

    pub(super) fn fmt_and_node(&mut self, node: &ruby_prism::AndNode<'_>) -> Doc {
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("&&").to_string();
        if let Some(doc) = self.verbatim_doc_if_commented(&node.as_node()) {
            return doc;
        }
        self.binary_bool_doc(node.left(), &op_str, node.right())
    }

    pub(super) fn fmt_or_node(&mut self, node: &ruby_prism::OrNode<'_>) -> Doc {
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("||").to_string();
        if let Some(doc) = self.verbatim_doc_if_commented(&node.as_node()) {
            return doc;
        }
        self.binary_bool_doc(node.left(), &op_str, node.right())
    }

    /// Shared renderer for `&&`/`||`. Wraps the pair as a Doc group so an
    /// overlong line breaks right after the operator with the right-hand
    /// side indented one level, instead of always rendering flat regardless
    /// of width -- the same pattern `fmt_call_node` uses for a single-arg
    /// operator-style call. Falls back to the old flat rendering when either
    /// side manages its own multi-line body imperatively (see
    /// `manages_own_indentation`), for the same reason `write_call_args`
    /// excludes those: a captured multi-line body's internal newlines aren't
    /// nest-aware, so wrapping it in this Doc would misalign continuation
    /// lines.
    fn binary_bool_doc(
        &mut self,
        left: ruby_prism::Node<'_>,
        op_str: &str,
        right: ruby_prism::Node<'_>,
    ) -> Doc {
        if super::manages_own_indentation(&left) || super::manages_own_indentation(&right) {
            let mut run = self.begin_run();
            self.run_node(&mut run, &left);
            self.run_text(&mut run, format!(" {op_str} "));
            self.run_node(&mut run, &right);
            return self.end_run(run);
        }

        // `a && b && c` is nested `AndNode`s. Left as one group per node,
        // each level decides to break on its own and the result bin-packs
        // -- `a &&\n  b && c`. Flattening the same-operator spine into one
        // group restores the all-or-nothing rule: the whole chain fits on
        // one line, or every operand gets its own.
        let mut operands = Vec::new();
        self.bool_operands(left, op_str, &mut operands);
        operands.push(right);

        let leaves: Vec<Doc> = operands.iter().map(|o| self.node_doc(o)).collect();
        let nest_width = self.indent_str.len();
        let separator = Doc::text(format!(" {op_str}")).append(Doc::line());
        let mut doc = Doc::nil();
        for (i, leaf) in leaves.into_iter().enumerate() {
            if i == 0 {
                doc = leaf;
            } else {
                doc = doc.append(separator.clone()).append(leaf);
            }
        }
        doc.indent(nest_width).group()
    }

    // Each of these lays out its own multi-line body with `self.indent`
    // and real newlines. As a `leaf` it is rendered where it will sit and
    // lifted straight back out, so the bytes are what the string emitter
    // produced and every width decision inside is made against the real
    // column -- the same bargain `statements_doc` makes, and sound for the
    // same reason: a body's indent comes from nesting depth, never from a
    // break decision.
    //
    // Listed under `self_indented` in the dispatch, so `visit` leaves the
    // leading indent to the writer -- an implicit `begin` has no keyword
    // of its own and starts with a body statement that indents itself.
    pub(super) fn fmt_if_node(&mut self, node: &ruby_prism::IfNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_if_node(node);
        })
    }

    pub(super) fn fmt_else_node(&mut self, node: &ruby_prism::ElseNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_else_node(node);
        })
    }

    pub(super) fn fmt_begin_node(&mut self, node: &ruby_prism::BeginNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_begin_node(node);
        })
    }

    pub(super) fn fmt_rescue_modifier_node(
        &mut self,
        node: &ruby_prism::RescueModifierNode<'_>,
    ) -> Doc {
        self.spliced_doc(|f| {
            f.write_rescue_modifier_node(node);
        })
    }

    pub(super) fn fmt_unless_node(&mut self, node: &ruby_prism::UnlessNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_unless_node(node);
        })
    }

    pub(super) fn fmt_while_node(&mut self, node: &ruby_prism::WhileNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_while_node(node);
        })
    }

    pub(super) fn fmt_until_node(&mut self, node: &ruby_prism::UntilNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_until_node(node);
        })
    }

    pub(super) fn fmt_for_node(&mut self, node: &ruby_prism::ForNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_for_node(node);
        })
    }

    pub(super) fn fmt_case_node(&mut self, node: &ruby_prism::CaseNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_case_node(node);
        })
    }

    pub(super) fn fmt_when_node(&mut self, node: &ruby_prism::WhenNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_when_node(node);
        })
    }

    pub(super) fn fmt_case_match_node(&mut self, node: &ruby_prism::CaseMatchNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_case_match_node(node);
        })
    }

    pub(super) fn fmt_in_node(&mut self, node: &ruby_prism::InNode<'_>) -> Doc {
        self.spliced_doc(|f| {
            f.write_in_node(node);
        })
    }

    /// Collects the operands of a run of the same boolean operator, so
    /// `a && b && c` is three operands rather than a nested pair.
    ///
    /// Stops at a node that has to go through the verbatim path or that
    /// lays out its own body: those have to stay whole.
    fn bool_operands<'a>(
        &mut self,
        node: ruby_prism::Node<'a>,
        op_str: &str,
        out: &mut Vec<ruby_prism::Node<'a>>,
    ) {
        let split = {
            let matching = |loc: ruby_prism::Location<'_>| {
                std::str::from_utf8(loc.as_slice()).unwrap_or("") == op_str
            };
            let pair = if let Some(and) = node.as_and_node() {
                matching(and.operator_loc()).then(|| (and.left(), and.right()))
            } else if let Some(or) = node.as_or_node() {
                matching(or.operator_loc()).then(|| (or.left(), or.right()))
            } else {
                None
            };
            match pair {
                Some((left, right))
                    if !self.should_write_verbatim(&node)
                        && !super::manages_own_indentation(&left)
                        && !super::manages_own_indentation(&right) =>
                {
                    Some((left, right))
                }
                _ => None,
            }
        };

        match split {
            Some((left, right)) => {
                self.bool_operands(left, op_str, out);
                out.push(right);
            }
            None => out.push(node),
        }
    }
}
