// Function expression printing for TypeScript
//
// This module handles function-related expressions:
// - Arrow function expressions (async, parameters, return types, body)
// - Function expressions (parameters, return types, body)
//
// Note: Block statements are in blocks.rs as a reusable utility

use super::super::{ParenContext, Printer, needs_parens};
use crate::ast::internal;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::is_same_line;

/// Check if an expression is directly an object literal (needs parentheses in arrow body)
/// Only returns true for direct ObjectExpression - TSAsExpression etc. are handled separately
fn is_object_expression(expr: &internal::Expression) -> bool {
    matches!(expr, internal::Expression::ObjectExpression(_))
}

/// Check if an arrow body should stay on the same line as `=>` (no line break option).
///
/// Prettier's `mayBreakAfterShortPrefix` - these expression types stay hugged to `=>`:
/// - Object literals: `() => ({...})`
/// - Array literals: `() => [...]`
/// - Arrow functions: `() => () => ...`
/// - Block statements (handled separately)
/// - JSX elements (not yet supported)
/// - Template literals on own line (not yet implemented)
///
/// When true, body uses `" " + body` (simple space).
/// When false, body uses `indent([line, body])` (can break to new line).
fn should_hug_arrow_body(expr: &internal::Expression) -> bool {
    matches!(
        expr,
        internal::Expression::ObjectExpression(_)
            | internal::Expression::ArrayExpression(_)
            | internal::Expression::ArrowFunctionExpression(_)
    )
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

        // Type parameters: group them independently only when function params exist
        if let Some(tp) = &arrow.type_parameters {
            sig_parts.push(self.build_type_params_doc_for_arrow(tp, has_params));
        }

        // Function parameters - NOT in their own group, just softlines
        // These will break when the outer signature group breaks
        sig_parts.push(self.build_arrow_params_doc_ungrouped(arrow));

        // Return type annotation - union types need special handling for breaking
        if let Some(return_type) = &arrow.return_type {
            sig_parts.push(self.build_arrow_return_type_doc(return_type));
        }

        // Calculate signature end position (after `)` or return type)
        // This is where comments BEFORE `=>` start
        let sig_end = if let Some(rt) = &arrow.return_type {
            rt.span.end
        } else if let Some(params_start) = arrow.params_start {
            // Find closing `)` to get accurate boundary
            self.find_closing_paren(params_start, arrow.body.span().start)
                .unwrap_or_else(|| arrow.body.span().start)
        } else {
            // No parens (single param arrow like `x => x`) - use param end
            arrow
                .params
                .last()
                .map_or(arrow.span.start, |p| p.span().end)
        };

        // Find the `=>` token position to distinguish:
        // - Comments between sig_end and `=>` → print BEFORE `=>`
        // - Comments between `=>` and body → print AFTER `=>`
        let arrow_pos = self
            .find_arrow_token(sig_end, arrow.body.span().start)
            .unwrap_or_else(|| arrow.body.span().start);
        let arrow_end = arrow_pos + 2; // Position after `=>`

        // Check for comments between signature and `=>` (e.g., `(x) /* c */ =>`)
        let has_pre_arrow_comments = self.has_comments_between(sig_end, arrow_pos);

        if has_pre_arrow_comments {
            // Print comments before `=>`
            for comment in tsv_lang::comments_in_range(self.comments, sig_end, arrow_pos) {
                sig_parts.push(doc::text(" "));
                sig_parts.push(self.build_comment_doc(comment));
            }
        }

        // Include " =>" in the signature group for width calculation
        sig_parts.push(doc::text(" =>"));

        // Wrap entire signature in a group
        parts.push(doc::group(doc::concat(sig_parts)));

        // Body - expression bodies can break to new line with indent
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                // Check for comments between `=>` and body start
                // These are comments like: `() => /* comment */ expr`
                let body_start = expr.span().start;
                let has_post_arrow_comments = self.has_comments_between(arrow_end, body_start);

                // Prettier's `shouldPutBodyOnSameLine`: certain expression types stay hugged to =>
                // Object/array literals and nested arrows don't break after =>
                let should_hug = !has_post_arrow_comments && should_hug_arrow_body(expr);

                if has_post_arrow_comments {
                    // Build body doc with leading comments - always breaks
                    let body_with_comments =
                        self.build_arrow_body_with_comments_doc(expr, arrow_end, body_start);
                    parts.push(doc::group(doc::indent(doc::concat(vec![
                        doc::line(),
                        body_with_comments,
                    ]))));
                } else if should_hug {
                    // Hugged body: simple space, no line break option
                    // `() => ({...})` stays on same line regardless of object's internal breaks
                    let body_doc = self.build_arrow_body_doc(expr);
                    parts.push(doc::text(" "));
                    parts.push(body_doc);
                } else {
                    // Normal expression: can break after => with indentation
                    // Short: (x) => x + 1
                    // Long:  (veryLongParams) =>
                    //            veryLongExpr
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
                // Check for comments between `=>` and body start
                let body_start = block.span.start;
                let has_post_arrow_comments = self.has_comments_between(arrow_end, body_start);

                if has_post_arrow_comments {
                    // Build comments doc
                    let mut comment_parts = Vec::new();
                    for comment in tsv_lang::comments_in_range(self.comments, arrow_end, body_start)
                    {
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

    /// Build doc for arrow function type params
    ///
    /// When `grouped` is true, wraps in its own group so type params can break independently.
    /// When false, softlines break with the outer signature group.
    fn build_type_params_doc_for_arrow(
        &self,
        decl: &internal::TSTypeParameterDeclaration,
        grouped: bool,
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

        let brackets_doc = doc::concat(vec![
            doc::text("<"),
            doc::indent_softline(inner_parts),
            doc::softline(),
            doc::text(">"),
        ]);

        if grouped {
            doc::group(brackets_doc)
        } else {
            brackets_doc
        }
    }

    /// Build doc for arrow params NOT in their own group (outer signature group controls breaking)
    ///
    /// Structure matches prettier's function-parameters.js:
    /// `[typeParams, "(", indent([softline, ...params]), ifBreak(","), softline, ")"]`
    fn build_arrow_params_doc_ungrouped(&self, arrow: &internal::ArrowFunctionExpression) -> Doc {
        let params_start = arrow.params_start;

        // Compute trailing comments boundary for params
        // IMPORTANT: Stop at `)` not at return type or body start
        // Comments between `)` and `=>` are handled separately by the arrow printer
        let trailing_comments_end = if let Some(ps) = params_start {
            // Find the closing `)` position
            let body_start = arrow.body.span().start;
            self.find_closing_paren(ps, body_start)
        } else {
            // No parens - use param end as boundary
            arrow.params.last().map(|p| p.span().end)
        };

        // Delegate to shared implementation
        self.build_params_doc_with_comments(&arrow.params, params_start, trailing_comments_end)
    }

    /// Check if any param has a trailing line comment
    fn has_trailing_line_comment_in_params(
        &self,
        params: &[internal::Expression],
        trailing_comments_end: Option<u32>,
    ) -> bool {
        params.iter().enumerate().any(|(i, param)| {
            let trailing_end = self.param_trailing_end(params, i, trailing_comments_end);
            self.has_line_comments_between(param.span().end, trailing_end)
        })
    }

    /// Get the end position for trailing comments after a parameter
    fn param_trailing_end(
        &self,
        params: &[internal::Expression],
        index: usize,
        trailing_comments_end: Option<u32>,
    ) -> u32 {
        if index + 1 < params.len() {
            params[index + 1].span().start
        } else {
            trailing_comments_end.unwrap_or_else(|| params[index].span().end)
        }
    }

    /// Build doc for trailing same-line comments after a parameter
    fn build_trailing_param_comments(&self, start: u32, end: u32) -> Doc {
        let mut parts = Vec::new();

        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            // Only include comments on the same line as the param (trailing comments)
            if is_same_line(self.source, start, comment.span.start) {
                parts.push(doc::text(" "));
                parts.push(self.build_comment_doc(comment));
            }
        }

        doc::concat(parts)
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
        // We need the position AFTER the closing `)` to avoid re-printing
        // comments that were already handled by the params printer.
        let sig_end = if let Some(return_type) = &func.return_type {
            return_type.span.end
        } else {
            // No return type - find the closing `)` by scanning from params_start
            // to body start. Comments between `)` and `{` should move into body.
            self.find_closing_paren(func.params_start, func.body.span.start)
                .unwrap_or(func.body.span.start)
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

    /// Build a Doc for just the function expression signature (type params, params, return type).
    /// Body is printed separately via imperative printer to preserve comments.
    fn build_function_expression_signature_doc(&self, func: &internal::FunctionExpression) -> Doc {
        let mut sig_parts = Vec::new();

        // Type parameters (TypeScript generics): <T, U>
        // Use _wrapping version for width-based line breaking
        if let Some(type_params) = &func.type_parameters {
            sig_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Function parameters - NOT in their own group, just softlines
        sig_parts.push(self.build_method_params_doc_ungrouped(func));

        // Return type annotation (e.g., `: number`)
        if let Some(return_type) = &func.return_type {
            sig_parts.push(self.build_type_annotation_doc(return_type));
        }

        // Wrap signature in a group for width-aware breaking
        doc::group(doc::concat(sig_parts))
    }

    /// Build a Doc for function expression body (type params, params, return type, body).
    ///
    /// Used for method shorthand in objects where the key is printed separately.
    /// For standalone function expressions, use `build_function_doc` instead.
    pub(in crate::printer) fn build_function_doc_body(
        &self,
        func: &internal::FunctionExpression,
    ) -> Doc {
        let sig_doc = self.build_function_expression_signature_doc(func);

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

        // Space before type params or params if no name: `function <T>` or `function ()`
        if func.id.is_none() {
            parts.push(doc::text(" "));
        }

        // Type params, params, return type, and body (signature_doc handles type params)
        parts.push(self.build_function_doc_body(func));

        doc::concat(parts)
    }

    /// Build doc for function/method params NOT in their own group
    ///
    /// `params_start` is the position of the opening paren (for comment detection).
    /// `trailing_comments_end` is where trailing comments after the last param end
    /// (typically return type start or body start).
    pub(in crate::printer) fn build_method_params_doc_ungrouped(
        &self,
        func: &internal::FunctionExpression,
    ) -> Doc {
        let params = &func.params;
        let params_start = Some(func.params_start);

        // Compute trailing comments boundary
        let trailing_comments_end = if let Some(rt) = &func.return_type {
            Some(rt.span.start)
        } else {
            Some(func.body.span.start)
        };

        // Delegate to shared implementation
        self.build_params_doc_with_comments(params, params_start, trailing_comments_end)
    }

    /// Shared implementation for building params doc with comment handling
    ///
    /// Used by arrow functions, function expressions, function declarations, and class methods.
    pub(in crate::printer) fn build_params_doc_with_comments(
        &self,
        params: &[internal::Expression],
        params_start: Option<u32>,
        trailing_comments_end: Option<u32>,
    ) -> Doc {
        self.build_params_doc_with_comments_ext(params, params_start, trailing_comments_end, false)
    }

    /// Extended version with external force_break flag
    ///
    /// `force_break_external` allows callers to force multiline based on width heuristics.
    pub(in crate::printer) fn build_params_doc_with_comments_ext(
        &self,
        params: &[internal::Expression],
        params_start: Option<u32>,
        trailing_comments_end: Option<u32>,
        force_break_external: bool,
    ) -> Doc {
        if params.is_empty() {
            return doc::text("()");
        }

        // Check if any trailing line comments exist on params
        // If so, we must use hardlines to force the group to break
        let has_trailing_line_comment =
            self.has_trailing_line_comment_in_params(params, trailing_comments_end);

        // Check if any leading line comments exist on their own line before params
        // Line comments on their own line also force break
        let has_leading_own_line_comment =
            self.has_leading_own_line_comment_in_params(params, params_start);

        // Prettier rule: force break when 2+ params and at least one is TSParameterProperty
        // (has access modifiers like private/public/protected/readonly)
        let should_break_for_param_properties = params.len() > 1
            && params
                .iter()
                .any(|p| matches!(p, internal::Expression::TSParameterProperty(_)));

        // Combined condition for forcing multiline (includes external width-based force)
        let force_break = force_break_external
            || has_trailing_line_comment
            || has_leading_own_line_comment
            || should_break_for_param_properties;

        // Check if last param is a rest parameter - no trailing comma after rest
        let has_rest_param = params
            .last()
            .is_some_and(|p| matches!(p, internal::Expression::RestElement(_)));

        let mut inner_parts = Vec::new();
        for (i, param) in params.iter().enumerate() {
            let param_start = param.span().start;
            let is_last = i == params.len() - 1;

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
                // Use hardline when forcing break (trailing line comments or param properties)
                if force_break {
                    inner_parts.push(doc::hardline());
                } else {
                    inner_parts.push(doc::line());
                }
            }

            // Add leading comments for this param
            // Use proper line breaks for line comments on their own line
            inner_parts.push(self.build_leading_param_comments(search_start, param_start));

            // Use FunctionParameter context for object patterns
            inner_parts.push(self.build_function_parameter_doc(param));

            // Add trailing comma BEFORE trailing comments
            // For non-last params: always add comma (it's a separator)
            // For last param: add comma if forcing break and not rest param
            let needs_comma = !is_last || (force_break && !has_rest_param);
            if needs_comma {
                inner_parts.push(doc::text(","));
            }

            // Add trailing comments ONLY for the last param
            // For non-last params, comments after the comma are handled as leading
            // comments for the next param (via build_comments_between above)
            if is_last {
                let trailing_end = self.param_trailing_end(params, i, trailing_comments_end);
                inner_parts
                    .push(self.build_trailing_param_comments(param.span().end, trailing_end));
            }
        }

        // No group - outer signature group controls breaking
        let mut result = vec![doc::text("(")];

        if force_break {
            // When forcing break (trailing comments or param properties), use hardlines
            result.push(doc::indent(doc::concat(vec![
                doc::hardline(),
                doc::concat(inner_parts),
            ])));
            result.push(doc::hardline());
        } else {
            result.push(doc::indent_softline(doc::concat(inner_parts)));
            // Trailing comma when broken, unless there's a rest param
            if !has_rest_param {
                result.push(doc::trailing_comma());
            }
            result.push(doc::softline());
        }

        result.push(doc::text(")"));

        doc::concat(result)
    }

    /// Check if any param has a leading line comment on its own line
    fn has_leading_own_line_comment_in_params(
        &self,
        params: &[internal::Expression],
        params_start: Option<u32>,
    ) -> bool {
        for (i, param) in params.iter().enumerate() {
            let search_start = if i == 0 {
                params_start.map_or_else(|| param.span().start, |pos| pos + 1)
            } else {
                params[i - 1].span().end
            };

            // Check if there's a line comment on its own line before this param
            if self.has_own_line_comment_between(search_start, param.span().start) {
                return true;
            }
        }
        false
    }

    /// Check if there's a line comment on its own line between two positions
    fn has_own_line_comment_between(&self, start: u32, end: u32) -> bool {
        for comment in tsv_lang::comments_in_range(self.comments, start, end) {
            // Line comments are always on their own line (they extend to EOL)
            // Block comments on their own line have a newline before them
            if !comment.is_block {
                return true;
            }
            // Check if block comment is on its own line (newline before it)
            if !is_same_line(self.source, start, comment.span.start) {
                return true;
            }
        }
        false
    }

    /// Build doc for leading comments before a parameter
    /// Handles line comments on their own line with proper hardlines
    fn build_leading_param_comments(&self, start: u32, end: u32) -> Doc {
        let comments: Vec<_> = tsv_lang::comments_in_range(self.comments, start, end).collect();
        if comments.is_empty() {
            return doc::concat(vec![]);
        }

        let mut parts = Vec::new();

        for (i, comment) in comments.iter().enumerate() {
            // Check if comment is on its own line (not same line as previous content)
            let prev_pos = if i == 0 {
                start
            } else {
                comments[i - 1].span.end
            };
            let on_own_line = !is_same_line(self.source, prev_pos, comment.span.start);

            if on_own_line && i > 0 {
                // Comment on its own line (not first) - add hardline before it
                parts.push(doc::hardline());
            }
            parts.push(self.build_comment_doc(comment));
        }

        // Check if the param itself is on its own line after the last comment
        let last_comment_end = comments.last().map_or(start, |c| c.span.end);
        let param_on_own_line = !is_same_line(self.source, last_comment_end, end);

        if param_on_own_line {
            parts.push(doc::hardline());
        } else {
            // Inline - add space after comment
            parts.push(doc::text(" "));
        }

        doc::concat(parts)
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
        // Use _wrapping version for width-based line breaking
        if let Some(type_params) = &class_expr.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
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
