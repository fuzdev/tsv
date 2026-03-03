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
// - **arg_wrapping.rs**: Argument classification and wrapping utilities
// - **call_formatting.rs**: Main call expression formatting logic
// - **import_expr.rs**: Import expression and meta property handling
// - **chain_args.rs**: Chain-specific argument building
// - **chain_printer.rs**: ChainPrinter trait implementation

mod arg_comments;
mod arg_wrapping;
mod call_formatting;
mod chain_args;
mod chain_printer;
mod import_expr;
mod module_paths;
mod test_patterns;

// Re-export items needed by other printer modules
pub(crate) use arg_comments::{
    PartitionedComments, find_comma_pos, has_inter_argument_comments_slice,
    has_trailing_comments_slice, has_trailing_line_comments_slice, is_comment_after_comma,
    is_comment_before_comma,
};
pub(crate) use arg_wrapping::{
    arrow_has_type_annotations, build_args_split_last, build_arrow_call_body_states,
    build_arrow_inline_signature, wrap_call_with_hard_breaks, wrap_call_with_soft_breaks,
};

use super::Printer;
use super::chain;
use super::utils::{is_block_function, preceding_args_allow_expand_last};
use crate::ast::internal;
use arg_comments::any_comment_forces_expansion;
use tsv_lang::doc::arena::DocId;

/// Check if a chain expression contains any call expressions
fn chain_has_calls(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::CallExpression(_) => true,
        internal::Expression::MemberExpression(member) => chain_has_calls(&member.object),
        internal::Expression::TSNonNullExpression(non_null) => {
            chain_has_calls(&non_null.expression)
        }
        // Look through await/yield to find nested calls: (await fn()).method()
        internal::Expression::AwaitExpression(await_expr) => chain_has_calls(&await_expr.argument),
        internal::Expression::YieldExpression(yield_expr) => yield_expr
            .argument
            .as_ref()
            .is_some_and(|arg| chain_has_calls(arg)),
        _ => false,
    }
}

/// Check if callee is a member expression (used for chain detection)
fn is_memberish(expr: &internal::Expression) -> bool {
    matches!(
        expr,
        internal::Expression::MemberExpression(_) | internal::Expression::TSNonNullExpression(_)
    )
}

impl<'a> Printer<'a> {
    /// Build a Doc for a call expression with argument wrapping (not chain-aware)
    pub(super) fn build_call_doc_with_wrapping(&self, call: &internal::CallExpression) -> DocId {
        call_formatting::build_call_doc_with_wrapping(self, call)
    }

    /// Build a Doc for a chain (method chain or member chain) with wrapping
    ///
    /// Uses the chain module's grouping and doc building logic for proper
    /// member chain formatting, including the 3+ calls rule.
    fn build_chain_doc_with_wrapping(&self, expr: &internal::Expression) -> DocId {
        let nodes = chain::linearize_chain(expr);
        let groups = chain::group_chain_nodes(nodes);
        let chain_doc = chain::build_chain_doc(&groups, self);

        // Prepend comments from removed parentheses at the chain base
        // e.g., (/* comment */ obj).prop.method()
        let base_start = get_chain_base_start(expr);
        self.prepend_removed_paren_comments(expr.span().start, base_start, chain_doc)
    }

    /// Build a Doc for a call expression (for nested contexts)
    ///
    /// Uses the chain module for:
    /// 1. True chains (callee contains nested calls, like `a().b()`)
    /// 2. Memberish callees with comments between member segments
    ///
    /// Simple calls like `obj.method()` use the simple call path unless they have
    /// comments between member segments.
    pub(super) fn build_call_doc(&self, call: &internal::CallExpression) -> DocId {
        // Curried call with callback pattern: fn()('arg', () => { ... })
        // When the callee is a call expression (curried call) and the last argument
        // is a block function, use conditional_group to try inline first, then expand-all.
        // This matches Prettier's behavior for test.each() and similar patterns.
        // Must check BEFORE chain handling to bypass chain logic.
        // Skip if there are blank lines between args or comments that force expansion
        // (line comments or block comments on their own line - inline block comments are OK).
        let has_blank_lines_between_args = call
            .arguments
            .windows(2)
            .any(|w| self.has_blank_line_between(w[0].span().end, w[1].span().start));
        let paren_open = call.callee.span().end;
        if matches!(&*call.callee, internal::Expression::CallExpression(_))
            && call.arguments.len() >= 2
            && call.arguments.last().is_some_and(is_block_function)
            && preceding_args_allow_expand_last(&call.arguments, self.line_breaks)
            && !has_blank_lines_between_args
            && !any_comment_forces_expansion(call, self, paren_open)
        {
            let d = self.d();
            let callee_doc = self.build_expression_doc(&call.callee);
            let first_arg_start = call.arguments[0].span().start;

            // Build args split into head (with commas) and last
            let (head_parts, last_arg_doc, all_args_broken) =
                build_args_split_last(&call.arguments, self);

            // Build inline state with optional leading comments
            let leading_comments = self
                .build_inline_comments_between_doc_trailing_space_opt(paren_open, first_arg_start);
            let mut inline_inner: Vec<DocId> = Vec::new();
            if let Some(comment) = leading_comments {
                inline_inner.push(comment);
            }
            inline_inner.extend(head_parts);
            inline_inner.push(last_arg_doc);

            let state_inline = d.concat(&[
                callee_doc,
                d.text("("),
                d.concat(&inline_inner),
                d.text(")"),
            ]);
            let state_expand_all = d.concat(&[
                callee_doc,
                d.text("("),
                d.indent(d.concat(&[d.line(), all_args_broken, d.text(",")])),
                d.line(),
                d.text(")"),
            ]);

            return d.conditional_group(&[state_inline, state_expand_all]);
        }

        // Test function calls (it.skip, test.only, etc.) stay on one line even
        // if they exceed print width. Must check BEFORE chain routing, because
        // memberish callees like `it.skip(...)` would otherwise be routed through
        // the chain path which doesn't know about test call special-casing.
        if test_patterns::is_test_call(call, self) {
            return self.build_call_doc_with_wrapping(call);
        }

        // Check if this is a true chain (callee contains calls, like `a().b()`)
        let is_true_chain = chain_has_calls(&call.callee);

        // For memberish callees, use chain module to format the entire call expression.
        // This ensures proper handling of member chains in assignments - the chain module
        // returns group(oneLine) for short chains, letting the assignment's Fluid layout
        // decide whether to break after `=`.
        //
        // Without this, the callee is formatted separately as a member chain with
        // conditional_group/fill that has internal break points, causing the chain
        // to break before the assignment breaks.
        let callee_is_memberish = is_memberish(&call.callee);

        if is_true_chain || callee_is_memberish {
            // Use chain wrapping for chains (nested calls) or memberish callees
            self.build_chain_doc_with_wrapping(&internal::Expression::CallExpression(call.clone()))
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
    pub(super) fn build_member_doc(&self, member: &internal::MemberExpression) -> DocId {
        // Use chain-based implementation
        let expr = internal::Expression::MemberExpression(member.clone());
        let nodes = chain::linearize_chain(&expr);
        let groups = chain::group_chain_nodes(nodes);
        let chain_doc = chain::build_chain_doc(&groups, self);

        // Prepend comments from removed parentheses at the chain base
        // e.g., (/* comment */ obj).prop has member.span.start at '(' and object.span.start at 'obj'
        let base_start = get_chain_base_start(&member.object);
        self.prepend_removed_paren_comments(member.span.start, base_start, chain_doc)
    }

    /// Build a Doc for a dynamic import expression: `import('module')` or `import('module', options)`
    pub(super) fn build_import_expression_doc(
        &self,
        import_expr: &internal::ImportExpression,
    ) -> DocId {
        import_expr::build_import_expression_doc(self, import_expr)
    }

    /// Build a Doc for a meta property: `import.meta`, `new.target`
    pub(super) fn build_meta_property_doc(&self, meta: &internal::MetaProperty) -> DocId {
        import_expr::build_meta_property_doc(self, meta)
    }

    /// Build a Doc for call arguments only (for chain printing)
    ///
    /// Uses proper group wrapping so args can break independently from the chain.
    pub(super) fn build_call_args_doc_for_chain(
        &self,
        call: &internal::CallExpression,
        optional: bool,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain(self, call, optional)
    }

    /// Build a Doc for call arguments with forced expansion (hardlines instead of softlines)
    ///
    /// Used for the "args expanded, chain inline" state in conditionalGroup.
    pub(super) fn build_call_args_doc_for_chain_expanded(
        &self,
        call: &internal::CallExpression,
        optional: bool,
    ) -> DocId {
        chain_args::build_call_args_doc_for_chain_expanded(self, call, optional)
    }
}

/// Get the start position of the innermost base expression in a chain
fn get_chain_base_start(expr: &internal::Expression) -> u32 {
    match expr {
        internal::Expression::MemberExpression(member) => get_chain_base_start(&member.object),
        internal::Expression::CallExpression(call) => get_chain_base_start(&call.callee),
        internal::Expression::TSNonNullExpression(non_null) => {
            get_chain_base_start(&non_null.expression)
        }
        // Note: TaggedTemplateExpression is NOT traversed here because its own
        // build_tagged_template_doc handles comments from removed parentheses
        _ => expr.span().start,
    }
}
