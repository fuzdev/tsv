// Function expression printing for TypeScript
//
// This module handles function-related expressions:
// - Arrow function expressions (async, parameters, return types, body)
// - Function expressions (parameters, return types, body)
//
// Note: Block statements are in blocks.rs as a reusable utility

use super::super::{CommentSpacing, ParenContext, Printer, needs_parens};
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};

/// Check if an expression is directly an object literal (needs parentheses in arrow body)
/// Only returns true for direct ObjectExpression - TSAsExpression etc. are handled separately
fn is_object_expression(expr: &internal::Expression) -> bool {
    matches!(expr, internal::Expression::ObjectExpression(_))
}

/// Check if an expression is a type assertion (as/satisfies) wrapping an object literal
/// Returns the inner expression type for special handling in arrow body context
fn get_type_assertion_with_object(
    expr: &internal::Expression,
) -> Option<TypeAssertionWithObject<'_>> {
    match expr {
        internal::Expression::TSAsExpression(as_expr)
            if is_object_expression(&as_expr.expression) =>
        {
            Some(TypeAssertionWithObject::As(as_expr))
        }
        internal::Expression::TSSatisfiesExpression(sat_expr)
            if is_object_expression(&sat_expr.expression) =>
        {
            Some(TypeAssertionWithObject::Satisfies(sat_expr))
        }
        _ => None,
    }
}

enum TypeAssertionWithObject<'a> {
    As(&'a internal::TSAsExpression),
    Satisfies(&'a internal::TSSatisfiesExpression),
}

impl<'a> Printer<'a> {
    /// Print an arrow function expression using doc-based formatting with width-aware wrapping.
    ///
    /// Prettier behavior for arrow functions:
    /// - If signature fits on one line, keep inline
    /// - If too long, break type params and/or function params
    /// - Type params and function params share a group - when one breaks, both break
    /// - Trailing commas are added when params are broken across lines
    pub(super) fn print_arrow_function(&mut self, arrow: &internal::ArrowFunctionExpression) {
        let arrow_doc = self.build_arrow_doc_wrapping(arrow);
        self.write_doc(&arrow_doc);
    }

    /// Build a Doc for an arrow function with width-aware wrapping.
    ///
    /// Prettier's algorithm (from arrow-function.js and function-parameters.js):
    /// 1. Type params are wrapped in their OWN group - they break independently
    /// 2. Function params are NOT in their own group - just softlines
    /// 3. The whole signature (type params group + params + return type) is wrapped in a group
    /// 4. When the signature group breaks, params break but type params may stay flat
    ///
    /// Structure:
    /// ```text
    /// group([
    ///     group(type_params),  // inner group - breaks independently
    ///     "(", indent([softline, params...]), ifBreak(","), softline, ")",
    ///     return_type,
    ///     " =>"
    /// ])
    /// " " + body
    /// ```
    fn build_arrow_doc_wrapping(&self, arrow: &internal::ArrowFunctionExpression) -> Doc {
        let mut parts = Vec::new();

        // Async keyword if present
        if arrow.r#async {
            parts.push(doc::text("async "));
        }

        // Build signature parts (will be wrapped in a group)
        let mut sig_parts = Vec::new();

        let has_params = !arrow.params.is_empty();

        // Type parameters handling depends on whether there are function params
        if let Some(tp) = &arrow.type_parameters {
            if has_params {
                // Function params exist - type params in their own group so they break independently
                sig_parts.push(self.build_type_params_doc_for_arrow_grouped(tp));
            } else {
                // No function params - type params NOT in own group, break with outer signature
                sig_parts.push(self.build_type_params_doc_for_arrow_ungrouped(tp));
            }
        }

        // Function parameters - NOT in their own group, just softlines
        // These will break when the outer signature group breaks
        sig_parts.push(self.build_arrow_params_doc_ungrouped(&arrow.params, arrow.params_start));

        // Return type annotation - union types need special handling for breaking
        if let Some(return_type) = &arrow.return_type {
            sig_parts.push(self.build_arrow_return_type_doc(return_type));
        }

        // Include " =>" in the signature group for width calculation
        sig_parts.push(doc::text(" =>"));

        // Wrap entire signature in a group
        parts.push(doc::group(doc::concat(sig_parts)));

        // Calculate signature end position for comment detection
        // This is the position after which comments belong to the body (between => and body)
        let sig_end = if let Some(rt) = &arrow.return_type {
            rt.span.end
        } else if let Some(last_param) = arrow.params.last() {
            // After last param, but we need to account for the closing `)`
            // We'll use the body start and check backwards
            last_param.span().end
        } else {
            // No params or return type - signature is just `()`
            arrow.span.start
        };

        // Body - expression bodies can break to new line with indent
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                // Check for comments between signature end and body start
                // These are comments like: `() => /* comment */ expr`
                let body_start = expr.span().start;
                let has_leading_comments = self.has_comments_between(sig_end, body_start);

                // Expression body: can break after => with indentation
                // Short: (x) => x + 1
                // Long:  (veryLongParams) =>
                //            veryLongExpr
                // With comment: (x) =>
                //            /* comment */ expr
                if has_leading_comments {
                    // Build body doc with leading comments
                    let body_with_comments =
                        self.build_arrow_body_with_comments_doc(expr, sig_end, body_start);
                    parts.push(doc::group(doc::indent(doc::concat(vec![
                        doc::line(),
                        body_with_comments,
                    ]))));
                } else {
                    let body_doc = self.build_arrow_body_doc(expr);
                    parts.push(doc::group(doc::indent(doc::concat(vec![
                        doc::line(),
                        body_doc,
                    ]))));
                }
            }
            internal::ArrowFunctionBody::BlockStatement(block) => {
                // Block body: always stays hugged to => (no break)
                // (params) => {
                //     ...
                // }
                // Check for comments between signature end and body start
                let body_start = block.span.start;
                let has_leading_comments = self.has_comments_between(sig_end, body_start);

                if has_leading_comments {
                    // Build comments doc
                    let mut comment_parts = Vec::new();
                    for comment in tsv_lang::comments_in_range(self.comments, sig_end, body_start) {
                        comment_parts.push(doc::text(" "));
                        comment_parts.push(self.build_comment_doc(comment));
                    }
                    parts.push(doc::concat(comment_parts));
                }

                parts.push(doc::text(" "));
                parts.push(self.build_block_statement_doc(block));
            }
        }

        doc::concat(parts)
    }

    /// Build doc for return type annotation in arrow function context
    /// Union return types get special handling when the signature breaks:
    ///
    /// Flat: (): A | B | C =>
    /// Break: ):
    ///            | A
    ///            | B
    ///            | C =>
    fn build_arrow_return_type_doc(&self, annotation: &internal::TSTypeAnnotation) -> Doc {
        match &*annotation.type_annotation {
            internal::TSType::Union(union) if union.types.len() > 1 => {
                // Union return types: break after colon, each member on own line with leading |
                let mut union_parts = Vec::new();
                for (i, t) in union.types.iter().enumerate() {
                    if i > 0 {
                        union_parts.push(doc::line());
                    }
                    union_parts.push(doc::text("| "));
                    union_parts.push(self.build_type_doc(t));
                }

                doc::concat(vec![
                    doc::text(":"),
                    doc::if_break(
                        // When breaking: colon, then indented union with leading pipes
                        doc::indent_line(doc::concat(union_parts)),
                        // When flat: normal inline format
                        doc::concat(vec![
                            doc::text(" "),
                            self.build_type_doc(&annotation.type_annotation),
                        ]),
                    ),
                ])
            }
            _ => self.build_type_annotation_doc(annotation),
        }
    }

    /// Build doc for type params NOT in their own group (when no function params)
    /// Softlines break with the outer signature group
    fn build_type_params_doc_for_arrow_ungrouped(
        &self,
        decl: &internal::TSTypeParameterDeclaration,
    ) -> Doc {
        if decl.params.is_empty() {
            return doc::text("<>");
        }

        let param_docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_type_parameter_doc(param))
            .collect();

        // Svelte disambiguation: single param without constraint needs trailing comma
        // in Svelte files to avoid confusion with template syntax like `<Component>`.
        // In pure .ts files (arrow_type_param_trailing_comma=false), no trailing comma needed.
        let needs_trailing_comma = self.config.arrow_type_param_trailing_comma
            && decl.params.len() == 1
            && decl.params[0].constraint.is_none();
        let inner_parts = if needs_trailing_comma {
            doc::concat(vec![doc::join(param_docs, ", "), doc::text(",")])
        } else {
            doc::join_trailing(param_docs, doc::comma_line())
        };

        // No group wrapper - softlines break with outer signature group
        doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(inner_parts),
            doc::softline(),
            doc::text(">"),
        ])
    }

    /// Build doc for type params wrapped in their own group (for multiple type params)
    fn build_type_params_doc_for_arrow_grouped(
        &self,
        decl: &internal::TSTypeParameterDeclaration,
    ) -> Doc {
        if decl.params.is_empty() {
            return doc::text("<>");
        }

        let param_docs: Vec<_> = decl
            .params
            .iter()
            .map(|param| self.build_type_parameter_doc(param))
            .collect();

        // Svelte disambiguation: single param without constraint needs trailing comma
        // in Svelte files to avoid confusion with template syntax like `<Component>`.
        // In pure .ts files (arrow_type_param_trailing_comma=false), no trailing comma needed.
        let needs_trailing_comma = self.config.arrow_type_param_trailing_comma
            && decl.params.len() == 1
            && decl.params[0].constraint.is_none();
        let inner_parts = if needs_trailing_comma {
            doc::concat(vec![doc::join(param_docs, ", "), doc::text(",")])
        } else {
            doc::join_trailing(param_docs, doc::comma_line())
        };

        // Wrap in own group so multiple type params can break independently
        doc::group(doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(inner_parts),
            doc::softline(),
            doc::text(">"),
        ]))
    }

    /// Build doc for params NOT in their own group (outer signature group controls breaking)
    ///
    /// `params_start` is the position of the opening paren (if parenthesized), used for
    /// accurate comment detection. For arrows without parens (`x => x`), this is `None`.
    ///
    /// Structure matches prettier's function-parameters.js:
    /// `[typeParams, "(", indent([softline, ...params]), ifBreak(","), softline, ")"]`
    fn build_arrow_params_doc_ungrouped(
        &self,
        params: &[internal::Expression],
        params_start: Option<u32>,
    ) -> Doc {
        if params.is_empty() {
            return doc::text("()");
        }

        let mut inner_parts = Vec::new();
        for (i, param) in params.iter().enumerate() {
            let param_start = param.span().start;

            // Check for leading comments before this param
            let search_start = if i == 0 {
                // First param: search from after '(' (position + 1)
                params_start.map_or(param_start, |pos| pos + 1)
            } else {
                // Subsequent params: search from after the previous param
                params[i - 1].span().end
            };

            // Add separator before non-first params
            if i > 0 {
                inner_parts.push(doc::text(","));
                inner_parts.push(doc::line());
            }

            // Add leading comments for this param
            inner_parts.push(self.build_comments_between(
                search_start,
                param_start,
                CommentSpacing::Trailing,
            ));

            // Use FunctionParameter context for object patterns
            inner_parts.push(self.build_function_parameter_doc(param));
        }

        // Check if last param is a rest parameter - no trailing comma after rest
        let has_rest_param = params
            .last()
            .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));

        // No group - outer signature group controls breaking
        let mut result = vec![
            doc::text("("),
            doc::indent_softline(doc::concat(inner_parts)),
        ];

        // Trailing comma when broken, unless there's a rest param
        if !has_rest_param {
            result.push(doc::trailing_comma());
        }

        result.push(doc::softline());
        result.push(doc::text(")"));

        doc::concat(result)
    }

    /// Build doc for arrow function body expression.
    fn build_arrow_body_doc(&self, expr: &internal::Expression) -> Doc {
        // Special case: type assertion wrapping object - parens go around inner object only
        // `() => ({}) as T` not `() => (({}) as T)`
        if let Some(assertion) = get_type_assertion_with_object(expr) {
            return match assertion {
                TypeAssertionWithObject::As(as_expr) => doc::concat(vec![
                    doc::text("("),
                    self.build_expression_doc(&as_expr.expression),
                    doc::text(") as "),
                    self.build_type_doc(&as_expr.type_annotation),
                ]),
                TypeAssertionWithObject::Satisfies(sat_expr) => doc::concat(vec![
                    doc::text("("),
                    self.build_expression_doc(&sat_expr.expression),
                    doc::text(") satisfies "),
                    self.build_type_doc(&sat_expr.type_annotation),
                ]),
            };
        }

        // Standard cases: objects and assignments need parens
        if needs_parens(expr, ParenContext::ArrowBody) {
            doc::concat(vec![
                doc::text("("),
                self.build_expression_doc(expr),
                doc::text(")"),
            ])
        } else {
            self.build_expression_doc(expr)
        }
    }

    /// Build doc for arrow function body with leading comments.
    ///
    /// Handles comments between `=>` and the body expression:
    /// ```typescript
    /// () => /* comment */ expr
    /// // becomes:
    /// () =>
    ///     /* comment */ expr
    /// ```
    fn build_arrow_body_with_comments_doc(
        &self,
        expr: &internal::Expression,
        sig_end: u32,
        body_start: u32,
    ) -> Doc {
        let mut parts = Vec::new();

        // Print leading comments
        let comments: Vec<_> =
            tsv_lang::comments_in_range(self.comments, sig_end, body_start).collect();
        for (i, comment) in comments.iter().enumerate() {
            parts.push(self.build_comment_doc(comment));

            // Determine separator after comment:
            // - Multi-line block comments (with newline before */) → hardline
            // - Single-line block or line comments → space
            // - Last comment followed by more → space
            let is_multi_line_block = comment.is_block && comment.content.ends_with('\n');

            if is_multi_line_block && i == comments.len() - 1 {
                // Multi-line block comment as last comment before body → put body on new line
                parts.push(doc::hardline());
            } else {
                // Single-line comment or not last → space
                parts.push(doc::text(" "));
            }
        }

        // Add the body expression
        parts.push(self.build_arrow_body_doc(expr));

        doc::concat(parts)
    }

    /// Build a Doc for an arrow function (simple, non-wrapping version for nested contexts)
    pub(super) fn build_arrow_doc(&self, arrow: &internal::ArrowFunctionExpression) -> Doc {
        // For nested contexts where we don't want independent wrapping decisions,
        // use the wrapping version which will be evaluated in context
        self.build_arrow_doc_wrapping(arrow)
    }

    /// Print a function expression body (params, return type, body): `() { return 1; }`
    ///
    /// Used for method shorthand in objects where the key is printed separately.
    /// For standalone function expressions, use `print_standalone_function_expression` instead.
    ///
    /// Uses hybrid approach: doc-based for signature wrapping, imperative for body (preserves comments).
    pub(in crate::printer) fn print_function_expression_body(
        &mut self,
        func: &internal::FunctionExpression,
    ) {
        // Build and print signature (params + return type) using doc-based wrapping
        let sig_doc = self.build_function_expression_signature_doc(func);
        self.write_doc(&sig_doc);

        // Find the end of the signature to check for dangling comments
        // (comments between signature and body that should move inside)
        let sig_end = if let Some(return_type) = &func.return_type {
            return_type.span.end
        } else if let Some(last_param) = func.params.last() {
            // After last param end (comments after ) will be caught)
            last_param.span().end
        } else {
            // Empty params like fn() - use function span start
            // This catches comments after () like: fn() // comment { }
            func.span.start
        };

        // Print body imperatively (preserves comments)
        self.write(" ");
        self.print_block_statement_with_outer_comments(&func.body, sig_end);
    }

    /// Print a standalone function expression: `function() {}` or `function name() {}`
    ///
    /// This prints the full function expression including:
    /// - `async` keyword if present
    /// - `function` keyword
    /// - `*` for generators
    /// - optional name
    /// - type parameters
    /// - parameters and return type
    /// - body
    pub(in crate::printer) fn print_function_expression(
        &mut self,
        func: &internal::FunctionExpression,
    ) {
        // Print async keyword if present
        if func.r#async {
            self.write("async ");
        }

        // Print 'function' keyword
        self.write("function");

        // Print '*' for generators
        if func.generator {
            self.write("*");
        }

        // Print optional function name
        if let Some(id) = &func.id {
            self.write(" ");
            self.print_identifier(id);
        }

        // Print type parameters (TypeScript generics)
        // Add space before type params if no name: `function <T>` not `function<T>`
        if let Some(type_params) = &func.type_parameters {
            if func.id.is_none() {
                self.write(" ");
            }
            self.print_type_parameter_declaration(type_params);
        }

        // Add space before params if no name or type params: `function ()`
        if func.id.is_none() && func.type_parameters.is_none() {
            self.write(" ");
        }

        // Print the rest (params, return type, body)
        self.print_function_expression_body(func);
    }

    /// Build a Doc for just the function expression signature (params + return type).
    /// Body is printed separately via imperative printer to preserve comments.
    fn build_function_expression_signature_doc(&self, func: &internal::FunctionExpression) -> Doc {
        let mut sig_parts = Vec::new();

        // Function parameters - NOT in their own group, just softlines
        sig_parts.push(self.build_method_params_doc_ungrouped(&func.params));

        // Return type annotation (e.g., `: number`)
        if let Some(return_type) = &func.return_type {
            sig_parts.push(self.build_type_annotation_doc(return_type));
        }

        // Wrap signature in a group for width-aware breaking
        doc::group(doc::concat(sig_parts))
    }

    /// Build a Doc for function expression body (params, return type, body).
    ///
    /// Used for method shorthand in objects where the key is printed separately.
    /// For standalone function expressions, use `build_function_doc` instead.
    pub(in crate::printer) fn build_function_doc_body(
        &self,
        func: &internal::FunctionExpression,
    ) -> Doc {
        // Build signature parts (will be wrapped in a group)
        let mut sig_parts = Vec::new();

        // Function parameters - NOT in their own group, just softlines
        sig_parts.push(self.build_method_params_doc_ungrouped(&func.params));

        // Return type annotation (e.g., `: number`)
        if let Some(return_type) = &func.return_type {
            sig_parts.push(self.build_type_annotation_doc(return_type));
        }

        // Wrap signature in a group for width-aware breaking
        let sig_doc = doc::group(doc::concat(sig_parts));

        // Body - always on same line as signature close
        doc::concat(vec![
            sig_doc,
            doc::text(" "),
            self.build_block_statement_doc(&func.body),
        ])
    }

    /// Build a Doc for a standalone function expression with width-aware wrapping.
    ///
    /// This includes:
    /// - `async` keyword if present
    /// - `function` keyword
    /// - `*` for generators
    /// - optional name
    /// - type parameters
    /// - parameters and return type
    /// - body
    pub(in crate::printer) fn build_function_doc(
        &self,
        func: &internal::FunctionExpression,
    ) -> Doc {
        let mut parts = Vec::new();

        // Async keyword if present
        if func.r#async {
            parts.push(doc::text("async "));
        }

        // Function keyword
        parts.push(doc::text("function"));

        // Generator asterisk
        if func.generator {
            parts.push(doc::text("*"));
        }

        // Optional function name
        if let Some(id) = &func.id {
            parts.push(doc::text(" "));
            parts.push(self.build_identifier_doc(id));
        }

        // Type parameters (TypeScript generics)
        // Always add space before type params if no name: `function <T>` not `function<T>`
        if let Some(type_params) = &func.type_parameters {
            if func.id.is_none() {
                parts.push(doc::text(" "));
            }
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Space before params if no name or type params: `function ()`
        if func.id.is_none() && func.type_parameters.is_none() {
            parts.push(doc::text(" "));
        }

        // Params, return type, and body
        parts.push(self.build_function_doc_body(func));

        doc::concat(parts)
    }

    /// Build doc for method params NOT in their own group
    fn build_method_params_doc_ungrouped(&self, params: &[internal::Expression]) -> Doc {
        if params.is_empty() {
            return doc::text("()");
        }

        let mut inner_parts = Vec::new();
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                inner_parts.push(doc::text(","));
                inner_parts.push(doc::line());
            }
            inner_parts.push(self.build_expression_doc(param));
        }

        let has_rest_param = params
            .last()
            .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));

        let mut result = vec![
            doc::text("("),
            doc::indent_softline(doc::concat(inner_parts)),
        ];

        if !has_rest_param {
            result.push(doc::trailing_comma());
        }

        result.push(doc::softline());
        result.push(doc::text(")"));

        doc::concat(result)
    }

    /// Print a class expression: `class { }` or `class Foo<T> extends Bar { }`
    pub(in crate::printer) fn print_class_expression(
        &mut self,
        class_expr: &internal::ClassExpression,
    ) {
        self.write("class");

        // Print optional class name
        if let Some(id) = &class_expr.id {
            self.write(" ");
            self.print_identifier(id);
        }

        // Print type parameters (TypeScript generics): class<T>
        if let Some(type_params) = &class_expr.type_parameters {
            self.print_type_parameter_declaration(type_params);
        }

        // Print optional extends clause
        if let Some(super_class) = &class_expr.super_class {
            self.write(" extends ");
            self.print_expression(super_class);
            // Print type arguments: extends Base<T>
            if let Some(super_type_params) = &class_expr.super_type_parameters {
                self.print_type_parameter_instantiation(super_type_params);
            }
        }

        // Print optional implements clause - extract from source for simplicity
        if !class_expr.implements.is_empty() {
            self.write(" implements ");
            // Extract implements list from source (between extends and body or between class header and body)
            let first_impl = &class_expr.implements[0];
            let last_impl = &class_expr.implements[class_expr.implements.len() - 1];
            let impl_start = first_impl.span.start_usize();
            let impl_end = last_impl.span.end_usize();
            let impl_str = &self.source[impl_start..impl_end];
            self.write(impl_str);
        }

        self.write(" ");
        // Use the shared print_class_body method
        self.print_class_body(&class_expr.body, false);
    }

    /// Build a Doc for a class expression
    pub(in crate::printer) fn build_class_expression_doc(
        &self,
        class_expr: &internal::ClassExpression,
    ) -> Doc {
        let mut parts = Vec::new();

        // 'class' keyword
        parts.push(doc::text("class"));

        // Optional class name
        if let Some(id) = &class_expr.id {
            parts.push(doc::text(" "));
            parts.push(self.build_identifier_doc(id));
        }

        // Type parameters (TypeScript generics): class<T>
        if let Some(type_params) = &class_expr.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc(type_params));
        }

        // Optional extends clause
        if let Some(super_class) = &class_expr.super_class {
            parts.push(doc::text(" extends "));
            parts.push(self.build_expression_doc(super_class));
            // Type arguments: extends Base<T>
            if let Some(super_type_params) = &class_expr.super_type_parameters {
                parts.push(self.build_type_parameter_instantiation_doc(super_type_params));
            }
        }

        // Implements clause (extract from source for simplicity)
        if !class_expr.implements.is_empty() {
            parts.push(doc::text(" implements "));
            let first_impl = &class_expr.implements[0];
            let last_impl = &class_expr.implements[class_expr.implements.len() - 1];
            let impl_start = first_impl.span.start_usize();
            let impl_end = last_impl.span.end_usize();
            let impl_str = &self.source[impl_start..impl_end];
            parts.push(doc::text_owned(impl_str.to_string()));
        }

        // Space before body
        parts.push(doc::text(" "));

        // Class body
        parts.push(self.build_class_body_doc(&class_expr.body, false));

        doc::concat(parts)
    }
}
