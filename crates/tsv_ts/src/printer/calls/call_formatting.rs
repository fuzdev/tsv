// Main call expression formatting logic
//
// Contains the primary `build_call_doc_with_wrapping` function that handles
// all the special cases for call expression formatting.

use super::super::utils::{
    could_expand_arrow_body, has_block_function_before_last, has_multiple_function_args,
    is_array_or_object_unwrapped, is_block_function, is_hopefully_short_arg,
    last_arg_is_array_or_object, preceding_args_allow_hug,
};
use super::super::{
    ParenContext, Printer, has_multiline_content, needs_parens, template_literal_has_newlines,
};
use super::arg_comments::{
    PartitionedComments, any_comment_forces_expansion, find_comma_pos, has_inter_argument_comments,
    has_trailing_comments_on_args, is_comment_after_comma, is_comment_before_comma,
    should_force_expansion_for_comments,
};
use super::arg_wrapping::{
    arg_needs_soft_wrap, arrow_has_type_annotations, build_args_split_last,
    build_arrow_inline_signature, build_expand_all_args, build_inline_args,
    build_inline_or_expand_all, wrap_call_with_hard_breaks, wrap_call_with_soft_breaks,
};
use super::module_paths::{get_module_path_chain_break, is_boolean_call, is_module_path_no_break};
use super::test_patterns::{get_member_chain_parts, is_test_call};
use crate::ast::internal;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

/// Check if call should use "expand first arg" pattern
///
/// This matches prettier's behavior for calls like `setTimeout(() => {...}, 100)`:
/// - First arg is function/arrow with block body
/// - Remaining args are "hopefully short" (simple values)
/// - Result: first arg expands, tail args stay inline after closing `}`
fn should_expand_first_arg(printer: &Printer, args: &[internal::Expression]) -> bool {
    // Need exactly 2 args (first is function, second is short)
    if args.len() != 2 {
        return false;
    }

    let first_arg = &args[0];
    let second_arg = &args[1];

    // First arg must be a function with block body
    if !is_block_function(first_arg) {
        return false;
    }

    // Second arg must be short/simple (won't expand)
    // Exclude: functions, ternaries, spreads - these should expand all args
    // Also exclude non-empty objects/arrays (they expand), but allow empty {} and []
    match second_arg {
        internal::Expression::ArrowFunctionExpression(_)
        | internal::Expression::FunctionExpression(_)
        | internal::Expression::ConditionalExpression(_)
        | internal::Expression::SpreadElement(_) => return false,
        // Non-empty objects expand - use "expand all args" instead
        internal::Expression::ObjectExpression(obj) if !obj.properties.is_empty() => return false,
        // Non-empty arrays expand - use "expand all args" instead
        internal::Expression::ArrayExpression(arr) if !arr.elements.is_empty() => return false,
        // Empty {} or [] with comments inside should expand (comments will break)
        internal::Expression::ObjectExpression(obj)
            if printer.has_comments_between(obj.span.start, obj.span.end) =>
        {
            return false;
        }
        internal::Expression::ArrayExpression(arr)
            if printer.has_comments_between(arr.span.start, arr.span.end) =>
        {
            return false;
        }
        // Truly empty {} and [] are short and don't expand - allow "expand first arg"
        _ => {}
    }

    is_hopefully_short_arg(second_arg)
}

/// Print a call expression: `foo()`, `obj.method(arg1, arg2)`
///
/// For method chains like `arr.filter().map()`, wraps with leading `.`:
/// ```javascript
/// arr
///     .filter(...)
///     .map(...)
/// ```
///
/// For standalone calls and simple method calls, wraps args when they exceed print_width:
/// ```javascript
/// fn(
///     arg1,
///     arg2,
/// )
/// assert.deepStrictEqual(
///     longArg1,
///     [1, 2],
/// )
/// ```
pub(super) fn build_call_doc_with_wrapping(
    printer: &Printer,
    call: &internal::CallExpression,
) -> Doc {
    let callee_doc = printer.build_expression_doc(&call.callee);

    // Wrap callee in parens if needed (e.g., ternary: `(a ? b : c)()`)
    // This must happen BEFORE adding removed-paren comments so comments stay outside
    let callee_doc = if needs_parens(&call.callee, ParenContext::Callee) {
        doc::parens(callee_doc)
    } else {
        callee_doc
    };

    // Check for comments between removed parentheses and callee
    // e.g., (/* comment */ foo)() has call.span.start at '(' and callee.span.start at 'foo'
    // The comment is in the range [call.span.start, callee.span.start) and needs to be preserved
    // Note: This happens AFTER parens wrapping so `(/* c */ (a ? b : c))()` -> `/* c */ (a ? b : c)()`
    let callee = printer.prepend_removed_paren_comments(
        call.span.start,
        call.callee.span().start,
        callee_doc,
    );

    // Handle optional chaining
    let callee = if call.optional {
        doc::concat(vec![callee, doc::text("?.")])
    } else {
        callee
    };

    // Build type arguments: `<T, U>`
    let type_args_doc = call
        .type_arguments
        .as_ref()
        .map(|ta| printer.build_type_parameter_instantiation_doc(ta));

    // Combine callee with type arguments
    let callee = match type_args_doc {
        Some(ta_doc) => doc::concat(vec![callee, ta_doc]),
        None => callee,
    };

    // Empty args: just `fn()` or `fn<T>()`
    if call.arguments.is_empty() {
        return doc::concat(vec![callee, doc::text("()")]);
    }

    // Check for comments inside call arguments (e.g., require(/* comment */ 'a'))
    // If there are line comments, expand to multi-line format
    if call.arguments.len() == 1 {
        let first_arg = &call.arguments[0];
        // Find the opening paren position (just after callee ends)
        let paren_open = call.callee.span().end;
        let arg_start = first_arg.span().start;
        let arg_end = first_arg.span().end;
        let paren_close = call.span.end;

        let has_line_comments = printer.has_line_comments_between(paren_open, arg_start);
        if has_line_comments {
            // Multi-line format: fn(\n\t// comment\n\targ,\n)
            let mut comment_parts = Vec::new();
            for comment in tsv_lang::comments_in_range(printer.comments, paren_open, arg_start) {
                comment_parts.push(printer.build_comment_doc(comment));
                comment_parts.push(doc::hardline());
            }

            let arg_doc = doc::concat(vec![
                doc::concat(comment_parts),
                printer.build_expression_doc(first_arg),
            ]);

            return wrap_call_with_hard_breaks(callee, arg_doc);
        }

        // Check for inline block comments (single binary search via _opt)
        if let Some(inline_comments) =
            printer.build_inline_comments_between_doc_no_leading_space_opt(paren_open, arg_start)
        {
            return doc::concat(vec![
                callee,
                doc::text("("),
                inline_comments,
                doc::text(" "),
                printer.build_expression_doc(first_arg),
                // Check for trailing comments
                printer.build_inline_comments_between_doc(arg_end, paren_close),
                doc::text(")"),
            ]);
        }
    }

    // Test function calls (it, test, describe, etc.) stay on one line
    // even if they exceed print width
    if is_test_call(call, printer) {
        // Build callee as flat string (no conditionalGroup)
        // This prevents breaking at `.skip` etc. even when very long
        let flat_callee = if let Some(parts) = get_member_chain_parts(&call.callee) {
            let callee_str: String = parts
                .iter()
                .rev()
                .map(|sym| printer.resolve_symbol(*sym))
                .collect::<Vec<_>>()
                .join(".");
            doc::text_owned(callee_str)
        } else {
            callee
        };

        // Check for trailing comments on last arg
        let last_arg = call
            .arguments
            .last()
            .expect("is_test_call requires arguments");
        let paren_close = call.span.end;
        let mut parts = vec![
            flat_callee,
            doc::text("("),
            doc::join(
                call.arguments
                    .iter()
                    .map(|arg| printer.build_expression_doc(arg)),
                ", ",
            ),
            doc::text(")"),
        ];

        // Add trailing comments as line suffix (stays on same line)
        if let Some(suffix) =
            printer.build_trailing_comments_line_suffix(last_arg.span().end, paren_close)
        {
            parts.push(suffix);
        }

        return doc::concat(parts);
    }

    // Module path calls that should not break at arguments (e.g., require.resolve)
    // Keep the call on one line; let assignment/parent break instead
    if is_module_path_no_break(call, printer) && !has_trailing_comments_on_args(call, printer) {
        return doc::concat(vec![
            callee,
            doc::text("("),
            doc::join(
                call.arguments
                    .iter()
                    .map(|arg| printer.build_expression_doc(arg)),
                ", ",
            ),
            doc::text(")"),
        ]);
    }

    // Module path calls (require.resolve.paths, import.meta.resolve) break at chain
    // rather than at arguments, keeping the path on the same line as the method
    if let Some((base_expr, method_name)) = get_module_path_chain_break(call, printer)
        .filter(|_| !has_trailing_comments_on_args(call, printer))
    {
        let base_doc = printer.build_expression_doc(base_expr);
        let method_str = printer.resolve_symbol(method_name.name);
        let arg_doc = printer.build_expression_doc(&call.arguments[0]);

        // Format: base\n\t.method(arg)
        // When it fits on one line, don't break
        return doc::group(doc::concat(vec![
            base_doc,
            doc::indent_softline(doc::concat(vec![
                doc::text_owned(format!(".{method_str}(")),
                arg_doc,
                doc::text(")"),
            ])),
        ]));
    }

    // Single function argument: "hugged" formatting
    // - Block arrows stay hugged if first line fits, wrap if it doesn't
    // - Expression arrows use width-aware group (wrap when exceeds line limit)
    // Skip hugging if there are trailing comments - let comment handling block handle it
    if call.arguments.len() == 1 && !has_trailing_comments_on_args(call, printer) {
        let arg = &call.arguments[0];

        // Non-huggable arguments: use soft-break wrapping so outer call can break first
        // (call expressions, member expressions, new expressions, identifiers)
        if arg_needs_soft_wrap(arg) {
            let arg_doc = printer.build_expression_doc(arg);
            return wrap_call_with_soft_breaks(callee, arg_doc);
        }

        match arg {
            // Block arrow: use conditional_group to let Doc decide hug vs wrap
            internal::Expression::ArrowFunctionExpression(arrow) if !arrow.body.is_expression() => {
                let arrow_doc = printer.build_expression_doc(arg);
                return doc::conditional_group(vec![
                    // State 1: hugged - callee((arrow) => { body })
                    doc::concat(vec![
                        callee.clone(),
                        doc::text("("),
                        arrow_doc.clone(),
                        doc::text(")"),
                    ]),
                    // State 2: wrapped - callee(\n\t(arrow) => { body },\n)
                    doc::concat(vec![
                        callee,
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

            // Regular function expression: keep hugged (block body handles own formatting)
            internal::Expression::FunctionExpression(_) => {
                return doc::concat(vec![
                    callee,
                    doc::text("("),
                    printer.build_expression_doc(arg),
                    doc::text(")"),
                ]);
            }

            // Object/array literals (or type assertions wrapping them): hug them
            // e.g., @decorator({...}), fn([item]), fn({...} as T), fn([...] satisfies T)
            _ if is_array_or_object_unwrapped(arg) => {
                return doc::concat(vec![
                    callee,
                    doc::text("("),
                    printer.build_expression_doc(arg),
                    doc::text(")"),
                ]);
            }

            // Short literals (non-string or short string): hug them
            // Long string literals and multiline strings should use standard wrapping
            internal::Expression::Literal(lit) => {
                let span_len = (lit.span.end - lit.span.start) as usize;
                let raw = lit.span.extract(printer.source);
                let is_multiline = raw.contains('\n');
                // Hug short, single-line literals (<=25 chars)
                if span_len <= 25 && !is_multiline {
                    return doc::concat(vec![
                        callee,
                        doc::text("("),
                        printer.build_expression_doc(arg),
                        doc::text(")"),
                    ]);
                }
                // Long or multiline string - fall through to standard wrapping
            }

            // Expression arrow: check special cases
            internal::Expression::ArrowFunctionExpression(arrow) => {
                if let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body {
                    // Object/array literal: hug it (array breaks internally when long)
                    if matches!(
                        &**body_expr,
                        internal::Expression::ObjectExpression(_)
                            | internal::Expression::ArrayExpression(_)
                    ) {
                        return doc::concat(vec![
                            callee,
                            doc::text("("),
                            printer.build_expression_doc(arg),
                            doc::text(")"),
                        ]);
                    }

                    // Call expression body with complex arguments: keep signature inline
                    // Pattern: call((params) => anotherCall(...complex...)) keeps params inline
                    // Only applies when:
                    // 1. The nested call has complex arguments (blocks, objects, etc.)
                    // 2. The arrow has NO type annotations (plain signature)
                    let has_no_type_annotations = !arrow_has_type_annotations(arrow);

                    let is_complex_call_body =
                        if let internal::Expression::CallExpression(nested_call) = &**body_expr {
                            // Check if any argument is complex (block arrow with statements, non-empty objects/arrays, etc.)
                            nested_call.arguments.iter().any(|arg| {
                                match arg {
                                    internal::Expression::ArrowFunctionExpression(arr) => {
                                        // Only consider block arrows with statements as complex
                                        if let internal::ArrowFunctionBody::BlockStatement(block) =
                                            &arr.body
                                        {
                                            !block.body.is_empty()
                                        } else {
                                            false
                                        }
                                    }
                                    internal::Expression::ObjectExpression(obj) => {
                                        !obj.properties.is_empty()
                                    }
                                    internal::Expression::ArrayExpression(arr) => {
                                        !arr.elements.is_empty()
                                    }
                                    internal::Expression::FunctionExpression(_) => true,
                                    _ => false,
                                }
                            })
                        } else {
                            false
                        };

                    if is_complex_call_body && has_no_type_annotations {
                        // Build arrow doc with non-breaking signature
                        // Use concat (not group) for signature to prevent breaking
                        let mut arrow_parts = Vec::new();
                        if arrow.r#async {
                            arrow_parts.push(doc::text("async "));
                        }

                        // Build params inline without softlines/breaks
                        if arrow.params.is_empty() {
                            arrow_parts.push(doc::text("()"));
                        } else if arrow.params.len() == 1 && arrow.params_start.is_none() {
                            // Single param without parens
                            arrow_parts
                                .push(printer.build_function_parameter_doc(&arrow.params[0]));
                        } else {
                            // Multiple params or single param with parens - inline format
                            arrow_parts.push(doc::text("("));
                            arrow_parts.push(doc::join(
                                arrow
                                    .params
                                    .iter()
                                    .map(|p| printer.build_function_parameter_doc(p)),
                                ", ",
                            ));
                            arrow_parts.push(doc::text(")"));
                        }
                        // Note: No return type check needed - has_no_type_annotations guarantees none

                        // Build custom call wrapping that keeps arrow signature inline
                        // The outer group controls whether we break the whole call
                        // The inner group around the arrow body controls whether the body breaks
                        return doc::group(doc::concat(vec![
                            callee,
                            doc::text("("),
                            doc::concat(arrow_parts),
                            doc::text(" =>"),
                            doc::group(doc::concat(vec![
                                doc::indent_line(printer.build_expression_doc(body_expr)),
                                doc::trailing_comma(),
                            ])),
                            doc::softline(),
                            doc::text(")"),
                        ]));
                    }

                    // Expandable body (ternary): use conditional parens
                    // Prettier's "expand last arg" pattern:
                    // - Flat: `map((x) => (x ? y : z))` - parens prevent `<=` ambiguity
                    // - Break: `map((x) =>\n  x ? y : z,)` - no parens, indented
                    if could_expand_arrow_body(body_expr) {
                        // Build arrow signature with grouping
                        // The group allows params to break when needed, but assignment logic
                        // uses is_complex_call_expression to decide layout strategy
                        let sig_doc = doc::group(printer.build_arrow_signature_doc(arrow));

                        // Build body expression
                        let body_doc = printer.build_expression_doc(body_expr);

                        // Build state 0: fully flat version including call wrapping
                        // Structure: callee + "(" + sig + " => (" + body + ")" + ")"
                        // This includes the full context so conditional_group can measure correctly
                        let state_flat = doc::concat(vec![
                            callee.clone(),
                            doc::text("("),
                            sig_doc.clone(),
                            doc::text(" => ("),
                            body_doc.clone(),
                            doc::text("))"), // Close both arrow body and call
                        ]);

                        // Build state 1: break version with params on call line, body breaks
                        // Structure: callee + "(" + sig + " =>" + indent([hardline, body, ","]) + hardline + ")"
                        // First hardline: breaks after "=>"
                        // Second hardline: breaks before ")" to put closing paren on its own line
                        // Note: Use literal "," not trailing_comma() because state[1] is used in Flat mode
                        let state_break = doc::concat(vec![
                            callee.clone(),
                            doc::text("("),
                            sig_doc.clone(),
                            doc::text(" =>"),
                            doc::indent(doc::concat(vec![
                                doc::hardline(),
                                body_doc.clone(),
                                doc::text(","),
                            ])),
                            doc::hardline(),
                            doc::text(")"),
                        ]);

                        // Build state 2: all args broken out (fallback for Break mode)
                        // This is used when the parent group breaks and we need maximum expansion
                        // Structure: callee + "(\n" + indent([sig + " =>" + indent([hardline, body, ","]) + softline]) + "\n)"
                        let state_all_broken = doc::concat(vec![
                            callee, // Last use, no clone needed
                            doc::text("("),
                            doc::indent(doc::concat(vec![
                                doc::hardline(),
                                sig_doc, // Last use, no clone needed
                                doc::text(" =>"),
                                doc::indent(doc::concat(vec![
                                    doc::hardline(),
                                    body_doc, // Last use, no clone needed
                                    doc::trailing_comma(),
                                ])),
                            ])),
                            doc::hardline(),
                            doc::text(")"),
                        ]);

                        // Use conditional_group with 3 states to match Prettier
                        // State 0: fully flat
                        // State 1: arrow breaks (checked during fits())
                        // State 2: all broken (only used in Break mode)
                        return doc::conditional_group(vec![
                            state_flat,
                            state_break,
                            state_all_broken,
                        ]);
                    }
                }
                // Other expression arrows: fall through to wrap
            }

            // Other arguments: fall through to standard handling
            _ => {}
        }

        // Wrap callback with width-aware breaking
        if let internal::Expression::ArrowFunctionExpression(arrow) = &call.arguments[0] {
            if let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body {
                // Prettier keeps `fn((x) =>` together (sig on opening line) only when:
                // 1. Body is a call expression
                // 2. No type annotations (return type, type params, or param types)
                // Otherwise it wraps at `fn(` putting the whole arrow on the next line.
                if matches!(&**body_expr, internal::Expression::CallExpression(_))
                    && !arrow_has_type_annotations(arrow)
                {
                    let arrow_doc = printer.build_expression_doc(&call.arguments[0]);
                    let body_doc = printer.build_expression_doc(body_expr);
                    let inline_sig = build_arrow_inline_signature(printer, arrow);

                    return doc::conditional_group(vec![
                        // Flat: callee(() => body)
                        doc::concat(vec![
                            callee.clone(),
                            doc::text("("),
                            arrow_doc,
                            doc::text(")"),
                        ]),
                        // Break: callee(() =>\n  body,\n)
                        doc::concat(vec![
                            callee,
                            doc::text("("),
                            inline_sig,
                            doc::text(" =>"),
                            doc::indent(doc::concat(vec![
                                doc::hardline(),
                                body_doc,
                                doc::text(","),
                            ])),
                            doc::hardline(),
                            doc::text(")"),
                        ]),
                    ]);
                }
                // Other expression types: fall through to standard wrapping
            }
            // Block arrow or non-call expression body: standard wrapping
            let arg_doc = printer.build_expression_doc(&call.arguments[0]);
            return wrap_call_with_soft_breaks(callee, arg_doc);
        }
    }

    // Single template literal argument with embedded newlines: never break
    // Prettier keeps these inline: `someFunction(\`a\nb\`)`
    // Only applies when the template itself contains newlines
    if call.arguments.len() == 1 {
        let has_template_with_newlines = match &call.arguments[0] {
            internal::Expression::TemplateLiteral(template) => {
                template_literal_has_newlines(template)
            }
            internal::Expression::TaggedTemplateExpression(tagged) => {
                template_literal_has_newlines(&tagged.quasi)
            }
            _ => false,
        };

        if has_template_with_newlines {
            let arg_doc = printer.build_expression_doc(&call.arguments[0]);
            let mut parts = vec![callee, doc::text("("), arg_doc, doc::text(")")];

            // Add trailing comments as line suffix (moves outside call, like test calls)
            let last_arg = &call.arguments[0];
            let paren_close = call.span.end;
            if let Some(suffix) =
                printer.build_trailing_comments_line_suffix(last_arg.span().end, paren_close)
            {
                parts.push(suffix);
            }

            return doc::concat(parts);
        }
    }

    // Check if any argument has multiline content (e.g., line continuation strings)
    // Prettier expands calls containing multiline strings (recursively)
    let has_multiline = call
        .arguments
        .iter()
        .any(|arg| has_multiline_content(arg, printer.source));

    if has_multiline {
        // Force expansion with hardlines for multiline content
        let arg_parts = doc::join_doc(
            call.arguments
                .iter()
                .map(|arg| printer.build_expression_doc(arg)),
            doc::comma_hardline(),
        );

        return wrap_call_with_hard_breaks(callee, arg_parts);
    }

    // "Expand first arg" pattern: when first arg is a function with block body
    // and remaining args are short, hug the function and put tail args after closing }
    // e.g., setTimeout(() => { tick(); }, 100);
    if should_expand_first_arg(printer, &call.arguments)
        && !has_trailing_comments_on_args(call, printer)
    {
        let first_arg_doc = printer.build_expression_doc(&call.arguments[0]);

        // Build tail args (everything after first)
        let mut tail_parts = Vec::new();
        for arg in call.arguments.iter().skip(1) {
            tail_parts.push(doc::text(", "));
            tail_parts.push(printer.build_expression_doc(arg));
        }

        // Structure: callee + ( + first_arg_with_breaks + , + tail_args + )
        // The first arg can expand internally, but tail args stay inline
        return doc::concat(vec![
            callee,
            doc::text("("),
            first_arg_doc,
            doc::concat(tail_parts),
            doc::text(")"),
        ]);
    }

    // Multiple arrow/function arguments: always break (Prettier behavior)
    // e.g., fn((a) => a, (b) => b) → fn(\n\t(a) => a,\n\t(b) => b,\n)
    if has_multiple_function_args(&call.arguments) && !has_trailing_comments_on_args(call, printer)
    {
        let arg_parts = doc::join_doc(
            call.arguments
                .iter()
                .map(|arg| printer.build_expression_doc(arg)),
            doc::comma_hardline(),
        );

        return wrap_call_with_hard_breaks(callee, arg_parts);
    }

    // Expand last arg pattern: N args with last being arrow/function
    // Prettier wraps the arrow in its own group to isolate hardlines,
    // then uses conditional_group to try inline first
    // e.g., fn('a', (x) => { ... }) stays inline, fn('long...', (x) => { ... }) breaks all
    if call.arguments.len() >= 2 {
        let last_is_function = matches!(
            call.arguments.last(),
            Some(
                internal::Expression::ArrowFunctionExpression(_)
                    | internal::Expression::FunctionExpression(_)
            )
        );

        // Check for comments that force expansion (line comments or block comments on own line).
        // Inline block comments are allowed without forcing expansion.
        let paren_open = call.callee.span().end;

        if last_is_function
            && preceding_args_allow_hug(&call.arguments, printer.line_breaks)
            && !any_comment_forces_expansion(call, printer, paren_open)
        {
            let (head_parts, last_arg_doc, all_args_broken) =
                build_args_split_last(&call.arguments, printer);

            // Special case: expression arrow with call expression body
            // Prettier keeps preceding args inline and only breaks arrow body after =>
            // e.g., fn({a: 1}, (x) =>\n  call(x, ...),\n)
            if let Some(internal::Expression::ArrowFunctionExpression(arrow)) =
                call.arguments.last()
                && let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body
                    && matches!(&**body_expr, internal::Expression::CallExpression(_))
                        && !arrow_has_type_annotations(arrow)
                    {
                        let inline_sig = build_arrow_inline_signature(printer, arrow);
                        let body_doc = printer.build_expression_doc(body_expr);

                        // State 1: all inline (reuse existing helper)
                        let state_inline =
                            build_inline_args(callee.clone(), head_parts.clone(), last_arg_doc);

                        // State 2: preceding args inline, arrow body breaks after =>
                        let state_break_body = doc::concat(vec![
                            callee.clone(),
                            doc::text("("),
                            doc::concat(head_parts),
                            inline_sig,
                            doc::text(" =>"),
                            doc::indent(doc::concat(vec![
                                doc::hardline(),
                                body_doc,
                                doc::text(","),
                            ])),
                            doc::hardline(),
                            doc::text(")"),
                        ]);

                        // State 3: all args expanded (reuse existing helper)
                        let state_expand_all = build_expand_all_args(callee, all_args_broken);

                        return doc::conditional_group(vec![
                            state_inline,
                            state_break_body,
                            state_expand_all,
                        ]);
                    }

            // Try: inline, or break all args
            // Note: last arg contains hardlines, so state 1 only succeeds if the whole
            // line (including arrow signature) fits within print_width
            return build_inline_or_expand_all(callee, head_parts, last_arg_doc, all_args_broken);
        }
    }

    // Check for any comments in arguments (leading, inter-argument, or trailing)
    let paren_open = call.callee.span().end;
    let has_leading_comments = !call.arguments.is_empty()
        && printer.has_comments_between(paren_open, call.arguments[0].span().start);
    let has_inter_arg_comments = has_inter_argument_comments(call, printer);
    let has_trailing_arg_comments = has_trailing_comments_on_args(call, printer);

    if has_leading_comments || has_inter_arg_comments || has_trailing_arg_comments {
        // Build arguments with leading and/or inter-argument comments
        let mut arg_parts = Vec::new();
        let mut force_expansion = false;
        let mut has_trailing_comma_on_last = false;

        for (i, arg) in call.arguments.iter().enumerate() {
            // Handle leading comments before first argument
            if i == 0 && has_leading_comments {
                let first_arg_start = arg.span().start;

                if should_force_expansion_for_comments(printer, paren_open, first_arg_start) {
                    force_expansion = true;
                }

                arg_parts.push(printer.build_inline_comments_between_doc_no_leading_space(
                    paren_open,
                    first_arg_start,
                ));
                arg_parts.push(doc::line());
            }

            // Build the argument
            arg_parts.push(printer.build_expression_doc(arg));

            // Check for comments after this argument (before next arg or closing paren)
            if i < call.arguments.len() - 1 {
                let arg_end = arg.span().end;
                let next_arg_start = call.arguments[i + 1].span().start;

                if printer.has_comments_between(arg_end, next_arg_start) {
                    if should_force_expansion_for_comments(printer, arg_end, next_arg_start) {
                        force_expansion = true;
                    }

                    let pc = PartitionedComments::new(
                        printer.comments,
                        printer.line_breaks,
                        arg_end,
                        next_arg_start,
                    );

                    let comma_pos = find_comma_pos(printer.source, arg_end, next_arg_start);

                    if pc.has_trailing_line() {
                        // Trailing line comments: comma, comment, hardline
                        force_expansion = true;
                        arg_parts.push(doc::text(","));
                        for comment in &pc.trailing_line {
                            arg_parts.push(doc::text(" "));
                            arg_parts.push(printer.build_comment_doc(comment));
                        }
                        arg_parts.push(doc::hardline());
                    } else if pc.has_trailing_block() {
                        // Trailing block comments: place relative to comma based on source position
                        if let Some(cpos) = comma_pos {
                            for comment in &pc.trailing_block {
                                if is_comment_before_comma(comment, cpos) {
                                    arg_parts.push(doc::text(" "));
                                    arg_parts.push(printer.build_comment_doc(comment));
                                }
                            }
                        }
                        arg_parts.push(doc::text(","));
                        if let Some(cpos) = comma_pos {
                            for comment in &pc.trailing_block {
                                if is_comment_after_comma(comment, cpos) {
                                    arg_parts.push(doc::text(" "));
                                    arg_parts.push(printer.build_comment_doc(comment));
                                }
                            }
                        }
                        arg_parts.push(doc::line());
                    } else {
                        // No trailing comments, add comma and line
                        arg_parts.push(doc::text(","));
                        arg_parts.push(doc::line());
                    }

                    // Add leading comments - inline with next arg if on same line
                    pc.emit_leading_comments_inline_aware(
                        &mut arg_parts,
                        printer,
                        next_arg_start,
                    );
                } else {
                    // No comments, just comma and line
                    arg_parts.push(doc::comma_line());
                }
            } else {
                // Last argument - check for trailing line comments before closing paren
                let arg_end = arg.span().end;
                let paren_close = call.span.end;

                let pc = PartitionedComments::new(
                    printer.comments,
                    printer.line_breaks,
                    arg_end,
                    paren_close,
                );

                if pc.has_trailing_line() {
                    arg_parts.push(doc::text(","));

                    // Build comment docs: " // comment" for each
                    let comment_docs: Vec<_> = pc
                        .trailing_line
                        .iter()
                        .flat_map(|c| [doc::text(" "), printer.build_comment_doc(c)])
                        .collect();
                    let comments = doc::concat(comment_docs);

                    // Line comments always force the CALL to expand - the newline after the
                    // comment means the call must break to multiple lines.
                    force_expansion = true;

                    // Arrays/objects have their own groups that decide internal expansion.
                    // Use line_suffix to exclude the comment from width calculations, so
                    // the array/object can stay inline even when the comment exceeds print_width.
                    // The force_expansion above ensures the call itself expands.
                    if is_array_or_object_unwrapped(arg) {
                        arg_parts.push(doc::line_suffix(comments));
                    } else {
                        arg_parts.push(comments);
                    }
                    has_trailing_comma_on_last = true;
                }
            }
        }

        let arg_doc = doc::concat(arg_parts);

        // Force expansion if needed, otherwise allow collapsing.
        // Use a group with break_parent instead of literal hardlines to avoid
        // propagating breaks to parent (e.g., assignment) during fits().
        if force_expansion {
            // Build manually when we have trailing comments (we already added our commas)
            // Add trailing comma after last arg ONLY if we didn't already add one
            let trailing = if has_trailing_comma_on_last {
                doc::empty()
            } else {
                doc::text(",")
            };
            // Use hardlines for the expansion. The assignment should use NeverBreakAfterOperator
            // for calls since they handle their own expansion.
            return doc::concat(vec![
                callee,
                doc::text("("),
                doc::indent(doc::concat(vec![doc::hardline(), arg_doc, trailing])),
                doc::hardline(),
                doc::text(")"),
            ]);
        }

        // When we have a trailing comment on the last arg, we already added the comma
        // before the comment. Use a custom soft-break structure that doesn't add
        // another trailing comma.
        if has_trailing_comma_on_last {
            return doc::concat(vec![
                callee,
                doc::group(doc::concat(vec![
                    doc::text("("),
                    doc::indent_softline(arg_doc),
                    doc::softline(),
                    doc::text(")"),
                ])),
            ]);
        }

        return wrap_call_with_soft_breaks(callee, arg_doc);
    }

    // Block function before last arg: force expansion
    // e.g., fn((x) => { ... }, {a: 1}) → all args on separate lines
    if call.arguments.len() >= 2 && has_block_function_before_last(&call.arguments) {
        let arg_parts = doc::join_doc(
            call.arguments
                .iter()
                .map(|arg| printer.build_expression_doc(arg)),
            doc::comma_hardline(),
        );
        return wrap_call_with_hard_breaks(callee, arg_parts);
    }

    // "Expand last arg" pattern (Prettier's shouldExpandLastArg):
    // When last arg is array/object and preceding args are short,
    // use different strategies based on whether last two args have same type.
    //
    // Prettier disables expand-last-arg when the last two arguments have the
    // same type (e.g., both arrays). In that case, it uses expand-all instead.
    if call.arguments.len() >= 2
        && last_arg_is_array_or_object(&call.arguments)
        && preceding_args_allow_hug(&call.arguments, printer.line_breaks)
        && !has_inter_argument_comments(call, printer)
    {
        // Check if last two args have the same "expandable" type (both arrays or both objects)
        // Prettier disables expand-last-arg in this case
        let last_two_same_type = {
            let last = &call.arguments[call.arguments.len() - 1];
            let penultimate = &call.arguments[call.arguments.len() - 2];
            matches!(
                (last, penultimate),
                (
                    internal::Expression::ArrayExpression(_),
                    internal::Expression::ArrayExpression(_)
                ) | (
                    internal::Expression::ObjectExpression(_),
                    internal::Expression::ObjectExpression(_)
                )
            )
        };

        if last_two_same_type {
            // Same type: use 2-state conditional (inline, expand-all)
            // Don't use the "hug with bracket" state
            let (head_parts, last_arg_doc, all_args_broken) =
                build_args_split_last(&call.arguments, printer);

            // If last arg has hardlines (e.g., comments), skip the conditional_group
            // and go directly to expand-all (state 1 would be selected incorrectly)
            if doc::will_break(&last_arg_doc) {
                return build_expand_all_args(callee, all_args_broken);
            }

            return build_inline_or_expand_all(callee, head_parts, last_arg_doc, all_args_broken);
        }

        // Different types: check if last arg has hardlines (e.g., comments)
        // If it does, Prettier uses expand-all instead of hug
        let (head_parts, last_arg_doc, all_args_broken) =
            build_args_split_last(&call.arguments, printer);

        // If last arg will break (has hardlines), use expand-all
        if doc::will_break(&last_arg_doc) {
            return build_inline_or_expand_all(callee, head_parts, last_arg_doc, all_args_broken);
        }

        // No hardlines: use the original "hug" behavior
        // The array/object has its own group that decides whether to expand
        // e.g., fn('x', [Long1, Long2]) → fn('x', [\n\tLong1,\n\tLong2,\n])
        return doc::group(doc::concat(vec![
            callee,
            doc::text("("),
            doc::concat(head_parts),
            last_arg_doc,
            doc::text(")"),
        ]));
    }

    // Check for blank lines between arguments (forces expansion and preservation)
    let has_blank_lines = call.arguments.windows(2).any(|window| {
        let prev_end = window[0].span().end;
        let curr_start = window[1].span().start;
        printer.has_blank_line_between(prev_end, curr_start)
    });

    if has_blank_lines {
        // Build arguments with blank line preservation (forced expansion)
        let mut arg_parts = Vec::new();
        for (i, arg) in call.arguments.iter().enumerate() {
            // Check for blank line before this arg
            if i > 0 {
                let prev_end = call.arguments[i - 1].span().end;
                let curr_start = arg.span().start;
                if printer.has_blank_line_between(prev_end, curr_start) {
                    arg_parts.push(doc::literalline());
                    arg_parts.push(doc::hardline());
                }
            }

            arg_parts.push(printer.build_expression_doc(arg));

            // Add comma+hardline after each arg except the last
            if i < call.arguments.len() - 1 {
                arg_parts.push(doc::text(","));

                // Check if next arg has blank line before it
                // If not, add hardline here
                let next_start = call.arguments[i + 1].span().start;
                let curr_end = arg.span().end;
                if !printer.has_blank_line_between(curr_end, next_start) {
                    arg_parts.push(doc::hardline());
                }
            }
        }

        let arg_doc = doc::concat(arg_parts);
        return wrap_call_with_hard_breaks(callee, arg_doc);
    }

    // Build args with line separators (one per line when broken)
    // Boolean() calls don't get extra indent on binary continuation lines
    let use_arg_indent = !is_boolean_call(call, printer);
    let arg_parts = doc::join_doc(
        call.arguments.iter().map(|arg| {
            if use_arg_indent {
                printer.build_arg_expression_doc(arg)
            } else {
                printer.build_expression_doc(arg)
            }
        }),
        doc::comma_line(),
    );

    // Wrap in group with parens
    wrap_call_with_soft_breaks(callee, arg_parts)
}
