//! Node dispatch: which `fmt_*` method renders which Prism node.
//!
//! This is a plain exhaustive `match`, deliberately not an
//! `impl ruby_prism::Visit`. `Visit`'s default for a node type with no
//! override is to walk into the children and emit nothing for the node
//! itself -- which is how `defined?(Foo)` used to come out as `Foo`, and
//! `undef foo` as `foo`: still valid Ruby, and a different program. With a
//! `match` and no wildcard arm, a node type Tuxedo has not learned yet has
//! to be listed under `verbatim`, where its source text is copied through
//! unchanged, and a node type added by a future Prism release is a compile
//! error instead of a silent deletion.
//!
//! There are three groups, and they differ in what the `fmt_*` method
//! does, not in what it is allowed to render:
//!
//! - `leaf` -- returns a [`Doc`]. `visit` renders it; `leaf_doc` hands it
//!   back unrendered so an enclosing group can measure and lay it out.
//! - `formatted` -- writes to `self.output`. To compose one of these into
//!   a `Doc`, `node_doc` has to render it first and wrap the string as one
//!   opaque leaf, which is why nested collections get rendered more than
//!   once and why `forces_list_break` exists.
//! - `verbatim` -- source text copied through unchanged.
//!
//! Moving a node type from `formatted` to `leaf` is the unit of work in
//! the migration; the two lists are generated from one macro invocation so
//! they cannot disagree about which group a node is in.

use super::Formatter;
use super::doc::Doc;

macro_rules! dispatch {
    (
        leaf { $($leaf_variant:ident => $leaf_cast:ident => $leaf_fmt:ident,)* }
        formatted { $($variant:ident => $cast:ident => $fmt:ident,)* }
        verbatim { $($raw:ident,)* }
    ) => {
        impl Formatter {
            pub(super) fn visit(&mut self, node: &ruby_prism::Node<'_>) {
                match node {
                    $(
                        ruby_prism::Node::$leaf_variant { .. } => {
                            if let Some(concrete) = node.$leaf_cast() {
                                let inline = self.inline;
                                // Before the `fmt_*`, not just before the
                                // render: a `fmt_*` that measures the
                                // current column while it builds (an
                                // interpolated literal, an implicit string
                                // concat) has to see the indent it will be
                                // laid out after.
                                if !inline {
                                    self.write_indent();
                                }
                                let doc = self.$leaf_fmt(&concrete);
                                self.write_leaf_doc(inline, doc);
                            }
                        }
                    )*
                    $(
                        ruby_prism::Node::$variant { .. } => {
                            if let Some(concrete) = node.$cast() {
                                self.$fmt(&concrete);
                            }
                        }
                    )*
                    ruby_prism::Node::ProgramNode { .. } => {
                        if let Some(program) = node.as_program_node() {
                            self.fmt_statements_node(&program.statements());
                        }
                    }
                    // `...` as a call argument, forwarding a `...` parameter through.
                    ruby_prism::Node::ForwardingArgumentsNode { .. } => {
                        self.output.push_str("...")
                    }
                    // `regex =~ str` where `regex` has named captures: Ruby defines
                    // the capture names as local variables as a side effect.
                    // `targets` names those locals but they aren't source text to
                    // emit -- only the `=~` call itself is.
                    ruby_prism::Node::MatchWriteNode { .. } => {
                        if let Some(match_write) = node.as_match_write_node() {
                            self.visit(&match_write.call().as_node());
                        }
                    }
                    // Markers for a block that uses `it`/`_1`: no source text of
                    // their own (see `explicit_parameters`).
                    ruby_prism::Node::NumberedParametersNode { .. }
                    | ruby_prism::Node::ItParametersNode { .. } => {}
                    $(ruby_prism::Node::$raw { .. })|* => self.write_verbatim(node),
                }
            }

            /// The `Doc` for `node` if its `fmt_*` returns one, `None` if it
            /// still writes strings. Callers want [`Formatter::node_doc`],
            /// which falls back to capturing; this is the half of the
            /// decision that the dispatch list owns.
            pub(super) fn leaf_doc(&mut self, node: &ruby_prism::Node<'_>) -> Option<Doc> {
                match node {
                    $(
                        ruby_prism::Node::$leaf_variant { .. } => {
                            node.$leaf_cast().map(|concrete| self.$leaf_fmt(&concrete))
                        }
                    )*
                    _ => None,
                }
            }
        }
    };
}

dispatch! {
    // Nodes whose `fmt_*` builds a `Doc`. Composing one into an enclosing
    // group is free -- it can be measured without being rendered first --
    // and any line points it carries of its own (an adjacent-literal
    // concatenation's `\` wraps) are decided by the printer along with
    // everything around them.
    leaf {
        LocalVariableReadNode => as_local_variable_read_node => fmt_local_variable_read_node,
        GlobalVariableReadNode => as_global_variable_read_node => fmt_global_variable_read_node,
        InstanceVariableReadNode => as_instance_variable_read_node => fmt_instance_variable_read_node,
        ClassVariableReadNode => as_class_variable_read_node => fmt_class_variable_read_node,
        SelfNode => as_self_node => fmt_self_node,
        ConstantReadNode => as_constant_read_node => fmt_constant_read_node,
        TrueNode => as_true_node => fmt_true_node,
        FalseNode => as_false_node => fmt_false_node,
        NilNode => as_nil_node => fmt_nil_node,
        IntegerNode => as_integer_node => fmt_integer_node,
        SymbolNode => as_symbol_node => fmt_symbol_node,
        RegularExpressionNode => as_regular_expression_node => fmt_regular_expression_node,
        SourceFileNode => as_source_file_node => fmt_source_file_node,
        SourceLineNode => as_source_line_node => fmt_source_line_node,
        FloatNode => as_float_node => fmt_float_node,
        RationalNode => as_rational_node => fmt_rational_node,
        ImaginaryNode => as_imaginary_node => fmt_imaginary_node,
        NumberedReferenceReadNode => as_numbered_reference_read_node => fmt_numbered_reference_read_node,
        BackReferenceReadNode => as_back_reference_read_node => fmt_back_reference_read_node,
        XStringNode => as_x_string_node => fmt_x_string_node,
        ParenthesesNode => as_parentheses_node => fmt_parentheses_node,
        StringNode => as_string_node => fmt_string_node,
        InterpolatedStringNode => as_interpolated_string_node => fmt_interpolated_string_node,
        InterpolatedSymbolNode => as_interpolated_symbol_node => fmt_interpolated_symbol_node,
        InterpolatedRegularExpressionNode => as_interpolated_regular_expression_node => fmt_interpolated_regular_expression_node,
        InterpolatedXStringNode => as_interpolated_x_string_node => fmt_interpolated_x_string_node,
        SplatNode => as_splat_node => fmt_splat_node,
        BlockArgumentNode => as_block_argument_node => fmt_block_argument_node,
        ArrayNode => as_array_node => fmt_array_node,
        RangeNode => as_range_node => fmt_range_node,
        HashNode => as_hash_node => fmt_hash_node,
        KeywordHashNode => as_keyword_hash_node => fmt_keyword_hash_node,
        AssocNode => as_assoc_node => fmt_assoc_node,
        AssocSplatNode => as_assoc_splat_node => fmt_assoc_splat_node,
        HashPatternNode => as_hash_pattern_node => fmt_hash_pattern_node,
        ArrayPatternNode => as_array_pattern_node => fmt_array_pattern_node,
        ImplicitNode => as_implicit_node => fmt_implicit_node,
        AlternationPatternNode => as_alternation_pattern_node => fmt_alternation_pattern_node,
        CapturePatternNode => as_capture_pattern_node => fmt_capture_pattern_node,
        PinnedVariableNode => as_pinned_variable_node => fmt_pinned_variable_node,
        PinnedExpressionNode => as_pinned_expression_node => fmt_pinned_expression_node,
        FindPatternNode => as_find_pattern_node => fmt_find_pattern_node,
        FlipFlopNode => as_flip_flop_node => fmt_flip_flop_node,
        DefinedNode => as_defined_node => fmt_defined_node,
        UndefNode => as_undef_node => fmt_undef_node,
        LocalVariableTargetNode => as_local_variable_target_node => fmt_local_variable_target_node,
        InstanceVariableTargetNode => as_instance_variable_target_node => fmt_instance_variable_target_node,
        ClassVariableTargetNode => as_class_variable_target_node => fmt_class_variable_target_node,
        GlobalVariableTargetNode => as_global_variable_target_node => fmt_global_variable_target_node,
        ConstantTargetNode => as_constant_target_node => fmt_constant_target_node,
        ConstantPathTargetNode => as_constant_path_target_node => fmt_constant_path_target_node,
        CallTargetNode => as_call_target_node => fmt_call_target_node,
        IndexTargetNode => as_index_target_node => fmt_index_target_node,
        MultiTargetNode => as_multi_target_node => fmt_multi_target_node,
    }
    formatted {
        StatementsNode => as_statements_node => fmt_statements_node,
        CallNode => as_call_node => fmt_call_node,
        SuperNode => as_super_node => fmt_super_node,
        ForwardingSuperNode => as_forwarding_super_node => fmt_forwarding_super_node,
        DefNode => as_def_node => fmt_def_node,
        RequiredParameterNode => as_required_parameter_node => fmt_required_parameter_node,
        OptionalParameterNode => as_optional_parameter_node => fmt_optional_parameter_node,
        RestParameterNode => as_rest_parameter_node => fmt_rest_parameter_node,
        ImplicitRestNode => as_implicit_rest_node => fmt_implicit_rest_node,
        RequiredKeywordParameterNode => as_required_keyword_parameter_node => fmt_required_keyword_parameter_node,
        OptionalKeywordParameterNode => as_optional_keyword_parameter_node => fmt_optional_keyword_parameter_node,
        KeywordRestParameterNode => as_keyword_rest_parameter_node => fmt_keyword_rest_parameter_node,
        NoKeywordsParameterNode => as_no_keywords_parameter_node => fmt_no_keywords_parameter_node,
        ForwardingParameterNode => as_forwarding_parameter_node => fmt_forwarding_parameter_node,
        BlockParameterNode => as_block_parameter_node => fmt_block_parameter_node,
        ParametersNode => as_parameters_node => fmt_parameters_node,
        LambdaNode => as_lambda_node => fmt_lambda_node,
        BlockParametersNode => as_block_parameters_node => fmt_block_parameters_node,
        BlockNode => as_block_node => fmt_block_node,
        YieldNode => as_yield_node => fmt_yield_node,
        ReturnNode => as_return_node => fmt_return_node,
        BreakNode => as_break_node => fmt_break_node,
        NextNode => as_next_node => fmt_next_node,
        LocalVariableWriteNode => as_local_variable_write_node => fmt_local_variable_write_node,
        LocalVariableOperatorWriteNode => as_local_variable_operator_write_node => fmt_local_variable_operator_write_node,
        LocalVariableOrWriteNode => as_local_variable_or_write_node => fmt_local_variable_or_write_node,
        LocalVariableAndWriteNode => as_local_variable_and_write_node => fmt_local_variable_and_write_node,
        InstanceVariableWriteNode => as_instance_variable_write_node => fmt_instance_variable_write_node,
        InstanceVariableOperatorWriteNode => as_instance_variable_operator_write_node => fmt_instance_variable_operator_write_node,
        InstanceVariableOrWriteNode => as_instance_variable_or_write_node => fmt_instance_variable_or_write_node,
        InstanceVariableAndWriteNode => as_instance_variable_and_write_node => fmt_instance_variable_and_write_node,
        ConstantWriteNode => as_constant_write_node => fmt_constant_write_node,
        ConstantOperatorWriteNode => as_constant_operator_write_node => fmt_constant_operator_write_node,
        ConstantOrWriteNode => as_constant_or_write_node => fmt_constant_or_write_node,
        ConstantAndWriteNode => as_constant_and_write_node => fmt_constant_and_write_node,
        GlobalVariableWriteNode => as_global_variable_write_node => fmt_global_variable_write_node,
        GlobalVariableOperatorWriteNode => as_global_variable_operator_write_node => fmt_global_variable_operator_write_node,
        GlobalVariableOrWriteNode => as_global_variable_or_write_node => fmt_global_variable_or_write_node,
        GlobalVariableAndWriteNode => as_global_variable_and_write_node => fmt_global_variable_and_write_node,
        ClassVariableWriteNode => as_class_variable_write_node => fmt_class_variable_write_node,
        ClassVariableOperatorWriteNode => as_class_variable_operator_write_node => fmt_class_variable_operator_write_node,
        ClassVariableOrWriteNode => as_class_variable_or_write_node => fmt_class_variable_or_write_node,
        ClassVariableAndWriteNode => as_class_variable_and_write_node => fmt_class_variable_and_write_node,
        ConstantPathWriteNode => as_constant_path_write_node => fmt_constant_path_write_node,
        ConstantPathOperatorWriteNode => as_constant_path_operator_write_node => fmt_constant_path_operator_write_node,
        ConstantPathOrWriteNode => as_constant_path_or_write_node => fmt_constant_path_or_write_node,
        ConstantPathAndWriteNode => as_constant_path_and_write_node => fmt_constant_path_and_write_node,
        MultiWriteNode => as_multi_write_node => fmt_multi_write_node,
        IndexOrWriteNode => as_index_or_write_node => fmt_index_or_write_node,
        IndexAndWriteNode => as_index_and_write_node => fmt_index_and_write_node,
        IndexOperatorWriteNode => as_index_operator_write_node => fmt_index_operator_write_node,
        CallOperatorWriteNode => as_call_operator_write_node => fmt_call_operator_write_node,
        CallOrWriteNode => as_call_or_write_node => fmt_call_or_write_node,
        CallAndWriteNode => as_call_and_write_node => fmt_call_and_write_node,
        ClassNode => as_class_node => fmt_class_node,
        ModuleNode => as_module_node => fmt_module_node,
        SingletonClassNode => as_singleton_class_node => fmt_singleton_class_node,
        ConstantPathNode => as_constant_path_node => fmt_constant_path_node,
        IfNode => as_if_node => fmt_if_node,
        ElseNode => as_else_node => fmt_else_node,
        BeginNode => as_begin_node => fmt_begin_node,
        RescueNode => as_rescue_node => fmt_rescue_node,
        RescueModifierNode => as_rescue_modifier_node => fmt_rescue_modifier_node,
        AliasMethodNode => as_alias_method_node => fmt_alias_method_node,
        AliasGlobalVariableNode => as_alias_global_variable_node => fmt_alias_global_variable_node,
        EnsureNode => as_ensure_node => fmt_ensure_node,
        UnlessNode => as_unless_node => fmt_unless_node,
        WhileNode => as_while_node => fmt_while_node,
        UntilNode => as_until_node => fmt_until_node,
        ForNode => as_for_node => fmt_for_node,
        CaseNode => as_case_node => fmt_case_node,
        WhenNode => as_when_node => fmt_when_node,
        CaseMatchNode => as_case_match_node => fmt_case_match_node,
        InNode => as_in_node => fmt_in_node,
        AndNode => as_and_node => fmt_and_node,
        OrNode => as_or_node => fmt_or_node,
        MatchRequiredNode => as_match_required_node => fmt_match_required_node,
        MatchPredicateNode => as_match_predicate_node => fmt_match_predicate_node,
        PreExecutionNode => as_pre_execution_node => fmt_pre_execution_node,
        PostExecutionNode => as_post_execution_node => fmt_post_execution_node,
        ShareableConstantNode => as_shareable_constant_node => fmt_shareable_constant_node,
    }
    // Copied through exactly as written. For most of these that *is*
    // the formatting: they are single tokens (`redo`, `it`,
    // `__ENCODING__`), or text whose whitespace is data (a `#{}` hole,
    // a regex literal used as a condition).
    verbatim {
        ArgumentsNode,
        BlockLocalVariableNode,
        EmbeddedStatementsNode,
        EmbeddedVariableNode,
        InterpolatedMatchLastLineNode,
        ItLocalVariableReadNode,
        MatchLastLineNode,
        MissingNode,
        RedoNode,
        RetryNode,
        SourceEncodingNode,
    }
}
