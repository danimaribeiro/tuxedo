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

    /// `if`/`elsif` in all three of its shapes: the block form, the
    /// modifier (`body if cond`) and the ternary, which Prism gives the
    /// same node type and tells apart by which keyword locations it has.
    pub(super) fn fmt_if_node(&mut self, node: &ruby_prism::IfNode<'_>) -> Doc {
        let keyword = node.if_keyword_loc().map(|k| k.as_slice().to_vec());
        let is_ternary = keyword.is_none();
        let is_elsif = keyword.as_deref() == Some(b"elsif");
        let is_modifier = !is_ternary && !is_elsif && node.end_keyword_loc().is_none();

        if is_ternary {
            return self.ternary_doc(node);
        }
        if is_modifier {
            return self.conditional_modifier_doc(
                "if",
                node.location().end_offset(),
                &node.predicate(),
                node.statements(),
                node.statements(),
            );
        }

        let header = Doc::text(if is_elsif { "elsif " } else { "if " })
            .append(self.condition_doc(&node.predicate()));
        self.branching_doc(
            header,
            &node.predicate(),
            node.statements(),
            node.subsequent(),
            node.end_keyword_loc().map(|e| e.start_offset()),
        )
    }

    pub(super) fn fmt_unless_node(&mut self, node: &ruby_prism::UnlessNode<'_>) -> Doc {
        if node.end_keyword_loc().is_none() {
            return self.conditional_modifier_doc(
                "unless",
                node.location().end_offset(),
                &node.predicate(),
                node.statements(),
                node.statements(),
            );
        }
        let header = Doc::text("unless ").append(self.condition_doc(&node.predicate()));
        self.branching_doc(
            header,
            &node.predicate(),
            node.statements(),
            node.else_clause().map(|e| e.as_node()),
            node.end_keyword_loc().map(|e| e.start_offset()),
        )
    }

    /// The condition of a block `if`/`elsif`/`unless`/`while`/`until`, one
    /// level deeper than the body it sits above.
    ///
    /// A condition that breaks and a body are otherwise drawn at the same
    /// indent -- both one level in from the statement -- so a reader
    /// cannot tell where the condition ends and the body starts:
    ///
    /// ```ruby
    /// Correctable = if defined?(SpellChecker) &&
    ///   defined?(Correctable)
    ///   Module.new do
    /// ```
    ///
    /// `defined?(Correctable)` is still the condition; `Module.new` is the
    /// body. The extra level is what separates them, and it is what
    /// `Layout/MultilineOperationIndentation: indented` -- set in
    /// `scripts/format-verify.rubocop.yml` -- holds Tuxedo to. A condition
    /// with no line point in it is unaffected: an indent only moves lines
    /// that break.
    fn condition_doc(&mut self, predicate: &ruby_prism::Node<'_>) -> Doc {
        self.condition_spine = true;
        let doc = self.node_doc(predicate);
        // A condition with no spine of its own leaves it set; it must not
        // reach the next one built anywhere else.
        self.condition_spine = false;
        doc
    }

    /// The shared body of a block `if`/`elsif`/`unless`: header, body, and
    /// then either the branch that continues the chain -- which closes it
    /// with its own `end` -- or the `end` itself.
    fn branching_doc(
        &mut self,
        header: Doc,
        predicate: &ruby_prism::Node<'_>,
        statements: Option<ruby_prism::StatementsNode<'_>>,
        subsequent: Option<ruby_prism::Node<'_>>,
        end_offset: Option<usize>,
    ) -> Doc {
        let body = statements.map(|s| s.as_node());
        if self.single_line {
            return header
                .append(Doc::text("; "))
                .append(self.optional_body_doc(body))
                .append(match subsequent {
                    Some(subsequent) => Doc::text("; ").append(self.node_doc(&subsequent)),
                    None => Doc::text("; end"),
                });
        }

        let header_line = self.line_of(predicate.location().end_offset().saturating_sub(1));
        // A comment above `elsif`/`else`/`end` belongs to the branch it
        // sits at the bottom of.
        let closer = subsequent
            .as_ref()
            .map(|s| s.location().start_offset())
            .or(end_offset);
        let doc = self.clause_body_doc(header, header_line, body, closer);
        doc.append(Doc::hardline()).append(match subsequent {
            Some(subsequent) => self.node_doc(&subsequent),
            None => Doc::text("end"),
        })
    }

    /// `body if cond` / `body unless cond`, or the block form when the
    /// modifier does not fit on one line.
    ///
    /// A modifier's condition is never broken: the statement would end
    /// with `if options.key?(` and the reader could not tell the body
    /// from the condition (`Style/MultilineIfModifier`). So the choice is
    /// between one flat line and `if cond` / body / `end`, and it is the
    /// printer's, by width. Prism parses both spellings as the same node.
    ///
    /// A modifier that has no flat shape at all (a `do...end` call or a
    /// heredoc in it) stays a modifier. `while`/`until` never take this
    /// path: `begin ... end while cond` runs its body first, and the block
    /// form would not.
    fn conditional_modifier_doc(
        &mut self,
        keyword: &str,
        end_offset: usize,
        predicate: &ruby_prism::Node<'_>,
        statements: Option<ruby_prism::StatementsNode<'_>>,
        block_statements: Option<ruby_prism::StatementsNode<'_>>,
    ) -> Doc {
        let spaced = format!(" {keyword} ");
        if self.single_line {
            return self.modifier_doc(statements, &spaced, predicate);
        }
        let snap = self.render_snapshot();
        let modifier = self.modifier_doc(statements, &spaced, predicate);
        if modifier.breaks() {
            return modifier;
        }
        self.rewind_render_snapshot(snap);
        // A comment after the statement is the statement's, in either
        // shape, and is taken by whoever owns the statement's line. The
        // block form must not claim it as its header's: the printer may
        // still pick the modifier, which has no header to carry it.
        let was_limit = self.comment_limit.replace(end_offset);
        let header = Doc::text(format!("{keyword} ")).append(self.condition_doc(predicate));
        let block = self.branching_doc(header, predicate, block_statements, None, None);
        self.comment_limit = was_limit;
        Doc::if_break(block, modifier.flat()).group()
    }

    /// `body if cond`, `body unless cond`, `body while cond`,
    /// `body until cond`: the whole statement on one line.
    fn modifier_doc(
        &mut self,
        statements: Option<ruby_prism::StatementsNode<'_>>,
        keyword: &str,
        predicate: &ruby_prism::Node<'_>,
    ) -> Doc {
        let body: Vec<_> = statements
            .map(|b| b.body().iter().collect())
            .unwrap_or_default();
        self.flat_statements_doc(&body)
            .append(Doc::text(keyword.to_string()))
            .append(self.node_doc(predicate))
    }

    fn optional_body_doc(&mut self, body: Option<ruby_prism::Node<'_>>) -> Doc {
        match body {
            Some(body) => self.node_doc(&body),
            None => Doc::nil(),
        }
    }

    /// `else`, and the `end` that closes the chain it is the last branch
    /// of. A `begin`'s `else` is not this node's job -- `fmt_begin_node`
    /// has its own `end` to write after the `ensure` that may follow.
    pub(super) fn fmt_else_node(&mut self, node: &ruby_prism::ElseNode<'_>) -> Doc {
        let body = node.statements().map(|s| s.as_node());
        if self.single_line {
            return Doc::text("else; ")
                .append(self.optional_body_doc(body))
                .append(Doc::text("; end"));
        }
        let header_line = self.line_of(node.else_keyword_loc().start_offset());
        self.keyword_body_doc(
            Doc::text("else"),
            header_line,
            body,
            node.end_keyword_loc().map(|e| e.start_offset()),
        )
    }

    /// `begin ... end`, including any attached `rescue`/`else`/`ensure`
    /// clauses. Also reached with no `begin`/`end` keywords at all: prism
    /// wraps a method/block body in a `BeginNode` whenever it has a
    /// `rescue`/`else`/`ensure` clause (e.g. `def foo; rescue; end`), in
    /// which case the enclosing `def`/block already owns the keyword and
    /// the `end`, and this only contributes the body and the clauses.
    pub(super) fn fmt_begin_node(&mut self, node: &ruby_prism::BeginNode<'_>) -> Doc {
        let has_keyword = node.begin_keyword_loc().is_some();
        let single_line = self.single_line;
        let nest = self.indent_str.len();

        // Where each section of the body stops: a comment above `rescue`/
        // `else`/`ensure`/`end` stays at the bottom of the section before
        // it. An implicit `begin` has no `end` of its own; whoever owns
        // that `end` takes the last section's comments.
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

        let statements = node.statements().map(|s| s.as_node());
        let has_statements = statements.is_some();

        // Taken before the body is built: the comment cursor is strictly
        // ordered, so `begin # why` has to be claimed by the header or the
        // first statement of the body will take it as its own.
        let header_comment = match node.begin_keyword_loc() {
            Some(keyword) if !single_line => {
                let line = self.line_of(keyword.start_offset());
                self.trailing_comment_doc(line)
            }
            _ => None,
        };

        // The body sits one level in from `begin`, and at the enclosing
        // level when there is no `begin` -- the `def`/block that owns the
        // implicit one has already indented it.
        let mut body = Doc::nil();
        if has_keyword && !single_line {
            self.indent += 1;
        }
        if let Some(statements) = statements {
            body = body.append(self.node_doc(&statements));
        }
        if let Some(closer) = rescue_offset
            && let Some(comments) = self.comments_before_doc(closer, has_statements)
        {
            body = body.append(comments);
        }
        if has_keyword && !single_line {
            self.indent -= 1;
        }

        // rescue/else/ensure align with the keyword that opens the body
        // they guard. For an explicit `begin` that is this node's own
        // level; for an implicit one -- a `def`/block body with a
        // `rescue` -- it is one level out from the body this node *is*,
        // and the owner has already indented that body, so the clauses
        // step back out of it (`dedent`, below).
        let dedent = usize::from(!has_keyword);
        if !single_line {
            self.indent -= dedent;
        }
        let mut clauses = Doc::nil();
        if let Some(rescue) = node.rescue_clause() {
            clauses = clauses
                .append(self.clause_separator(single_line))
                .append(self.rescue_clause_doc(&rescue, else_offset));
        }
        if let Some(else_clause) = node.else_clause() {
            let body = else_clause.statements().map(|s| s.as_node());
            clauses = clauses.append(if single_line {
                let inner = self.optional_body_doc(body);
                Doc::text("; else; ").append(inner)
            } else {
                let header_line = self.line_of(else_clause.else_keyword_loc().start_offset());
                let inner =
                    self.clause_body_doc(Doc::text("else"), header_line, body, ensure_offset);
                Doc::hardline().append(inner)
            });
        }
        if let Some(ensure) = node.ensure_clause() {
            clauses = clauses
                .append(self.clause_separator(single_line))
                .append(self.fmt_ensure_node(&ensure));
        }
        if !single_line {
            self.indent += dedent;
        }

        if !has_keyword {
            // An empty implicit body (`def foo\nrescue\nend`) has nothing
            // of its own to draw a line break for: `clauses` already opens
            // on one (its `clause_separator`), which is this returned
            // doc's first break. Telling the caller so keeps it from
            // adding a second one before it, which would print a line
            // with nothing on it but its own indent.
            if !has_statements && !single_line {
                self.body_owns_first_break = true;
            }
            return body.append(if single_line {
                clauses
            } else {
                clauses.dedent(nest)
            });
        }
        if single_line {
            return Doc::text("begin; ")
                .append(body)
                .append(clauses)
                .append(Doc::text("; end"));
        }
        let doc = Doc::text("begin").append(header_comment.unwrap_or_else(Doc::nil));
        let body_doc = if has_statements {
            Doc::hardline().append(body).indent(nest)
        } else {
            Doc::nil()
        };
        doc.append(body_doc)
            .append(clauses)
            .append(Doc::hardline())
            .append(Doc::text("end"))
    }

    /// What goes between a `begin` body and the clause that guards it:
    /// `; ` on one line, a line break otherwise.
    fn clause_separator(&self, single_line: bool) -> Doc {
        if single_line {
            Doc::text("; ")
        } else {
            Doc::hardline()
        }
    }

    /// `expr rescue fallback` -- the modifier form, as opposed to an
    /// explicit `begin`/`rescue` block. Prism's default visitor for this
    /// node just visits `expression()` then `rescue_expression()` back to
    /// back with no separator, which is why this needs its own handler:
    /// without the literal " rescue " keyword between them, a statement
    /// like `transmit(...) rescue nil` followed by more code lost the
    /// boundary between "nil" and the next line entirely.
    pub(super) fn fmt_rescue_modifier_node(
        &mut self,
        node: &ruby_prism::RescueModifierNode<'_>,
    ) -> Doc {
        let expression = self.node_doc(&node.expression());
        let fallback = self.node_doc(&node.rescue_expression());
        expression.append(Doc::text(" rescue ")).append(fallback)
    }

    pub(super) fn fmt_rescue_node(&mut self, node: &ruby_prism::RescueNode<'_>) -> Doc {
        self.rescue_clause_doc(node, None)
    }

    /// `closer` is where the last `rescue` of the chain stops (the start
    /// of the `else`/`ensure`/`end` that follows), if the caller knows it.
    fn rescue_clause_doc(
        &mut self,
        node: &ruby_prism::RescueNode<'_>,
        closer: Option<usize>,
    ) -> Doc {
        let single_line = self.single_line;
        let mut header = Doc::text("rescue");
        for (i, exception) in node.exceptions().iter().enumerate() {
            header = header
                .append(Doc::text(if i == 0 { " " } else { ", " }))
                .append(self.node_doc(&exception));
        }
        if let Some(reference) = node.reference() {
            header = header
                .append(Doc::text(" => "))
                .append(self.node_doc(&reference));
        }

        let body = node.statements().map(|s| s.as_node());
        let mut doc = if single_line {
            header
                .append(Doc::text("; "))
                .append(self.optional_body_doc(body))
        } else {
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
            let header_line = self.line_of(header_end.saturating_sub(1));
            let section_end = node
                .subsequent()
                .map(|s| s.location().start_offset())
                .or(closer);
            self.clause_body_doc(header, header_line, body, section_end)
        };

        if let Some(subsequent) = node.subsequent() {
            doc = doc
                .append(self.clause_separator(single_line))
                .append(self.rescue_clause_doc(&subsequent, closer));
        }
        doc
    }

    pub(super) fn fmt_ensure_node(&mut self, node: &ruby_prism::EnsureNode<'_>) -> Doc {
        let body = node.statements().map(|s| s.as_node());
        if self.single_line {
            return Doc::text("ensure; ").append(self.optional_body_doc(body));
        }
        let header_line = self.line_of(node.ensure_keyword_loc().start_offset());
        self.clause_body_doc(
            Doc::text("ensure"),
            header_line,
            body,
            Some(node.end_keyword_loc().start_offset()),
        )
    }

    pub(super) fn fmt_while_node(&mut self, node: &ruby_prism::WhileNode<'_>) -> Doc {
        if node.closing_loc().is_none() {
            return self.modifier_doc(node.statements(), " while ", &node.predicate());
        }
        self.loop_doc(
            "while ",
            node.statements(),
            &node.predicate(),
            node.closing_loc(),
        )
    }

    pub(super) fn fmt_until_node(&mut self, node: &ruby_prism::UntilNode<'_>) -> Doc {
        if node.closing_loc().is_none() {
            return self.modifier_doc(node.statements(), " until ", &node.predicate());
        }
        self.loop_doc(
            "until ",
            node.statements(),
            &node.predicate(),
            node.closing_loc(),
        )
    }

    /// The block form of `while`/`until`: a predicate, a body and `end`.
    fn loop_doc(
        &mut self,
        keyword: &str,
        statements: Option<ruby_prism::StatementsNode<'_>>,
        predicate: &ruby_prism::Node<'_>,
        closing: Option<ruby_prism::Location<'_>>,
    ) -> Doc {
        let header = Doc::text(keyword.to_string()).append(self.condition_doc(predicate));
        let body = statements.map(|s| s.as_node());
        if self.single_line {
            return header
                .append(Doc::text("; "))
                .append(self.optional_body_doc(body))
                .append(Doc::text("; end"));
        }
        let header_line = self.line_of(predicate.location().end_offset().saturating_sub(1));
        self.keyword_body_doc(header, header_line, body, closing.map(|c| c.start_offset()))
    }

    pub(super) fn fmt_for_node(&mut self, node: &ruby_prism::ForNode<'_>) -> Doc {
        let header = Doc::text("for ")
            .append(self.node_doc(&node.index()))
            .append(Doc::text(" in "))
            .append(self.node_doc(&node.collection()));
        let header_line = self.line_of(node.collection().location().end_offset().saturating_sub(1));
        self.keyword_body_doc(
            header,
            header_line,
            node.statements().map(|s| s.as_node()),
            Some(node.end_keyword_loc().start_offset()),
        )
    }

    pub(super) fn fmt_case_node(&mut self, node: &ruby_prism::CaseNode<'_>) -> Doc {
        let predicate = node.predicate();
        self.case_doc(
            predicate,
            node.conditions().iter().collect(),
            node.else_clause().map(|e| e.as_node()),
            node.else_clause().map(|e| e.location().start_offset()),
            node.end_keyword_loc().start_offset(),
        )
    }

    /// `case pred; in pattern; ...; end` -- pattern-matching's `case/in`,
    /// as opposed to `fmt_case_node`'s `case/when`. Structurally identical
    /// to it (predicate, a list of clauses, an optional else), just a
    /// distinct node type with `in` clauses (`InNode`) instead of `when`
    /// (`WhenNode`).
    pub(super) fn fmt_case_match_node(&mut self, node: &ruby_prism::CaseMatchNode<'_>) -> Doc {
        let predicate = node.predicate();
        self.case_doc(
            predicate,
            node.conditions().iter().collect(),
            node.else_clause().map(|e| e.as_node()),
            node.else_clause().map(|e| e.location().start_offset()),
            node.end_keyword_loc().start_offset(),
        )
    }

    /// Shared by `case/when` and `case/in`: the header, every clause at
    /// the `case`'s own level, and then the `else` (which closes the chain
    /// itself) or `end`.
    ///
    /// Clauses are *not* indented from `case`. That is
    /// Layout/CaseIndentation's `end` style, and it is the fixed-indent
    /// rule this project applies everywhere: `type = case value` puts
    /// `case` wherever the target's length leaves it, and indenting `when`
    /// under that column would let renaming the target reindent every
    /// clause.
    fn case_doc(
        &mut self,
        predicate: Option<ruby_prism::Node<'_>>,
        clauses: Vec<ruby_prism::Node<'_>>,
        else_clause: Option<ruby_prism::Node<'_>>,
        else_offset: Option<usize>,
        end_offset: usize,
    ) -> Doc {
        let single_line = self.single_line;
        let mut doc = Doc::text("case");
        if let Some(predicate) = predicate {
            doc = doc.append(Doc::text(" ")).append(self.node_doc(&predicate));
        }

        // A comment between `case x` and the first clause goes above that
        // clause.
        if let Some(first) = clauses.first()
            && let Some(comments) =
                self.leading_comments_doc(first.location().start_offset(), false)
        {
            doc = doc.append(comments);
        }

        for (i, clause) in clauses.iter().enumerate() {
            // Each clause has to be told where it stops so it can keep a
            // comment at the bottom of its body; it has no other way to
            // learn it.
            self.clause_closer = Some(
                clauses
                    .get(i + 1)
                    .map(|next| next.location().start_offset())
                    .or(else_offset)
                    .unwrap_or(end_offset),
            );
            let clause_doc = self.node_doc(clause);
            self.clause_closer = None;
            doc = doc
                .append(self.clause_separator(single_line))
                .append(clause_doc);
        }

        doc.append(self.clause_separator(single_line))
            .append(match else_clause {
                Some(else_clause) => self.node_doc(&else_clause),
                None => Doc::text("end"),
            })
    }

    pub(super) fn fmt_when_node(&mut self, node: &ruby_prism::WhenNode<'_>) -> Doc {
        let closer = self.clause_closer.take();
        // The values are one collection: all on the `when` line, or one per
        // line at the body's indent. Held to one line, there is no line
        // point to take.
        let separator = if self.single_line {
            Doc::text(", ")
        } else {
            Doc::text(",").append(Doc::line())
        };
        let mut values = Doc::nil();
        for (i, condition) in node.conditions().iter().enumerate() {
            if i > 0 {
                values = values.append(separator.clone());
            }
            values = values.append(self.node_doc(&condition));
        }
        let header = Doc::text("when ").append(if self.single_line {
            values
        } else {
            values.indent(self.indent_str.len()).group()
        });
        let header_end = node
            .conditions()
            .iter()
            .last()
            .map_or(node.keyword_loc().end_offset(), |c| {
                c.location().end_offset()
            });
        self.clause_doc(header, header_end, node.statements(), closer)
    }

    /// One `in pattern` clause of a `case/in`. Prism models a guard
    /// (`in pattern if cond`) as the pattern being a modifier `IfNode` or
    /// `UnlessNode` whose body is the real pattern. That node must not go
    /// through the conditional formatting: the block form is not a
    /// pattern, so an overlong guard breaks inside its condition instead.
    pub(super) fn fmt_in_node(&mut self, node: &ruby_prism::InNode<'_>) -> Doc {
        let closer = self.clause_closer.take();
        let pattern = node.pattern();
        let header = Doc::text("in ").append(match guarded_pattern(&pattern) {
            Some((inner, keyword, predicate)) => self
                .node_doc(&inner)
                .append(Doc::text(format!(" {keyword} ")))
                .append(self.condition_doc(&predicate)),
            None => self.node_doc(&pattern),
        });
        let header_end = pattern.location().end_offset();
        self.clause_doc(header, header_end, node.statements(), closer)
    }

    /// A `when`/`in` clause: header and body, and nothing to close it --
    /// the `case` above supplies the separator before whatever follows.
    fn clause_doc(
        &mut self,
        header: Doc,
        header_end: usize,
        statements: Option<ruby_prism::StatementsNode<'_>>,
        closer: Option<usize>,
    ) -> Doc {
        let body = statements.map(|s| s.as_node());
        if self.single_line {
            if body.is_none() {
                return header;
            }
            return header
                .append(Doc::text("; "))
                .append(self.optional_body_doc(body));
        }
        let header_line = self.line_of(header_end.saturating_sub(1));
        self.clause_body_doc(header, header_line, body, closer)
    }

    /// `predicate ? true_branch : false_branch`, as one group: an overlong
    /// ternary breaks after `?` with both branches at a fixed indent from
    /// the statement -- not aligned under `?`/`:`, so renaming the
    /// predicate cannot reindent the branches.
    fn ternary_doc(&mut self, node: &ruby_prism::IfNode<'_>) -> Doc {
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

        let predicate_doc = self.node_doc(&predicate);
        let true_doc = self.flat_statements_doc(&true_branch);
        let false_doc = self.flat_statements_doc(&false_branch);
        let nest_width = self.indent_str.len();
        predicate_doc.append(Doc::text(" ?")).append(
            Doc::line()
                .append(true_doc)
                .append(Doc::text(" :"))
                .append(Doc::line())
                .append(false_doc)
                .indent(nest_width)
                .group(),
        )
    }

    pub(super) fn fmt_and_node(&mut self, node: &ruby_prism::AndNode<'_>) -> Doc {
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("&&").to_string();
        self.binary_bool_doc(node.left(), &op_str, node.right())
    }

    pub(super) fn fmt_or_node(&mut self, node: &ruby_prism::OrNode<'_>) -> Doc {
        let op = node.operator_loc().as_slice();
        let op_str = std::str::from_utf8(op).unwrap_or("||").to_string();
        self.binary_bool_doc(node.left(), &op_str, node.right())
    }

    /// Shared renderer for `&&`/`||`. Wraps the spine as one Doc group so
    /// an overlong line breaks after each operator with the operands
    /// indented one level, instead of always rendering flat regardless of
    /// width -- see `infix_doc`, which `operator_call_doc` shares for
    /// `a + b + c`.
    ///
    /// `infix_doc` falls back to the flat shape when an operand turns out
    /// to be multi-line, which no line point of this group could move.
    fn binary_bool_doc(
        &mut self,
        left: ruby_prism::Node<'_>,
        op_str: &str,
        right: ruby_prism::Node<'_>,
    ) -> Doc {
        // `a && b && c` is nested `AndNode`s, and one group per node would
        // bin-pack. Flattening the same-operator spine into one group
        // restores the all-or-nothing rule.
        let condition = std::mem::take(&mut self.condition_spine);
        let mut operands = Vec::new();
        Self::bool_operands(left, op_str, &mut operands);
        operands.push(right);

        let block_tail = operands.last().is_some_and(|last| {
            last.as_begin_node().is_some()
                || last
                    .as_if_node()
                    .is_some_and(|n| n.if_keyword_loc().is_some())
                || last.as_unless_node().is_some()
                || last.as_case_node().is_some()
                || last.as_case_match_node().is_some()
        });
        let (leaves, gaps) = self.operand_docs(&operands);
        self.infix_doc(leaves, gaps, op_str, condition, block_tail)
    }

    /// Collects the operands of a run of the same boolean operator, so
    /// `a && b && c` is three operands rather than a nested pair.
    fn bool_operands<'a>(
        node: ruby_prism::Node<'a>,
        op_str: &str,
        out: &mut Vec<ruby_prism::Node<'a>>,
    ) {
        let matching = |loc: ruby_prism::Location<'_>| {
            std::str::from_utf8(loc.as_slice()).unwrap_or("") == op_str
        };
        let split = if let Some(and) = node.as_and_node() {
            matching(and.operator_loc()).then(|| (and.left(), and.right()))
        } else if let Some(or) = node.as_or_node() {
            matching(or.operator_loc()).then(|| (or.left(), or.right()))
        } else {
            None
        };

        match split {
            Some((left, right)) => {
                Self::bool_operands(left, op_str, out);
                out.push(right);
            }
            None => out.push(node),
        }
    }
}

/// The pattern, keyword and condition of `in pattern if cond`, which prism
/// hands over as a modifier `if`/`unless` wrapping the pattern.
fn guarded_pattern<'pr>(
    node: &ruby_prism::Node<'pr>,
) -> Option<(ruby_prism::Node<'pr>, &'static str, ruby_prism::Node<'pr>)> {
    let (keyword, statements, predicate) = if let Some(n) = node.as_if_node() {
        ("if", n.statements()?, n.predicate())
    } else {
        let n = node.as_unless_node()?;
        ("unless", n.statements()?, n.predicate())
    };
    Some((statements.body().iter().next()?, keyword, predicate))
}
