// Call and member expression printing for TypeScript
//
// Handles printing of:
// - Call expressions: `foo()`, `obj.method(arg1, arg2)`
// - Member expressions: `obj.prop`, `arr[0]`
// - Method chains: `arr.filter().map()`
// - Test function calls: `it()`, `test.skip()`, `describe()`, etc.
// - Import expressions: `import('module')`, `import('module', options)`
//
// ## Module Organization
//
// - **mod.rs** (this file): Re-exports and entry point methods
// - **test_patterns.rs**: Test function detection (Jest, Mocha, Playwright, etc.)
// - **module_paths.rs**: Module path patterns (require, import.meta)
// - **arg_comments.rs**: Comment handling in argument lists
// - **arg_predicates.rs**: Call-argument and arrow shape predicates
// - **arg_wrapping.rs**: Argument classification and wrapping utilities
// - **call_formatting.rs**: Main call expression formatting logic
// - **new_expression.rs**: `new` expression formatting (shares the call wrapping patterns)
// - **import_expr.rs**: Import expression and meta property handling
// - **chain_args.rs**: Chain-specific argument building

mod arg_comments;
pub(in crate::printer) mod arg_predicates;
mod arg_wrapping;
mod call_formatting;
mod chain_args;
mod import_expr;
mod module_paths;
mod new_expression;
mod test_patterns;

// Re-export items needed by other printer modules
pub(crate) use arg_comments::{
    PartitionedComments, emit_first_arg_leading_comments, has_blank_line_between_args,
    has_inter_argument_comments_slice, has_trailing_comments_slice,
    has_trailing_line_comments_slice, should_force_expansion_for_comments,
    skip_stripped_open_paren,
};
pub(crate) use arg_wrapping::{
    build_args_joined_with_comments, build_args_split_last, build_arrow_call_body_states,
    build_arrow_sig_doc, build_break_body_state, build_expand_all_args, build_inline_args,
    build_inline_or_expand_all, could_expand_arrow_chain, last_two_args_same_type,
    prebuild_expand_last_break_body, prepend_arrow_body_comments, wrap_call_with_hard_breaks,
    wrap_call_with_will_break_guard,
};

use super::Printer;
use super::chain;
use crate::ast::internal;
use arg_comments::{any_comment_forces_expansion, last_arg_has_comments};
use arg_predicates::{is_block_function, preceding_args_allow_expand_last};
use tsv_lang::doc::arena::DocId;

/// Check if a chain expression contains any call expressions
fn chain_has_calls(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::CallExpression(_) => true,
        internal::Expression::MemberExpression(member) => chain_has_calls(member.object),
        internal::Expression::TSNonNullExpression(non_null) => chain_has_calls(non_null.expression),
        // Look through await/yield to find nested calls: (await fn()).method()
        internal::Expression::AwaitExpression(await_expr) => chain_has_calls(await_expr.argument),
        internal::Expression::YieldExpression(yield_expr) => yield_expr
            .argument
            .as_ref()
            .is_some_and(|arg| chain_has_calls(arg)),
        _ => false,
    }
}

/// Check if callee is a member expression (used for chain detection)
fn is_memberish(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::MemberExpression(_) | internal::Expression::TSNonNullExpression(_)
    )
}

impl<'a> Printer<'a> {
    /// Build a Doc for a call expression with argument wrapping (not chain-aware)
    pub(super) fn build_call_doc_with_wrapping(
        &self,
        call: &internal::CallExpression<'_>,
    ) -> DocId {
        call_formatting::build_call_doc_with_wrapping(self, call)
    }

    /// Build a Doc for a call expression (for nested contexts)
    ///
    /// Uses the chain module for:
    /// 1. True chains (callee contains nested calls, like `a().b()`)
    /// 2. Memberish callees with comments between member segments
    ///
    /// Simple calls like `obj.method()` use the simple call path unless they have
    /// comments between member segments.
    pub(super) fn build_call_doc(&self, call: &internal::CallExpression<'_>) -> DocId {
        // Curried call with callback pattern: fn()('arg', () => { ... })
        // When the callee is a simple call expression and the last argument is a
        // block function, use conditional_group to try inline first, then expand-all.
        //
        // Skip when the inner call has array/object args — those may force multiline,
        // and the chain formatter handles that correctly via group(oneLine).
        if let internal::Expression::CallExpression(inner) = call.callee {
            let inner_has_multiline_arg = inner.arguments.iter().any(|arg| {
                matches!(
                    arg,
                    internal::Expression::ArrayExpression(_)
                        | internal::Expression::ObjectExpression(_)
                )
            });
            let has_blank_lines_between_args = call.arguments.windows(2).any(|w| {
                has_blank_line_between_args(
                    self.source,
                    self.line_breaks,
                    w[0].span().end,
                    w[1].span().start,
                )
            });
            let paren_open = call.callee.span().end;
            // Whole-call comment-presence gate (one binary search over the argument
            // window); short-circuits the comment predicates below and threads into
            // build_args_split_last. Canonical reference: build_params_doc_with_comments.
            let call_has_comments = self.has_comments_between(paren_open, call.span.end);
            if call.arguments.len() >= 2
                && call.arguments.last().is_some_and(is_block_function)
                && preceding_args_allow_expand_last(call.arguments, self.line_breaks)
                && !has_blank_lines_between_args
                && !(call_has_comments && any_comment_forces_expansion(call, self, paren_open))
                && !(call_has_comments
                    && last_arg_has_comments(call.arguments, self, call.span.end, paren_open))
                && !inner_has_multiline_arg
            {
                let d = self.d();
                let callee_doc = self.build_expression_doc(call.callee);

                // Build args split into head (with commas) and last
                // Leading comments before first arg are handled inside build_args_split_last
                let (head_parts, last_arg_doc, all_args_broken) =
                    build_args_split_last(call.arguments, self, paren_open, call_has_comments);

                let state_inline = d.concat(&[
                    callee_doc,
                    d.text("("),
                    d.concat(&head_parts),
                    last_arg_doc,
                    d.text(")"),
                ]);
                let state_expand_all = d.concat(&[
                    callee_doc,
                    d.text("("),
                    d.indent(d.concat(&[d.line(), all_args_broken])),
                    d.line(),
                    d.text(")"),
                ]);

                return d.conditional_group(&[state_inline, state_expand_all]);
            }
        }

        // Test function calls (it.skip, test.only, etc.) stay on one line even
        // if they exceed print width. Must check BEFORE chain routing, because
        // memberish callees like `it.skip(...)` would otherwise be routed through
        // the chain path which doesn't know about test call special-casing.
        if test_patterns::is_test_call(call, self) {
            return self.build_call_doc_with_wrapping(call);
        }

        // Check if this is a true chain (callee contains calls, like `a().b()`)
        let is_true_chain = chain_has_calls(call.callee);

        // For memberish callees, use chain module to format the entire call expression.
        // This ensures proper handling of member chains in assignments - the chain module
        // returns group(oneLine) for short chains, letting the assignment's Fluid layout
        // decide whether to break after `=`.
        //
        // Without this, the callee is formatted separately as a member chain with
        // conditional_group/fill that has internal break points, causing the chain
        // to break before the assignment breaks.
        let callee_is_memberish = is_memberish(call.callee);

        if is_true_chain || callee_is_memberish {
            // Use chain wrapping for chains (nested calls) or memberish callees
            let nodes = chain::linearize_chain_from_call(call);
            let base_start = get_chain_base_comment_start(&nodes, call.callee);
            let groups = chain::group_chain_nodes(&nodes);
            let chain_doc = chain::build_chain_doc(&groups, call.span, self);
            self.prepend_removed_paren_comments(call.span.start, base_start, chain_doc)
        } else {
            // Simple call (non-memberish callee) - wrap args directly
            self.build_call_doc_with_wrapping(call)
        }
    }

    /// Build a Doc for a member expression with optional breaking at dots
    ///
    /// Uses the new chain architecture based on prettier's member-chain.js:
    /// 1. Linearize AST into flat list of chain nodes
    /// 2. Group nodes by natural break points
    /// 3. Build doc with conditionalGroup for oneLine/expanded alternatives
    pub(super) fn build_member_doc(&self, member: &internal::MemberExpression<'_>) -> DocId {
        // A format-ignore directive attached to this member access (in the gap between
        // the object and the property) makes prettier print the entire member
        // expression verbatim from source — preserving inner call args (numbers,
        // etc.) that the chain formatter would otherwise reformat. Mirrors
        // prettier's `hasPrettierIgnore` → verbatim-print behavior.
        if self.has_format_ignore_in_range(member.object.span().end, member.property.span().start) {
            return self.raw_source_doc(member.span);
        }

        // Use chain-based implementation
        let nodes = chain::linearize_chain_from_member(member);
        let base_start = get_chain_base_comment_start(&nodes, member.object);
        let groups = chain::group_chain_nodes(&nodes);
        let chain_doc = chain::build_chain_doc(&groups, member.span, self);

        // Prepend comments from removed parentheses at the chain base.
        // For call chains, base_start excludes paren gaps handled mid-chain.
        self.prepend_removed_paren_comments(member.span.start, base_start, chain_doc)
    }

    /// Build a Doc for a dynamic import expression: `import('module')` or `import('module', options)`
    pub(super) fn build_import_expression_doc(
        &self,
        import_expr: &internal::ImportExpression<'_>,
    ) -> DocId {
        import_expr::build_import_expression_doc(self, import_expr)
    }

    /// Build a Doc for a meta property: `import.meta`, `new.target`
    pub(super) fn build_meta_property_doc(&self, meta: &internal::MetaProperty<'_>) -> DocId {
        import_expr::build_meta_property_doc(self, meta)
    }

    /// Build a Doc for call arguments only (for chain printing)
    ///
    /// Uses proper group wrapping so args can break independently from the chain.
    pub(super) fn build_call_args_doc_for_chain(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain(self, call, optional)
    }

    /// Build a Doc for call arguments with forced expansion (hardlines instead of softlines)
    ///
    /// Used for the "args expanded, chain inline" state in conditionalGroup.
    pub(super) fn build_call_args_doc_for_chain_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain_expanded(self, call, optional)
    }

    /// Build a Doc for call arguments with standard forced expansion
    ///
    /// Always uses `(\n  args,\n)` form, never arrow-hugging `(sig =>\n  body,\n)`.
    pub(super) fn build_call_args_doc_for_chain_standard_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain_standard_expanded(self, call, optional)
    }
}

/// Get the comment boundary for prepend_removed_paren_comments in chains.
///
/// When linearization extends a member's object_end backward to cover a paren gap
/// (indicated by object_end < base_start), returns that extended position so
/// prepend_removed_paren_comments won't double-print comments already handled
/// mid-chain. Falls back to the normal base start otherwise.
///
/// Only applies to call chains: prettier places comments mid-chain only when the
/// chain has calls. Member-only chains keep all comments before the chain base.
fn get_chain_base_comment_start(
    nodes: &[chain::ChainNode<'_>],
    expr: &internal::Expression<'_>,
) -> u32 {
    let base_start = get_chain_base_start(expr);
    // Only check for extended ranges in call chains — prettier doesn't
    // place comments mid-chain for member-only chains
    let has_calls = nodes.iter().any(chain::ChainNode::is_call);
    if has_calls {
        for node in nodes {
            if let Some((object_end, _)) = node.comment_range()
                && object_end < base_start
            {
                // This member's range was extended by linearization to cover
                // a paren gap — prepend should stop here to avoid duplication
                return object_end;
            }
        }
    }
    base_start
}

/// Get the start position of the innermost base expression in a chain
fn get_chain_base_start(expr: &internal::Expression<'_>) -> u32 {
    match expr {
        internal::Expression::MemberExpression(member) => get_chain_base_start(member.object),
        internal::Expression::CallExpression(call) => get_chain_base_start(call.callee),
        internal::Expression::TSNonNullExpression(non_null) => {
            get_chain_base_start(non_null.expression)
        }
        // Note: TaggedTemplateExpression is NOT traversed here because its own
        // build_tagged_template_doc handles comments from removed parentheses
        _ => expr.span().start,
    }
}
