// New expression printing for TypeScript
//
// Handles: new Foo(), new Foo(arg1, arg2), new Foo<T>()

use super::calls::{
    PartitionedComments, build_args_split_last, has_inter_argument_comments_slice,
    has_trailing_line_comments_slice, wrap_call_with_hard_breaks, wrap_call_with_soft_breaks,
};
use super::utils::{
    is_block_function, is_function_composition_args, is_hopefully_short_arg,
    last_arg_is_array_or_object, preceding_args_allow_hug,
};
use super::{ParenContext, Printer, has_multiline_content, needs_parens};
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};

impl<'a> Printer<'a> {
    /// Build a Doc for a new expression with argument wrapping
    pub(super) fn build_new_doc_with_wrapping(&self, new_expr: &internal::NewExpression) -> Doc {
        // Wrap callee in parens if needed (e.g., `new (a || b)()`, `new (a ? b : c)()`)
        let callee = if needs_parens(&new_expr.callee, ParenContext::NewCallee) {
            // For binary expressions (including logical), use a group with softlines
            // so the parens can break independently when the content is too long:
            // new (
            //     a || b || c
            // )()
            //
            // Use ungrouped binary doc so the inner expression doesn't have its own
            // group - the outer group controls whether to break after `(`.
            if let internal::Expression::BinaryExpression(binary) = &*new_expr.callee {
                let inner_doc = self.build_binary_chain_doc_ungrouped(binary);
                doc::group(doc::concat(vec![
                    doc::text("("),
                    doc::indent_softline(inner_doc),
                    doc::softline(),
                    doc::text(")"),
                ]))
            } else {
                let callee_doc = self.build_expression_doc(&new_expr.callee);
                doc::parens(callee_doc)
            }
        } else {
            self.build_expression_doc(&new_expr.callee)
        };

        // Check for comments between removed parentheses and callee
        // e.g., new (/* comment */ Foo)() has comments in the gap between 'new ' and 'Foo'
        let callee = self.prepend_removed_paren_comments(
            new_expr.span.start,
            new_expr.callee.span().start,
            callee,
        );

        // Build type arguments: `<K, V>`
        let type_args_doc = new_expr
            .type_arguments
            .as_ref()
            .map(|ta| self.build_type_parameter_instantiation_doc(ta));

        // Empty args: just `new Foo()` or `new Foo<K, V>()`
        if new_expr.arguments.is_empty() {
            let mut parts = vec![doc::text("new "), callee];
            if let Some(ta_doc) = type_args_doc {
                parts.push(ta_doc);
            }
            parts.push(doc::text("()"));
            return doc::concat(parts);
        }

        // Build callee with type args: `new Foo<K, V>`
        let callee_with_types = match type_args_doc {
            Some(ta_doc) => doc::concat(vec![doc::text("new "), callee, ta_doc]),
            None => doc::concat(vec![doc::text("new "), callee]),
        };

        // Single huggable argument: object literal or function
        // These stay on the same line as the opening paren: `new Cls({...})` not `new Cls(\n{...})`
        if new_expr.arguments.len() == 1 {
            match &new_expr.arguments[0] {
                // Object literal: hug it
                internal::Expression::ObjectExpression(_) => {
                    return doc::concat(vec![
                        callee_with_types,
                        doc::text("("),
                        self.build_expression_doc(&new_expr.arguments[0]),
                        doc::text(")"),
                    ]);
                }
                // Array literal: hug it
                internal::Expression::ArrayExpression(_) => {
                    return doc::concat(vec![
                        callee_with_types,
                        doc::text("("),
                        self.build_expression_doc(&new_expr.arguments[0]),
                        doc::text(")"),
                    ]);
                }
                // Block arrow function: use conditional_group to let Doc decide hug vs wrap
                internal::Expression::ArrowFunctionExpression(arrow)
                    if !arrow.body.is_expression() =>
                {
                    let arrow_doc = self.build_expression_doc(&new_expr.arguments[0]);
                    return doc::conditional_group(vec![
                        // State 1: hugged - new Callee((arrow) => { body })
                        doc::concat(vec![
                            callee_with_types.clone(),
                            doc::text("("),
                            arrow_doc.clone(),
                            doc::text(")"),
                        ]),
                        // State 2: wrapped - new Callee(\n\t(arrow) => { body },\n)
                        doc::concat(vec![
                            callee_with_types,
                            doc::text("("),
                            doc::indent(doc::concat(vec![
                                doc::softline(),
                                arrow_doc,
                                doc::text(","),
                            ])),
                            doc::softline(),
                            doc::text(")"),
                        ]),
                    ]);
                }
                // Function expression: hug it
                internal::Expression::FunctionExpression(_) => {
                    return doc::concat(vec![
                        callee_with_types,
                        doc::text("("),
                        self.build_expression_doc(&new_expr.arguments[0]),
                        doc::text(")"),
                    ]);
                }
                _ => {}
            }
        }

        // Function composition pattern: when any argument is a call containing a callback
        // OR when there are multiple function arguments
        // e.g., new Cls(arr.map((x) => x), b) → new Cls(\n\t...,\n)
        // e.g., new Cls(() => a, () => b) → new Cls(\n\t...,\n)
        if is_function_composition_args(&new_expr.arguments) {
            let arg_parts = doc::join_doc(
                new_expr
                    .arguments
                    .iter()
                    .map(|arg| self.build_arg_expression_doc(arg)),
                doc::comma_hardline(),
            );

            return wrap_call_with_hard_breaks(callee_with_types, arg_parts);
        }

        // Check if any argument has multiline content
        let has_multiline = new_expr
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, self.source));

        if has_multiline {
            // Force expansion with hardlines for multiline content
            let arg_parts = doc::join_doc(
                new_expr
                    .arguments
                    .iter()
                    .map(|arg| self.build_arg_expression_doc(arg)),
                doc::comma_hardline(),
            );

            return wrap_call_with_hard_breaks(callee_with_types, arg_parts);
        }

        // "Expand first arg" pattern: callback first, short/empty container last
        // e.g., new Proxy((x) => { ... }, {}) - callback hugs, empty obj stays inline
        if new_expr.arguments.len() == 2 {
            let first_arg = &new_expr.arguments[0];
            let second_arg = &new_expr.arguments[1];

            if is_block_function(first_arg) && is_short_second_arg(second_arg, self) {
                let first_arg_doc = self.build_expression_doc(first_arg);
                let second_arg_doc = self.build_expression_doc(second_arg);

                return doc::concat(vec![
                    callee_with_types,
                    doc::text("("),
                    first_arg_doc,
                    doc::text(", "),
                    second_arg_doc,
                    doc::text(")"),
                ]);
            }
        }

        // Check for trailing line comments on arguments (forces expansion)
        // Must check this BEFORE the "last arg is array/object" pattern below,
        // otherwise trailing comments on the last arg cause it to be hugged incorrectly.
        // e.g., new Class(arg1, // comment\n  arg2)
        if has_trailing_line_comments_slice(&new_expr.arguments, new_expr.span.end, self) {
            let mut arg_parts = Vec::new();
            let mut has_trailing_comma_on_last = false;

            for (i, arg) in new_expr.arguments.iter().enumerate() {
                // Build the argument (use build_expression_doc to match calls.rs comment handling)
                arg_parts.push(self.build_expression_doc(arg));

                // Check for comments after this argument
                if i < new_expr.arguments.len() - 1 {
                    let arg_end = arg.span().end;
                    let next_arg_start = new_expr.arguments[i + 1].span().start;

                    let pc = PartitionedComments::new(
                        self.comments,
                        self.line_breaks,
                        arg_end,
                        next_arg_start,
                    );

                    arg_parts.push(doc::text(","));
                    pc.emit_trailing_comments(&mut arg_parts, self);
                    arg_parts.push(doc::hardline());
                    pc.emit_leading_comments(&mut arg_parts, self);
                } else {
                    // Last argument - check for trailing line comments before closing paren
                    let arg_end = arg.span().end;
                    let paren_close = new_expr.span.end;

                    let pc = PartitionedComments::new(
                        self.comments,
                        self.line_breaks,
                        arg_end,
                        paren_close,
                    );

                    if pc.has_trailing_line() {
                        // Add comma before trailing comments
                        arg_parts.push(doc::text(","));
                        pc.emit_trailing_comments(&mut arg_parts, self);
                        has_trailing_comma_on_last = true;
                    }
                }
            }

            let arg_doc = doc::concat(arg_parts);
            let trailing = if has_trailing_comma_on_last {
                doc::empty()
            } else {
                doc::text(",")
            };

            return doc::concat(vec![
                callee_with_types,
                doc::text("("),
                doc::indent(doc::concat(vec![doc::hardline(), arg_doc, trailing])),
                doc::hardline(),
                doc::text(")"),
            ]);
        }

        // "First args inline with last array/object" pattern (same as CallExpression):
        // When last arg is array/object and preceding args are short,
        // keep short args inline with the opening bracket/brace.
        // NOTE: This must come AFTER the trailing comment check above.
        if new_expr.arguments.len() >= 2
            && last_arg_is_array_or_object(&new_expr.arguments)
            && preceding_args_allow_hug(&new_expr.arguments, self.line_breaks)
            && !has_inter_argument_comments_slice(&new_expr.arguments, self)
        {
            let (head_parts, last_arg_doc, _) = build_args_split_last(&new_expr.arguments, self);

            // Keep short args inline with last arg's opener
            return doc::group(doc::concat(vec![
                callee_with_types,
                doc::text("("),
                doc::concat(head_parts),
                last_arg_doc,
                doc::text(")"),
            ]));
        }

        // Build args with line separators (one per line when broken)
        let arg_parts = doc::join_doc(
            new_expr
                .arguments
                .iter()
                .map(|arg| self.build_arg_expression_doc(arg)),
            doc::comma_line(),
        );

        // Wrap in group with parens
        wrap_call_with_soft_breaks(callee_with_types, arg_parts)
    }

    /// Build a Doc for a new expression (for nested contexts)
    pub(super) fn build_new_doc(&self, new_expr: &internal::NewExpression) -> Doc {
        self.build_new_doc_with_wrapping(new_expr)
    }
}

/// Check if second arg is short enough for "expand first arg" pattern in new expressions.
///
/// Allows: simple values (identifiers, literals, etc.) and empty {} or []
/// Rejects: functions, ternaries, spreads, non-empty objects/arrays, objects/arrays with comments
fn is_short_second_arg(arg: &internal::Expression, printer: &Printer) -> bool {
    match arg {
        // Functions, ternaries, spreads - these should expand all args
        internal::Expression::ArrowFunctionExpression(_)
        | internal::Expression::FunctionExpression(_)
        | internal::Expression::ConditionalExpression(_)
        | internal::Expression::SpreadElement(_) => false,
        // Non-empty objects expand - use "expand all args" instead
        internal::Expression::ObjectExpression(obj) if !obj.properties.is_empty() => false,
        // Non-empty arrays expand - use "expand all args" instead
        internal::Expression::ArrayExpression(arr) if !arr.elements.is_empty() => false,
        // Empty {} or [] with comments inside should expand
        internal::Expression::ObjectExpression(obj)
            if printer.has_comments_between(obj.span.start, obj.span.end) =>
        {
            false
        }
        internal::Expression::ArrayExpression(arr)
            if printer.has_comments_between(arr.span.start, arr.span.end) =>
        {
            false
        }
        // Truly empty {} and [] are short
        internal::Expression::ObjectExpression(_) | internal::Expression::ArrayExpression(_) => {
            true
        }
        // Other args: check if "hopefully short"
        _ => is_hopefully_short_arg(arg),
    }
}
