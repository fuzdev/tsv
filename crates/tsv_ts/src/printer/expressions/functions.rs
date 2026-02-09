// Function expression printing for TypeScript
//
// This module handles function-related expressions:
// - Arrow function expressions (async, parameters, return types, body)
// - Function expressions (parameters, return types, body)
//
// Note: Block statements are in blocks.rs as a reusable utility

use super::super::utils::arrow_has_trailing_param_comments;
use super::{ParenContext, Printer, needs_parens, unwrap_parenthesized};
use crate::ast::internal;
use crate::printer::types::helpers::is_huggable_type;
use tsv_lang::doc::arena::DocId;

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

/// Check if an expression is a huggable pattern for function parameters.
///
/// Prettier's `shouldHugFunctionParameters` hugs single object/array patterns,
/// keeping `({` and `}: Type)` together while letting the pattern's content break.
fn is_huggable_pattern(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::ObjectPattern(_) | internal::Expression::ArrayPattern(_) => true,
        // Assignment pattern with object/array on left: `{a, b} = default`
        internal::Expression::AssignmentPattern(ap) => {
            matches!(
                ap.left.as_ref(),
                internal::Expression::ObjectPattern(_) | internal::Expression::ArrayPattern(_)
            )
        }
        _ => false,
    }
}

/// Check if an expression has a huggable type annotation.
///
/// Parameters with huggable type annotations like `a?: { b: T }` should be hugged:
/// - The opening `{` stays on the same line as the parameter name
/// - The content expands internally
/// - The closing `}` comes on its own line
///
/// This matches Prettier's behavior where `fn(a?: {` stays together.
fn has_huggable_type_annotation(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::Identifier(id) => id
            .type_annotation
            .as_ref()
            .is_some_and(|ann| is_huggable_type(&ann.type_annotation)),
        internal::Expression::AssignmentPattern(ap) => has_huggable_type_annotation(&ap.left),
        _ => false,
    }
}

impl<'a> Printer<'a> {
    /// Print an arrow function expression using doc-based formatting with width-aware wrapping.
    ///
    /// Prettier behavior for arrow functions:
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
    fn build_arrow_doc_wrapping(&self, arrow: &internal::ArrowFunctionExpression) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Async keyword if present
        if arrow.r#async {
            parts.push(d.text("async "));
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
        // Single binary search via comments_in_range
        for comment in tsv_lang::comments_in_range(self.comments, sig_end, arrow_pos) {
            sig_parts.push(d.text(" "));
            sig_parts.push(self.build_comment_doc(comment));
        }

        // Include " =>" in the signature group for width calculation
        sig_parts.push(d.text(" =>"));

        // Wrap entire signature in a group
        parts.push(d.group(d.concat(&sig_parts)));

        // Body - expression bodies can break to new line with indent
        match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => {
                // Check for comments between `=>` and body start
                // These are comments like: `() => /* comment */ expr`
                let body_start = expr.span().start;
                let has_post_arrow_comments = self.has_comments_between(arrow_end, body_start);

                // Prettier's `shouldPutBodyOnSameLine`: certain expression types stay hugged to =>
                // Object/array literals always hug.
                // Nested arrows hug ONLY when outer has no return type annotation.
                // With return type: const f = (x: T): H => (y) => expr; // breaks
                // Without:          const f = (x: T) => (y) => expr;    // hugs
                let is_arrow_body =
                    matches!(&**expr, internal::Expression::ArrowFunctionExpression(_));

                // Check if this is a curried arrow where ANY arrow triggers chain breaking.
                // Triggers: return type with params, type parameters, non-identifier params
                let chain_has_return_type =
                    is_arrow_body && crate::printer::arrow_chain_has_return_type(arrow);

                // Check if body arrow has trailing param comments (forces break)
                let body_arrow_has_trailing_param_comments =
                    if let internal::Expression::ArrowFunctionExpression(body_arrow) = expr.as_ref()
                    {
                        arrow_has_trailing_param_comments(body_arrow, |start, end| {
                            self.has_comments_between(start, end)
                        })
                    } else {
                        false
                    };

                let should_hug = !has_post_arrow_comments
                    && should_hug_arrow_body(expr)
                    && !chain_has_return_type
                    && !body_arrow_has_trailing_param_comments;

                if has_post_arrow_comments {
                    // Build body doc with leading comments - always breaks
                    let body_with_comments =
                        self.build_arrow_body_with_comments_doc(expr, arrow_end, body_start);
                    parts.push(d.group(d.indent(d.concat(&[d.line(), body_with_comments]))));
                } else if should_hug {
                    // Hugged body: simple space, no line break option
                    // `() => ({...})` stays on same line regardless of object's internal breaks
                    let body_doc = self.build_arrow_body_doc(expr);
                    parts.push(d.text(" "));
                    parts.push(body_doc);
                } else if is_arrow_body
                    && (chain_has_return_type || self.in_curried_typed_arrow.get())
                {
                    // Curried arrow chain - all arrows break without indent so they align:
                    // const f = (x: T): H => (y) => expr   // outer has return type
                    // const f = (x: T) => (y): H => expr   // inner has return type
                    // becomes:
                    // const f =
                    //     (x: T): H =>      or      (x: T) =>
                    //     (y) =>                    (y): H =>
                    //         expr                      expr
                    //
                    // Set context flag when entering chain, restore when done.
                    let was_in_curried = self.in_curried_typed_arrow.get();
                    if chain_has_return_type {
                        self.in_curried_typed_arrow.set(true);
                    }
                    let body_doc = self.build_arrow_body_doc(expr);
                    self.in_curried_typed_arrow.set(was_in_curried);
                    parts.push(d.concat(&[d.hardline(), body_doc]));
                } else if is_arrow_body && body_arrow_has_trailing_param_comments {
                    // Nested arrow with trailing param comments - first level gets indent,
                    // subsequent levels align (use curried pattern)
                    // (a, // c) => (b, // c) => {}
                    // becomes:
                    // (a, // c) =>
                    //     (b, // c) =>
                    //     (c, // c) => {}
                    let was_in_curried = self.in_curried_typed_arrow.get();
                    self.in_curried_typed_arrow.set(true);
                    let body_doc = self.build_arrow_body_doc(expr);
                    self.in_curried_typed_arrow.set(was_in_curried);
                    parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
                } else if self.in_curried_typed_arrow.get() {
                    // Innermost arrow in curried chain - body is NOT another arrow.
                    // This needs indent since it's the final expression.
                    let body_doc = self.build_arrow_body_doc(expr);
                    parts.push(d.indent(d.concat(&[d.hardline(), body_doc])));
                } else {
                    // Normal expression: can break after => with indentation
                    // Short: (x) => x + 1
                    // Long:  (veryLongParams) =>
                    //            veryLongExpr
                    //
                    // The body is wrapped in a group so it can make its own fits() decision.
                    // This allows the arrow body to stay inline even when the parent element
                    // is in break mode, as long as the body content fits from its position.
                    //
                    // For template literal bodies, wrap in IsolatedGroup to prevent the
                    // template's internal ${} breaks from forcing the arrow body to break.
                    // This enables `.map((x) => \`${...}\`)` to stay hugged.
                    let body_doc = self.build_arrow_body_doc(expr);
                    let body_doc = if matches!(
                        expr.as_ref(),
                        internal::Expression::TemplateLiteral(_)
                            | internal::Expression::TaggedTemplateExpression(_)
                    ) {
                        d.isolated_group(body_doc)
                    } else {
                        body_doc
                    };
                    parts.push(d.group(d.indent(d.concat(&[d.line(), body_doc]))));
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
                        comment_parts.push(d.text(" "));
                        comment_parts.push(self.build_comment_doc(comment));
                    }
                    parts.push(d.concat(&comment_parts));
                }

                parts.push(d.text(" "));
                parts.push(self.build_block_statement_doc(block));
            }
        }

        d.concat(&parts)
    }

    /// Build doc for return type annotation in arrow function context
    /// Union return types get special handling when the signature breaks:
    ///
    /// Flat: (): A | B | C =>
    /// Break: ):
    ///            | A
    ///            | B
    ///            | C =>
    ///
    /// Function types as return types get wrapped in parentheses for disambiguation:
    /// `(x: T): ((y: T) => U) =>` not `(x: T): (y: T) => U =>`
    fn build_arrow_return_type_doc(&self, annotation: &internal::TSTypeAnnotation) -> DocId {
        let d = self.d();
        // Function types need parentheses to disambiguate from the arrow's `=>`
        // Example: `(x: T): ((y: T) => U) =>` not `(x: T): (y: T) => U =>`
        // Unwrap any explicit parenthesized types to check the inner type
        let inner_type = unwrap_parenthesized(&annotation.type_annotation);
        if matches!(inner_type, internal::TSType::Function(_)) {
            let type_doc = self.build_type_doc(inner_type);
            return d.concat(&[d.text(": ("), type_doc, d.text(")")]);
        }

        // Use return type version - only wraps for complex type args (unions/intersections)
        // Simple cases like Promise<void> let params break first
        self.build_type_annotation_doc_for_return_type(annotation)
    }

    /// Build doc for arrow function type params
    ///
    /// When `grouped` is true, wraps in its own group so type params can break independently.
    /// When false, softlines break with the outer signature group.
    fn build_type_params_doc_for_arrow(
        &self,
        decl: &internal::TSTypeParameterDeclaration,
        grouped: bool,
    ) -> DocId {
        let d = self.d();
        if decl.params.is_empty() {
            return d.text("<>");
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
            d.concat(&[d.join(param_docs, ", "), d.text(",")])
        } else {
            d.join_trailing(param_docs, d.comma_line())
        };

        let brackets_doc = d.concat(&[
            d.text("<"),
            d.indent_softline(inner_parts),
            d.softline(),
            d.text(">"),
        ]);

        if grouped {
            d.group(brackets_doc)
        } else {
            brackets_doc
        }
    }

    /// Build doc for arrow params NOT in their own group (outer signature group controls breaking)
    ///
    /// Structure matches prettier's function-parameters.js:
    /// `[typeParams, "(", indent([softline, ...params]), ifBreak(","), softline, ")"]`
    fn build_arrow_params_doc_ungrouped(&self, arrow: &internal::ArrowFunctionExpression) -> DocId {
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

    /// Build just the arrow function signature (async + type params + params + return type)
    /// WITHOUT the ` =>` and body. Used by call printer for expand-last-arg pattern.
    ///
    /// This is extracted from `build_arrow_doc_wrapping` to support the special case
    /// where call expressions need to build arrows with conditional parens around the body.
    pub(crate) fn build_arrow_signature_doc(
        &self,
        arrow: &internal::ArrowFunctionExpression,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Async keyword if present
        if arrow.r#async {
            parts.push(d.text("async "));
        }

        let has_params = !arrow.params.is_empty();

        // Type parameters: group them independently only when function params exist
        if let Some(tp) = &arrow.type_parameters {
            parts.push(self.build_type_params_doc_for_arrow(tp, has_params));
        }

        // Function parameters
        parts.push(self.build_arrow_params_doc_ungrouped(arrow));

        // Return type annotation
        if let Some(return_type) = &arrow.return_type {
            parts.push(self.build_arrow_return_type_doc(return_type));
        }

        d.concat(&parts)
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

    /// Build doc for arrow function body expression.
    fn build_arrow_body_doc(&self, expr: &internal::Expression) -> DocId {
        let d = self.d();
        // Special case: type assertion wrapping object - parens go around inner object only
        // `() => ({}) as T` not `() => (({}) as T)`
        if let Some(assertion) = get_type_assertion_with_object(expr) {
            return match assertion {
                TypeAssertionWithObject::As(as_expr) => d.concat(&[
                    d.text("("),
                    self.build_expression_doc(&as_expr.expression),
                    d.text(") as "),
                    self.build_type_doc_with_wrapping_type_args(&as_expr.type_annotation),
                ]),
                TypeAssertionWithObject::Satisfies(sat_expr) => d.concat(&[
                    d.text("("),
                    self.build_expression_doc(&sat_expr.expression),
                    d.text(") satisfies "),
                    self.build_type_doc_with_wrapping_type_args(&sat_expr.type_annotation),
                ]),
            };
        }

        // Conditional expressions need parens only when inline:
        // Same line: `() => (a ? b : c)` - parens needed to disambiguate
        // New line:  `() =>\n    a ? b : c` - no parens needed
        //
        // We use two checks:
        // 1. will_break: If body contains hardlines, it WILL break, so no parens needed
        // 2. if_break: For bodies without hardlines, check if enclosing group breaks
        if matches!(expr, internal::Expression::ConditionalExpression(_)) {
            let body_doc = self.build_expression_doc(expr);
            // If body contains hardlines (will definitely break), no parens
            if d.will_break(body_doc) {
                return body_doc;
            }
            // Otherwise, use if_break to check enclosing group
            return d.if_break(body_doc, d.parens(body_doc));
        }

        // Standard cases: objects and assignments always need parens
        if needs_parens(expr, ParenContext::ArrowBody) {
            d.parens(self.build_expression_doc(expr))
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
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Print leading comments
        let comments: Vec<_> =
            tsv_lang::comments_in_range(self.comments, sig_end, body_start).collect();
        for (i, comment) in comments.iter().enumerate() {
            parts.push(self.build_comment_doc(comment));

            // Determine separator after comment:
            // - Line comments (// ...) → MUST use hardline (comment extends to EOL)
            // - Multi-line block comments (with newline before */) → hardline
            // - Single-line block comments → space
            let is_line_comment = !comment.is_block;
            let is_multi_line_block = comment.is_block && comment.content.ends_with('\n');

            if is_line_comment || (is_multi_line_block && i == comments.len() - 1) {
                // Line comment or multi-line block as last → put next content on new line
                parts.push(d.hardline());
            } else {
                // Single-line block comment → space
                parts.push(d.text(" "));
            }
        }

        // Add the body expression
        parts.push(self.build_arrow_body_doc(expr));

        d.concat(&parts)
    }

    /// Build a Doc for an arrow function (simple, non-wrapping version for nested contexts)
    pub(super) fn build_arrow_doc(&self, arrow: &internal::ArrowFunctionExpression) -> DocId {
        // For nested contexts where we don't want independent wrapping decisions,
        // use the wrapping version which will be evaluated in context
        self.build_arrow_doc_wrapping(arrow)
    }

    /// Build a Doc for just the function expression signature (type params, params, return type).
    /// Body is printed separately via imperative printer to preserve comments.
    fn build_function_expression_signature_doc(
        &self,
        func: &internal::FunctionExpression,
    ) -> DocId {
        let d = self.d();
        let mut sig_parts = Vec::new();

        // Type parameters (TypeScript generics): <T, U>
        // Use _wrapping version for width-based line breaking
        if let Some(type_params) = &func.type_parameters {
            sig_parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Function parameters - NOT in their own group, just softlines
        sig_parts.push(self.build_method_params_doc_ungrouped(func));

        // Return type annotation (e.g., `: number`)
        // Use return type version - only wraps for complex type args (unions/intersections)
        if let Some(return_type) = &func.return_type {
            sig_parts.push(self.build_type_annotation_doc_for_return_type(return_type));
        }

        // Wrap signature in a group for width-aware breaking
        d.group(d.concat(&sig_parts))
    }

    /// Build a Doc for function expression body (type params, params, return type, body).
    ///
    /// Used for method shorthand in objects where the key is printed separately.
    /// For standalone function expressions, use `build_function_doc` instead.
    pub(in crate::printer) fn build_function_doc_body(
        &self,
        func: &internal::FunctionExpression,
    ) -> DocId {
        let d = self.d();
        let sig_doc = self.build_function_expression_signature_doc(func);

        // Find signature end for outer comment detection
        let sig_end = if let Some(rt) = &func.return_type {
            rt.span.end
        } else if let Some(paren) = self.find_closing_paren(func.params_start, func.body.span.start)
        {
            paren
        } else {
            func.body.span.start
        };

        // Check for comments between signature and body (outer comments)
        let outer_comments = self.build_outer_comments_for_block(sig_end, &func.body);

        // Body - always on same line as signature close
        d.concat(&[
            sig_doc,
            d.text(" "),
            self.build_block_statement_with_outer_comments_doc(&func.body, outer_comments),
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
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // Async keyword if present
        if func.r#async {
            parts.push(d.text("async "));
        }

        // Function keyword
        parts.push(d.text("function"));

        // Generator asterisk
        if func.generator {
            parts.push(d.text("*"));
        }

        // Optional function name
        if let Some(id) = &func.id {
            parts.push(d.text(" "));
            // Comments between keywords and the name (same as FunctionDeclaration)
            parts.push(
                self.build_inline_comments_between_doc_trailing_space(
                    func.span.start,
                    id.span.start,
                ),
            );
            parts.push(self.build_identifier_doc(id));
        }

        // Space before type params or params if no name: `function <T>` or `function ()`
        if func.id.is_none() {
            parts.push(d.text(" "));
        }

        // Type params, params, return type, and body (signature_doc handles type params)
        parts.push(self.build_function_doc_body(func));

        d.concat(&parts)
    }

    /// Build doc for function/method params NOT in their own group
    ///
    /// `params_start` is the position of the opening paren (for comment detection).
    /// `trailing_comments_end` is where trailing comments after the last param end
    /// (typically return type start or body start).
    pub(in crate::printer) fn build_method_params_doc_ungrouped(
        &self,
        func: &internal::FunctionExpression,
    ) -> DocId {
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
    ) -> DocId {
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
    ) -> DocId {
        let d = self.d();
        if params.is_empty() {
            return d.text("()");
        }

        // Prettier's shouldHugFunctionParameters: single param that's an object/array pattern
        // gets hugged - no breaks added around it, the pattern handles its own expansion.
        // This keeps `({` and `}: Type)` together, letting the pattern's content break:
        //   function fn({
        //       a,
        //       b,
        //   }: Type): void {}
        // NOT:
        //   function fn(
        //       {a, b}: Type,
        //   ): void {}
        //
        // Also applies to parameters with TypeLiteral type annotations like `a?: { b: T }`:
        //   function fn(a?: {
        //       b: T;
        //   }): void {}
        // NOT:
        //   function fn(
        //       a?: { b: T },
        //   ): void {}
        let no_leading_comments = !self.has_comments_between(
            params_start.unwrap_or_else(|| params[0].span().start),
            params[0].span().start,
        );
        let should_hug_single_pattern = params.len() == 1
            && (is_huggable_pattern(&params[0]) || has_huggable_type_annotation(&params[0]))
            && no_leading_comments;

        if should_hug_single_pattern {
            // Hug mode: just ( + pattern + optional trailing comma + )
            let param_doc = self.build_function_parameter_doc(&params[0]);
            return d.parens(param_doc);
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
                    inner_parts.push(d.hardline());
                } else {
                    inner_parts.push(d.line());
                }
            }

            // Add leading comments for this param
            // Use proper line breaks for line comments on their own line
            // For non-first params, find comma position to filter properly
            let prev_comma_pos = if i > 0 {
                self.find_comma_after(params[i - 1].span().end)
            } else {
                None
            };
            inner_parts.push(self.build_leading_param_comments(
                search_start,
                param_start,
                prev_comma_pos,
            ));

            // Use FunctionParameter context for object patterns
            inner_parts.push(self.build_function_parameter_doc(param));

            // Handle trailing same-line comments
            let search_end = if is_last {
                self.param_trailing_end(params, i, trailing_comments_end)
            } else {
                params[i + 1].span().start
            };

            // Find comma position for non-last params
            let comma_pos = if !is_last {
                self.find_comma_after(param.span().end)
            } else {
                None
            };

            // Collect same-line comments
            let same_line_comments: Vec<_> =
                tsv_lang::comments_in_range(self.comments, param.span().end, search_end)
                    .filter(|c| self.is_same_line(param.span().end, c.span.start))
                    .collect();

            // Block comments BEFORE comma go before comma
            for comment in same_line_comments
                .iter()
                .filter(|c| c.is_block && comma_pos.is_none_or(|pos| c.span.start < pos))
            {
                inner_parts.push(d.text(" "));
                inner_parts.push(self.build_comment_doc(comment));
            }

            // Add trailing comma
            // For non-last params: always add comma (it's a separator)
            // For last param: add comma if forcing break and not rest param
            let needs_comma = !is_last || (force_break && !has_rest_param);
            if needs_comma {
                inner_parts.push(d.text(","));
            }

            // Line comments (same-line) go after comma (excluded from width)
            // Block comments AFTER comma are handled as leading for next param
            for comment in same_line_comments.iter().filter(|c| !c.is_block) {
                inner_parts.push(self.build_trailing_line_comment_doc(comment));
            }

            // Own-line line comments (on their own line after last param, before `)`)
            // Only for the last param - non-last param comments are handled as leading for next param
            if is_last {
                for comment in
                    tsv_lang::comments_in_range(self.comments, param.span().end, search_end).filter(
                        |c| !c.is_block && !self.is_same_line(param.span().end, c.span.start),
                    )
                {
                    inner_parts.push(d.hardline());
                    inner_parts.push(self.build_comment_doc(comment));
                }
            }
        }

        // No group - outer signature group controls breaking
        let mut result = vec![d.text("(")];

        if force_break {
            // When forcing break (trailing comments or param properties), use hardlines
            result.push(d.indent(d.concat(&[d.hardline(), d.concat(&inner_parts)])));
            result.push(d.hardline());
        } else {
            result.push(d.indent_softline(d.concat(&inner_parts)));
            // Trailing comma when broken, unless there's a rest param
            if !has_rest_param {
                result.push(d.trailing_comma());
            }
            result.push(d.softline());
        }

        result.push(d.text(")"));

        d.concat(&result)
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
            if !self.is_same_line(start, comment.span.start) {
                return true;
            }
        }
        false
    }

    /// Build doc for leading comments before a parameter
    /// Handles line comments on their own line with proper hardlines
    /// `prev_comma_pos`: if Some, filter out trailing comments for the previous param
    fn build_leading_param_comments(
        &self,
        start: u32,
        end: u32,
        prev_comma_pos: Option<u32>,
    ) -> DocId {
        let d = self.d();
        let comments: Vec<_> = tsv_lang::comments_in_range(self.comments, start, end)
            .filter(|c| {
                let Some(comma) = prev_comma_pos else {
                    return true; // First param - keep all comments
                };
                // Different line from prev param - definitely a leading comment
                if !self.is_same_line(start, c.span.start) {
                    return true;
                }
                // Same line as prev param: only keep block comments after the comma
                // (line comments go in line_suffix, block comments before comma are trailing)
                c.is_block && c.span.start >= comma
            })
            .collect();
        if comments.is_empty() {
            return d.empty();
        }

        let mut parts = Vec::new();

        for (i, comment) in comments.iter().enumerate() {
            // Check if comment is on its own line (not same line as previous content)
            let prev_pos = if i == 0 {
                start
            } else {
                comments[i - 1].span.end
            };
            let on_own_line = !self.is_same_line(prev_pos, comment.span.start);

            if on_own_line && i > 0 {
                // Comment on its own line (not first) - add hardline before it
                parts.push(d.hardline());
            }
            parts.push(self.build_comment_doc(comment));
        }

        // Check if the param itself is on its own line after the last comment
        let last_comment_end = comments.last().map_or(start, |c| c.span.end);
        let param_on_own_line = !self.is_same_line(last_comment_end, end);

        if param_on_own_line {
            parts.push(d.hardline());
        } else {
            // Inline - add space after comment
            parts.push(d.text(" "));
        }

        d.concat(&parts)
    }

    /// Build a Doc for a class expression
    pub(in crate::printer) fn build_class_expression_doc(
        &self,
        class_expr: &internal::ClassExpression,
    ) -> DocId {
        let d = self.d();
        let mut parts = Vec::new();

        // 'class' keyword
        parts.push(d.text("class"));

        // Optional class name
        if let Some(id) = &class_expr.id {
            parts.push(d.text(" "));
            parts.push(self.build_identifier_doc(id));
        }

        // Type parameters (TypeScript generics): class<T>
        // Use _wrapping version for width-based line breaking
        if let Some(type_params) = &class_expr.type_parameters {
            parts.push(self.build_type_parameter_declaration_doc_wrapping(type_params));
        }

        // Optional extends clause
        if let Some(super_class) = &class_expr.super_class {
            parts.push(d.text(" extends "));
            parts.push(self.build_expression_doc(super_class));
            // Type arguments: extends Base<T>
            if let Some(super_type_params) = &class_expr.super_type_parameters {
                parts.push(self.build_type_parameter_instantiation_doc(super_type_params));
            }
        }

        // Implements clause (extract from source for simplicity)
        if !class_expr.implements.is_empty() {
            parts.push(d.text(" implements "));
            let first_impl = &class_expr.implements[0];
            let last_impl = &class_expr.implements[class_expr.implements.len() - 1];
            let impl_start = first_impl.span.start_usize();
            let impl_end = last_impl.span.end_usize();
            let impl_str = &self.source[impl_start..impl_end];
            parts.push(d.text_owned(impl_str.to_string()));
        }

        // Space before body
        parts.push(d.text(" "));

        // Class body
        parts.push(self.build_class_body_doc(&class_expr.body, false));

        d.concat(&parts)
    }
}
