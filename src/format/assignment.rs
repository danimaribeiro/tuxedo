//! Every assignment: `=`, `op=`, `||=` and `&&=` to a local, instance,
//! class or global variable, a constant or constant path, an attribute
//! (`a.b ||= c`) or an index (`a[i] += 1`); multiple assignment
//! (`a, (b, c), *d = e`); and the `*_target_node`s a multiple assignment,
//! a `for` or a `rescue => e` binds through.
//!
//! Almost all of them reduce to `finish_assignment` in `mod.rs`: the target
//! and ` op ` as one run of text, then the value at the statement's own
//! indent. The value is never aligned under the `=`, because that column
//! is set by the length of the target, and renaming the target would
//! reindent the whole value.

use super::doc::Doc;
use super::{Formatter, bytes_doc};

impl Formatter {
    pub(super) fn fmt_local_variable_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "=", &node.value())
    }

    pub(super) fn fmt_local_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOperatorWriteNode<'_>,
    ) -> Doc {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.assignment_doc(node.name().as_slice(), op, &node.value())
    }

    pub(super) fn fmt_local_variable_or_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableOrWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "||=", &node.value())
    }

    pub(super) fn fmt_local_variable_and_write_node(
        &mut self,
        node: &ruby_prism::LocalVariableAndWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "&&=", &node.value())
    }

    pub(super) fn fmt_instance_variable_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "=", &node.value())
    }

    pub(super) fn fmt_instance_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOperatorWriteNode<'_>,
    ) -> Doc {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.assignment_doc(node.name().as_slice(), op, &node.value())
    }

    pub(super) fn fmt_instance_variable_or_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableOrWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "||=", &node.value())
    }

    pub(super) fn fmt_instance_variable_and_write_node(
        &mut self,
        node: &ruby_prism::InstanceVariableAndWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "&&=", &node.value())
    }

    pub(super) fn fmt_constant_write_node(
        &mut self,
        node: &ruby_prism::ConstantWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "=", &node.value())
    }

    pub(super) fn fmt_constant_operator_write_node(
        &mut self,
        node: &ruby_prism::ConstantOperatorWriteNode<'_>,
    ) -> Doc {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.assignment_doc(node.name().as_slice(), op, &node.value())
    }

    pub(super) fn fmt_constant_or_write_node(
        &mut self,
        node: &ruby_prism::ConstantOrWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "||=", &node.value())
    }

    pub(super) fn fmt_constant_and_write_node(
        &mut self,
        node: &ruby_prism::ConstantAndWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "&&=", &node.value())
    }

    pub(super) fn fmt_global_variable_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "=", &node.value())
    }

    pub(super) fn fmt_global_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableOperatorWriteNode<'_>,
    ) -> Doc {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.assignment_doc(node.name().as_slice(), op, &node.value())
    }

    pub(super) fn fmt_global_variable_or_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableOrWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "||=", &node.value())
    }

    pub(super) fn fmt_global_variable_and_write_node(
        &mut self,
        node: &ruby_prism::GlobalVariableAndWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "&&=", &node.value())
    }

    pub(super) fn fmt_class_variable_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "=", &node.value())
    }

    pub(super) fn fmt_class_variable_operator_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableOperatorWriteNode<'_>,
    ) -> Doc {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.assignment_doc(node.name().as_slice(), op, &node.value())
    }

    pub(super) fn fmt_class_variable_or_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableOrWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "||=", &node.value())
    }

    pub(super) fn fmt_class_variable_and_write_node(
        &mut self,
        node: &ruby_prism::ClassVariableAndWriteNode<'_>,
    ) -> Doc {
        self.assignment_doc(node.name().as_slice(), "&&=", &node.value())
    }

    pub(super) fn fmt_constant_path_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathWriteNode<'_>,
    ) -> Doc {
        let target = node.target().as_node();
        self.assignment_to_doc(&target, "=", &node.value())
    }

    pub(super) fn fmt_constant_path_operator_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathOperatorWriteNode<'_>,
    ) -> Doc {
        let target = node.target().as_node();
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.assignment_to_doc(&target, op, &node.value())
    }

    pub(super) fn fmt_constant_path_or_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathOrWriteNode<'_>,
    ) -> Doc {
        let target = node.target().as_node();
        self.assignment_to_doc(&target, "||=", &node.value())
    }

    pub(super) fn fmt_constant_path_and_write_node(
        &mut self,
        node: &ruby_prism::ConstantPathAndWriteNode<'_>,
    ) -> Doc {
        let target = node.target().as_node();
        self.assignment_to_doc(&target, "&&=", &node.value())
    }

    /// `a, b, c = value`. While the targets and the `=` fit on the line they
    /// stay there, and the value breaks after the `=` as any assignment's
    /// does. When they do not, every target goes on a line of its own one
    /// indent in, the way a command call's arguments do, and the value
    /// follows the last one:
    ///
    /// ```ruby
    /// @indirect_deps,
    ///   @undeclared_deps,
    ///   @unexpected_linkage_deps = check_formula_deps
    /// ```
    ///
    /// Ruby continues a target list after a comma, so no `\` is needed.
    /// One target per line rather than as many as fit, so that adding a
    /// target never reflows its neighbours.
    pub(super) fn fmt_multi_write_node(&mut self, node: &ruby_prism::MultiWriteNode<'_>) -> Doc {
        let mut flat = self.begin_run();
        let snapshot = self.render_snapshot();
        self.run_multi_targets(&mut flat, node.lefts(), node.rest(), node.rights());
        let flat = self.end_run(flat);
        let head = flat.clone().flat().render(usize::MAX / 2);
        let targets = if self.single_line || head.contains('\n') {
            flat
        } else {
            self.restore_render_snapshot(snapshot);
            let pieces = self.multi_target_pieces(node.lefts(), node.rest(), node.rights());
            let broken = Doc::join(pieces, Doc::text(",").append(Doc::hardline()))
                .indent(self.indent_str.len());
            Doc::stay_or_move(&format!("{head} ="), flat, broken)
        };
        let mut run = self.begin_run();
        run.leaves.push(targets);
        self.finish_assignment(run, "=", &node.value())
    }

    /// Each top-level target of a multiple assignment as its own document,
    /// for the shape that puts one per line. A bare trailing comma
    /// (`a, b, = x`) stays attached to the target before it.
    fn multi_target_pieces(
        &mut self,
        lefts: ruby_prism::NodeList<'_>,
        rest: Option<ruby_prism::Node<'_>>,
        rights: ruby_prism::NodeList<'_>,
    ) -> Vec<Doc> {
        let mut pieces: Vec<Doc> = lefts.iter().map(|t| self.node_doc(&t)).collect();
        if let Some(rest) = rest {
            if matches!(rest, ruby_prism::Node::ImplicitRestNode { .. }) {
                if let Some(last) = pieces.pop() {
                    pieces.push(last.append(Doc::text(",")));
                }
            } else {
                pieces.push(self.node_doc(&rest));
            }
        }
        pieces.extend(rights.iter().map(|t| self.node_doc(&t)));
        pieces
    }

    /// Appends a comma-joined multiple-assignment target list
    /// (`a, @b, (c, d), *rest`) to `run`. Shared by `MultiWriteNode` and by
    /// the nested `MultiTargetNode` that a parenthesized group parses as.
    ///
    /// This is the flat shape: one run of text with no line point. A
    /// parenthesized group is always written this way; only the top-level
    /// list of a `MultiWriteNode` has a broken shape, built from
    /// `multi_target_pieces`.
    fn run_multi_targets(
        &mut self,
        run: &mut super::InlineRun,
        lefts: ruby_prism::NodeList<'_>,
        rest: Option<ruby_prism::Node<'_>>,
        rights: ruby_prism::NodeList<'_>,
    ) {
        let mut first = true;
        for target in lefts.iter() {
            if !first {
                self.run_text(run, ", ");
            }
            first = false;
            self.run_node(run, &target);
        }
        if let Some(rest) = rest {
            // A bare trailing comma (`a, b, = expr`) parses as an
            // `ImplicitRestNode` with nothing following it — it should stay
            // a bare comma, not grow an explicit `*` (that's only correct
            // for an actual anonymous splat target like `a, *, b = expr`,
            // which parses as a `SplatNode` instead and goes through
            // `run_node` below).
            if matches!(rest, ruby_prism::Node::ImplicitRestNode { .. }) {
                self.run_text(run, ",");
            } else {
                if !first {
                    self.run_text(run, ", ");
                }
                self.run_node(run, &rest);
            }
            first = false;
        }
        for target in rights.iter() {
            if !first {
                self.run_text(run, ", ");
            }
            first = false;
            self.run_node(run, &target);
        }
    }

    pub(super) fn fmt_call_operator_write_node(
        &mut self,
        node: &ruby_prism::CallOperatorWriteNode<'_>,
    ) -> Doc {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.call_write_doc(
            node.receiver(),
            node.call_operator_loc(),
            node.message_loc(),
            op,
            &node.value(),
        )
    }

    pub(super) fn fmt_call_or_write_node(&mut self, node: &ruby_prism::CallOrWriteNode<'_>) -> Doc {
        self.call_write_doc(
            node.receiver(),
            node.call_operator_loc(),
            node.message_loc(),
            "||=",
            &node.value(),
        )
    }

    pub(super) fn fmt_call_and_write_node(
        &mut self,
        node: &ruby_prism::CallAndWriteNode<'_>,
    ) -> Doc {
        self.call_write_doc(
            node.receiver(),
            node.call_operator_loc(),
            node.message_loc(),
            "&&=",
            &node.value(),
        )
    }

    /// Shared renderer for `receiver.attr op= value` (`CallOrWriteNode`,
    /// `CallAndWriteNode`, `CallOperatorWriteNode`) — the attribute-writer
    /// counterpart to `write_index_write`.
    ///
    /// The message is taken from `message_loc` rather than `write_name`,
    /// which is the writer name Prism synthesizes (`padding=`) and not what
    /// the source says.
    fn call_write_doc(
        &mut self,
        receiver: Option<ruby_prism::Node<'_>>,
        call_operator: Option<ruby_prism::Location<'_>>,
        message: Option<ruby_prism::Location<'_>>,
        op: &str,
        value: &ruby_prism::Node<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        if let Some(receiver) = receiver {
            self.run_node(&mut run, &receiver);
        }
        if let Some(call_operator) = call_operator {
            self.run_bytes(&mut run, call_operator.as_slice());
        }
        if let Some(message) = message {
            self.run_bytes(&mut run, message.as_slice());
        }
        self.finish_assignment(run, op, value)
    }

    pub(super) fn fmt_local_variable_target_node(
        &mut self,
        node: &ruby_prism::LocalVariableTargetNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_instance_variable_target_node(
        &mut self,
        node: &ruby_prism::InstanceVariableTargetNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_class_variable_target_node(
        &mut self,
        node: &ruby_prism::ClassVariableTargetNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_global_variable_target_node(
        &mut self,
        node: &ruby_prism::GlobalVariableTargetNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_constant_target_node(
        &mut self,
        node: &ruby_prism::ConstantTargetNode<'_>,
    ) -> Doc {
        bytes_doc(node.name().as_slice())
    }

    pub(super) fn fmt_constant_path_target_node(
        &mut self,
        node: &ruby_prism::ConstantPathTargetNode<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        // `parent` is absent for a cbase path (`::Foo, x = ...`), where the
        // leading `::` delimiter is the whole prefix.
        if let Some(parent) = node.parent() {
            self.run_node(&mut run, &parent);
        }
        self.run_bytes(&mut run, node.delimiter_loc().as_slice());
        self.run_bytes(&mut run, node.name_loc().as_slice());
        self.end_run(run)
    }

    pub(super) fn fmt_call_target_node(&mut self, node: &ruby_prism::CallTargetNode<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, &node.receiver());
        self.run_bytes(&mut run, node.call_operator_loc().as_slice());
        // `name()` is the attribute-writer name Prism synthesizes (`a=`);
        // the source spelling of the message is `message_loc`.
        self.run_bytes(&mut run, node.message_loc().as_slice());
        self.end_run(run)
    }

    pub(super) fn fmt_index_target_node(&mut self, node: &ruby_prism::IndexTargetNode<'_>) -> Doc {
        let mut run = self.begin_run();
        self.run_node(&mut run, &node.receiver());
        self.run_bytes(&mut run, node.opening_loc().as_slice());
        if let Some(arguments) = node.arguments() {
            for (i, argument) in arguments.arguments().iter().enumerate() {
                if i > 0 {
                    self.run_text(&mut run, ", ");
                }
                self.run_node(&mut run, &argument);
            }
        }
        self.run_bytes(&mut run, node.closing_loc().as_slice());
        self.end_run(run)
    }

    /// A parenthesized destructuring group nested inside a larger target
    /// list (`a, (b, c) = 1, [2, 3]`).
    pub(super) fn fmt_multi_target_node(&mut self, node: &ruby_prism::MultiTargetNode<'_>) -> Doc {
        let parenthesized = node.lparen_loc().is_some();
        let mut run = self.begin_run();
        if parenthesized {
            self.run_text(&mut run, "(");
        }
        self.run_multi_targets(&mut run, node.lefts(), node.rest(), node.rights());
        if parenthesized {
            self.run_text(&mut run, ")");
        }
        self.end_run(run)
    }

    pub(super) fn fmt_index_or_write_node(
        &mut self,
        node: &ruby_prism::IndexOrWriteNode<'_>,
    ) -> Doc {
        self.index_write_doc(node.receiver(), node.arguments(), "||=", &node.value())
    }

    pub(super) fn fmt_index_and_write_node(
        &mut self,
        node: &ruby_prism::IndexAndWriteNode<'_>,
    ) -> Doc {
        self.index_write_doc(node.receiver(), node.arguments(), "&&=", &node.value())
    }

    pub(super) fn fmt_index_operator_write_node(
        &mut self,
        node: &ruby_prism::IndexOperatorWriteNode<'_>,
    ) -> Doc {
        let op = std::str::from_utf8(node.binary_operator_loc().as_slice()).unwrap_or("+=");
        self.index_write_doc(node.receiver(), node.arguments(), op, &node.value())
    }

    /// Shared renderer for `receiver[args] op= value` (`IndexOrWriteNode`,
    /// `IndexAndWriteNode`, `IndexOperatorWriteNode`) — the compound-
    /// assignment counterpart to the plain `receiver[args] = value` case,
    /// which is a `CallNode` (`[]=`) handled in `calls.rs` instead.
    fn index_write_doc(
        &mut self,
        receiver: Option<ruby_prism::Node<'_>>,
        arguments: Option<ruby_prism::ArgumentsNode<'_>>,
        op: &str,
        value: &ruby_prism::Node<'_>,
    ) -> Doc {
        let mut run = self.begin_run();
        if let Some(receiver) = receiver {
            self.run_node(&mut run, &receiver);
        }
        self.run_text(&mut run, "[");
        if let Some(arguments) = arguments {
            for (i, arg) in arguments.arguments().iter().enumerate() {
                if i > 0 {
                    self.run_text(&mut run, ", ");
                }
                self.run_node(&mut run, &arg);
            }
        }
        self.run_text(&mut run, "]");
        self.finish_assignment(run, op, value)
    }
}
