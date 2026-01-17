// New expression printing for TypeScript
//
// Handles: new Foo(), new Foo(arg1, arg2), new Foo<T>()

use super::calls::{
    build_args_split_last, has_inter_argument_comments_slice, wrap_call_with_hard_breaks,
    wrap_call_with_soft_breaks,
};
use super::utils::{
    has_multiple_function_args, last_arg_is_array_or_object, preceding_args_are_short,
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

        // Multiple arrow/function arguments: always break (Prettier behavior)
        // e.g., new Cls(() => a, () => b) → new Cls(\n\t() => a,\n\t() => b,\n)
        if has_multiple_function_args(&new_expr.arguments) {
            let arg_docs: Vec<_> = new_expr
                .arguments
                .iter()
                .map(|arg| self.build_expression_doc(arg))
                .collect();
            let arg_parts = doc::join_doc(arg_docs, doc::comma_hardline());

            return wrap_call_with_hard_breaks(callee_with_types, arg_parts);
        }

        // Check if any argument has multiline content
        let has_multiline = new_expr
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, self.source));

        if has_multiline {
            // Force expansion with hardlines for multiline content
            let arg_docs: Vec<_> = new_expr
                .arguments
                .iter()
                .map(|arg| self.build_expression_doc(arg))
                .collect();
            let arg_parts = doc::join_doc(arg_docs, doc::comma_hardline());

            return wrap_call_with_hard_breaks(callee_with_types, arg_parts);
        }

        // "First args inline with last array/object" pattern (same as CallExpression):
        // When last arg is array/object and preceding args are short,
        // keep short args inline with the opening bracket/brace
        if new_expr.arguments.len() >= 2 && last_arg_is_array_or_object(&new_expr.arguments) {
            // Skip this pattern if there are inter-argument comments
            let has_inter_arg_comments =
                has_inter_argument_comments_slice(&new_expr.arguments, self);

            if preceding_args_are_short(&new_expr.arguments) && !has_inter_arg_comments {
                let (head_parts, last_arg_doc, _) =
                    build_args_split_last(&new_expr.arguments, self);

                // Keep short args inline with last arg's opener
                return doc::group(doc::concat(vec![
                    callee_with_types,
                    doc::text("("),
                    doc::concat(head_parts),
                    last_arg_doc,
                    doc::text(")"),
                ]));
            }
        }

        // Build args with line separators (one per line when broken)
        let arg_docs: Vec<_> = new_expr
            .arguments
            .iter()
            .map(|arg| self.build_expression_doc(arg))
            .collect();
        let arg_parts = doc::join_doc(arg_docs, doc::comma_line());

        // Wrap in group with parens
        wrap_call_with_soft_breaks(callee_with_types, arg_parts)
    }

    /// Build a Doc for a new expression (for nested contexts)
    pub(super) fn build_new_doc(&self, new_expr: &internal::NewExpression) -> Doc {
        self.build_new_doc_with_wrapping(new_expr)
    }
}
