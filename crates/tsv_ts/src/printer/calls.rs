// Call and member expression printing for TypeScript
//
// Handles printing of:
// - Call expressions: `foo()`, `obj.method(arg1, arg2)`
// - Member expressions: `obj.prop`, `arr[0]`
// - Method chains: `arr.filter().map()`
// - Conditional expressions: `a ? b : c`
// - Test function calls: `it()`, `test.skip()`, `describe()`, etc.

use super::chain::{self, ChainPrinter, SymbolLookup};
use super::{ParenContext, Printer, has_multiline_content, needs_parens};
use crate::ast::internal;
use string_interner::DefaultSymbol;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};

/// Check if a chain expression contains any call expressions
fn chain_has_calls(expr: &internal::Expression) -> bool {
    match expr {
        internal::Expression::CallExpression(_) => true,
        internal::Expression::MemberExpression(member) => chain_has_calls(&member.object),
        internal::Expression::TSNonNullExpression(non_null) => {
            chain_has_calls(&non_null.expression)
        }
        _ => false,
    }
}
/// Check if an argument is a "short" simple value that won't expand
/// Used to determine if tail args can stay inline after a function callback
fn is_hopefully_short_arg(expr: &internal::Expression) -> bool {
    match expr {
        // Simple literals are always short
        internal::Expression::Literal(_) => true,
        // Identifiers are short
        internal::Expression::Identifier(_) => true,
        // Simple member accesses like obj.prop are short
        internal::Expression::MemberExpression(member) => {
            !member.computed && is_hopefully_short_arg(&member.object)
        }
        // Unary expressions with short operands
        internal::Expression::UnaryExpression(unary) => is_hopefully_short_arg(&unary.argument),
        // Empty arrays/objects are short
        internal::Expression::ArrayExpression(arr) => arr.elements.is_empty(),
        internal::Expression::ObjectExpression(obj) => obj.properties.is_empty(),
        // Everything else might be complex
        _ => false,
    }
}

/// Check if an expression is an object that could expand (has properties)
/// Used for "expand last arg" pattern in import expressions
fn is_expandable_object(expr: &internal::Expression) -> bool {
    matches!(expr, internal::Expression::ObjectExpression(obj) if !obj.properties.is_empty())
}

/// Test function patterns that Prettier keeps on a single line
/// Includes: Jest, Mocha, Jasmine, Playwright, Vitest patterns
const TEST_CALL_PATTERNS: &[&str] = &[
    // Core test functions
    "it",
    "it.only",
    "it.skip",
    "describe",
    "describe.only",
    "describe.skip",
    "test",
    "test.only",
    "test.skip",
    "test.fixme", // Playwright 3.7
    "test.step",
    // Playwright describe variants
    "test.describe",
    "test.describe.only",
    "test.describe.skip",
    "test.describe.fixme", // Playwright 3.7
    "test.describe.parallel",
    "test.describe.parallel.only",
    "test.describe.serial",
    "test.describe.serial.only",
    // Focus/skip prefixes
    "skip",
    "xit",
    "xdescribe",
    "xtest",
    "fit",
    "fdescribe",
    "ftest",
];

/// Get the name of an identifier if it's a simple identifier
fn get_identifier_name(expr: &internal::Expression) -> Option<DefaultSymbol> {
    if let internal::Expression::Identifier(id) = expr {
        Some(id.name)
    } else {
        None
    }
}

/// Get the member chain parts from an expression
/// Returns (parts_reversed, is_optional) where parts_reversed is e.g. ["skip", "test"]
fn get_member_chain_parts(expr: &internal::Expression) -> Option<Vec<DefaultSymbol>> {
    let mut parts = Vec::new();

    match expr {
        internal::Expression::Identifier(id) => {
            parts.push(id.name);
            Some(parts)
        }
        internal::Expression::MemberExpression(member) => {
            // Don't match computed or optional chains (a[b] or a?.b)
            if member.computed || member.optional {
                return None;
            }

            // Get property name
            let prop_name = get_identifier_name(&member.property)?;
            parts.push(prop_name);

            // Recursively get object parts
            let mut object_parts = get_member_chain_parts(&member.object)?;
            parts.append(&mut object_parts);

            Some(parts)
        }
        _ => None,
    }
}

/// Check if a call is a module path call that should not break at arguments.
/// These calls keep the module path on the same line as the method.
///
/// Patterns:
/// - `require(string)` → don't break args, stay on one line
/// - `require.resolve(string)` → don't break args, let assignment break
fn is_module_path_no_break(call: &internal::CallExpression, printer: &Printer) -> bool {
    // Must have exactly 1 argument that is a string literal
    if call.arguments.len() != 1 {
        return false;
    }
    let is_string_arg = matches!(
        &call.arguments[0],
        internal::Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. })
    );
    if !is_string_arg {
        return false;
    }

    // Check for `require()`
    if let internal::Expression::Identifier(id) = call.callee.as_ref()
        && printer.resolve_symbol(id.name) == "require"
    {
        return true;
    }

    // Check for `require.resolve()`
    if let internal::Expression::MemberExpression(member) = call.callee.as_ref()
        && !member.computed
        && !member.optional
        && let internal::Expression::Identifier(resolve_id) = member.property.as_ref()
        && printer.resolve_symbol(resolve_id.name) == "resolve"
        && let internal::Expression::Identifier(require_id) = member.object.as_ref()
        && printer.resolve_symbol(require_id.name) == "require"
    {
        return true;
    }

    false
}

/// Module path call patterns where prettier breaks at the chain rather than at args.
/// Returns (base_expr, method_name) if this is a module path call that should break at chain.
///
/// Patterns:
/// - `require.resolve.paths(string)` → break before `.paths`
/// - `import.meta.resolve(string)` → break before `.resolve`
fn get_module_path_chain_break<'a>(
    call: &'a internal::CallExpression,
    printer: &Printer,
) -> Option<(&'a internal::Expression, &'a internal::Identifier)> {
    // Must have exactly 1 argument that is a string literal
    if call.arguments.len() != 1 {
        return None;
    }
    let is_string_arg = matches!(
        &call.arguments[0],
        internal::Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. })
    );
    if !is_string_arg {
        return None;
    }

    // Callee must be a member expression (not computed, not optional)
    let internal::Expression::MemberExpression(member) = call.callee.as_ref() else {
        return None;
    };
    if member.computed || member.optional {
        return None;
    }

    // Property must be an identifier
    let internal::Expression::Identifier(method_name) = member.property.as_ref() else {
        return None;
    };

    let method_str = printer.resolve_symbol(method_name.name);

    // Check for `require.resolve.paths()`
    if method_str == "paths" {
        // Object should be `require.resolve`
        if let internal::Expression::MemberExpression(obj_member) = member.object.as_ref()
            && !obj_member.computed
            && !obj_member.optional
            && let internal::Expression::Identifier(resolve_id) = obj_member.property.as_ref()
            && printer.resolve_symbol(resolve_id.name) == "resolve"
            && let internal::Expression::Identifier(require_id) = obj_member.object.as_ref()
            && printer.resolve_symbol(require_id.name) == "require"
        {
            return Some((&member.object, method_name));
        }
    }

    // Check for `import.meta.resolve()`
    if method_str == "resolve"
        && let internal::Expression::MetaProperty(meta) = member.object.as_ref()
    {
        let meta_name = printer.resolve_symbol(meta.meta.name);
        let prop_name = printer.resolve_symbol(meta.property.name);
        if meta_name == "import" && prop_name == "meta" {
            return Some((&member.object, method_name));
        }
    }

    None
}

/// Check if a call expression is a test function call that should stay on one line
fn is_test_call(call: &internal::CallExpression, printer: &Printer) -> bool {
    // Must have 2-3 arguments
    let arg_count = call.arguments.len();
    if !(2..=3).contains(&arg_count) {
        return false;
    }

    // First argument must be a string or template literal
    let first_is_string = match &call.arguments[0] {
        internal::Expression::Literal(lit) => {
            matches!(lit.value, internal::LiteralValue::String { .. })
        }
        internal::Expression::TemplateLiteral(_) => true,
        _ => false,
    };
    if !first_is_string {
        return false;
    }

    // Second argument must be a function expression (arrow or regular)
    let second_is_function = matches!(
        &call.arguments[1],
        internal::Expression::ArrowFunctionExpression(_)
            | internal::Expression::FunctionExpression(_)
    );
    if !second_is_function {
        return false;
    }

    // Third argument (if present) must be a number (timeout)
    if arg_count == 3 {
        let third_is_number = match &call.arguments[2] {
            internal::Expression::Literal(lit) => {
                matches!(lit.value, internal::LiteralValue::Number(_))
            }
            _ => false,
        };
        if !third_is_number {
            return false;
        }
    }

    // Check if callee matches a test pattern
    let Some(parts) = get_member_chain_parts(&call.callee) else {
        return false;
    };

    // Build the callee string in correct order (parts are reversed)
    let callee_str: String = parts
        .iter()
        .rev()
        .map(|sym| printer.resolve_symbol(*sym))
        .collect::<Vec<_>>()
        .join(".");

    // Check against known test patterns
    TEST_CALL_PATTERNS.contains(&callee_str.as_str())
}

impl<'a> Printer<'a> {
    /// Check if call should use "expand first arg" pattern
    ///
    /// This matches prettier's behavior for calls like `setTimeout(() => {...}, 100)`:
    /// - First arg is function/arrow with block body
    /// - Remaining args are "hopefully short" (simple values)
    /// - Result: first arg expands, tail args stay inline after closing `}`
    fn should_expand_first_arg(&self, args: &[internal::Expression]) -> bool {
        // Need at least 2 args (first is function, rest are short)
        if args.len() != 2 {
            return false;
        }

        let first_arg = &args[0];
        let second_arg = &args[1];

        // First arg must be a function with block body
        let first_is_expandable_function = match first_arg {
            internal::Expression::ArrowFunctionExpression(arrow) => {
                matches!(&arrow.body, internal::ArrowFunctionBody::BlockStatement(_))
            }
            internal::Expression::FunctionExpression(_) => true,
            _ => false,
        };

        if !first_is_expandable_function {
            return false;
        }

        // Second arg must be short/simple (won't expand)
        // Also, second arg shouldn't be another function (would look odd)
        if matches!(
            second_arg,
            internal::Expression::ArrowFunctionExpression(_)
                | internal::Expression::FunctionExpression(_)
                | internal::Expression::ConditionalExpression(_)
        ) {
            return false;
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
    ///
    /// Build a Doc for a call expression with argument wrapping (not chain-aware)
    pub(super) fn build_call_doc_with_wrapping(&self, call: &internal::CallExpression) -> Doc {
        let callee_doc = self.build_expression_doc(&call.callee);

        // Wrap callee in parens if needed (e.g., ternary: `(a ? b : c)()`)
        let callee = if needs_parens(&call.callee, ParenContext::Callee) {
            doc::parens(callee_doc)
        } else {
            callee_doc
        };

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
            .map(|ta| self.build_type_parameter_instantiation_doc(ta));

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

            let has_line_comments = self.has_line_comments_between(paren_open, arg_start);
            if has_line_comments {
                // Multi-line format: fn(\n\t// comment\n\targ,\n)
                let mut comment_parts = Vec::new();
                for comment in tsv_lang::comments_in_range(self.comments, paren_open, arg_start) {
                    comment_parts.push(self.build_comment_doc(comment));
                    comment_parts.push(doc::hardline());
                }

                return doc::concat(vec![
                    callee,
                    doc::text("("),
                    doc::indent(doc::concat(vec![
                        doc::hardline(),
                        doc::concat(comment_parts),
                        self.build_expression_doc(first_arg),
                        doc::text(","),
                    ])),
                    doc::hardline(),
                    doc::text(")"),
                ]);
            }

            // Check for inline block comments
            let has_inline_comments = self.has_comments_between(paren_open, arg_start);
            if has_inline_comments {
                return doc::concat(vec![
                    callee,
                    doc::text("("),
                    self.build_inline_comments_between_doc_no_leading_space(paren_open, arg_start),
                    doc::text(" "),
                    self.build_expression_doc(first_arg),
                    // Check for trailing comments
                    self.build_inline_comments_between_doc(arg_end, paren_close),
                    doc::text(")"),
                ]);
            }
        }

        // Test function calls (it, test, describe, etc.) stay on one line
        // even if they exceed print width
        if is_test_call(call, self) {
            // Build callee as flat string (no conditionalGroup)
            // This prevents breaking at `.skip` etc. even when very long
            let flat_callee = if let Some(parts) = get_member_chain_parts(&call.callee) {
                let callee_str: String = parts
                    .iter()
                    .rev()
                    .map(|sym| self.resolve_symbol(*sym))
                    .collect::<Vec<_>>()
                    .join(".");
                doc::text_owned(callee_str)
            } else {
                callee
            };

            let arg_docs: Vec<_> = call
                .arguments
                .iter()
                .map(|arg| self.build_expression_doc(arg))
                .collect();
            return doc::concat(vec![
                flat_callee,
                doc::text("("),
                doc::join(arg_docs, ", "),
                doc::text(")"),
            ]);
        }

        // Module path calls that should not break at arguments (e.g., require.resolve)
        // Keep the call on one line; let assignment/parent break instead
        if is_module_path_no_break(call, self) {
            let arg_docs: Vec<_> = call
                .arguments
                .iter()
                .map(|arg| self.build_expression_doc(arg))
                .collect();
            return doc::concat(vec![
                callee,
                doc::text("("),
                doc::join(arg_docs, ", "),
                doc::text(")"),
            ]);
        }

        // Module path calls (require.resolve.paths, import.meta.resolve) break at chain
        // rather than at arguments, keeping the path on the same line as the method
        if let Some((base_expr, method_name)) = get_module_path_chain_break(call, self) {
            let base_doc = self.build_expression_doc(base_expr);
            let method_str = self.resolve_symbol(method_name.name);
            let arg_doc = self.build_expression_doc(&call.arguments[0]);

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
        if call.arguments.len() == 1 {
            match &call.arguments[0] {
                // Block arrow: check if first line fits to decide hugging vs wrapping
                internal::Expression::ArrowFunctionExpression(arrow)
                    if !arrow.body.is_expression() =>
                {
                    // Estimate first line length: callee + "(" + signature + "{"
                    // sig_length spans from arrow start to block start (includes params and " => ")
                    let sig_end = match &arrow.body {
                        internal::ArrowFunctionBody::BlockStatement(block) => block.span.start,
                        _ => arrow.span.end,
                    };
                    let sig_length = (sig_end - arrow.span.start) as usize;
                    let total_first_line = 5 + 1 + sig_length + 1; // ~callee + "(" + sig + "{"

                    if total_first_line + 4 < self.config.print_width {
                        // Fits - keep hugged
                        return doc::concat(vec![
                            callee,
                            doc::text("("),
                            self.build_expression_doc(&call.arguments[0]),
                            doc::text(")"),
                        ]);
                    }
                    // Doesn't fit - fall through to wrap
                }

                // Regular function expression: keep hugged (block body handles own formatting)
                internal::Expression::FunctionExpression(_) => {
                    return doc::concat(vec![
                        callee,
                        doc::text("("),
                        self.build_expression_doc(&call.arguments[0]),
                        doc::text(")"),
                    ]);
                }

                // Object literal: hug it (object handles its own internal wrapping)
                // e.g., @decorator({...}) or fn({prop: value})
                internal::Expression::ObjectExpression(_) => {
                    return doc::concat(vec![
                        callee,
                        doc::text("("),
                        self.build_expression_doc(&call.arguments[0]),
                        doc::text(")"),
                    ]);
                }

                // Array literal: hug it (array handles its own internal wrapping)
                internal::Expression::ArrayExpression(_) => {
                    return doc::concat(vec![
                        callee,
                        doc::text("("),
                        self.build_expression_doc(&call.arguments[0]),
                        doc::text(")"),
                    ]);
                }

                // Expression arrow or block arrow that doesn't fit: wrap with width-aware group
                internal::Expression::ArrowFunctionExpression(_) => {}

                // Other arguments: fall through to standard handling
                _ => {}
            }

            // Wrap callback with width-aware breaking
            if matches!(
                &call.arguments[0],
                internal::Expression::ArrowFunctionExpression(_)
            ) {
                let arg_doc = self.build_expression_doc(&call.arguments[0]);
                return doc::group(doc::concat(vec![
                    callee,
                    doc::text("("),
                    doc::indent_softline(doc::concat(vec![arg_doc, doc::trailing_comma()])),
                    doc::softline(),
                    doc::text(")"),
                ]));
            }
        }

        // Check if any argument has multiline content (e.g., line continuation strings)
        // Prettier expands calls containing multiline strings (recursively)
        let has_multiline = call
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, self.source));

        if has_multiline {
            // Force expansion with hardlines for multiline content
            let arg_docs: Vec<_> = call
                .arguments
                .iter()
                .map(|arg| self.build_expression_doc(arg))
                .collect();
            let arg_parts = doc::join_doc(arg_docs, doc::comma_hardline());

            // Always expanded with trailing comma
            return doc::concat(vec![
                callee,
                doc::text("("),
                doc::indent(doc::concat(vec![
                    doc::hardline(),
                    arg_parts,
                    doc::text(","),
                ])),
                doc::hardline(),
                doc::text(")"),
            ]);
        }

        // "Expand first arg" pattern: when first arg is a function with block body
        // and remaining args are short, hug the function and put tail args after closing }
        // e.g., setTimeout(() => { tick(); }, 100);
        if self.should_expand_first_arg(&call.arguments) {
            let first_arg_doc = self.build_expression_doc(&call.arguments[0]);

            // Build tail args (everything after first)
            let mut tail_parts = Vec::new();
            for arg in call.arguments.iter().skip(1) {
                tail_parts.push(doc::text(", "));
                tail_parts.push(self.build_expression_doc(arg));
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

        // Check for leading comments before first argument in multi-arg calls
        if !call.arguments.is_empty() {
            let paren_open = call.callee.span().end;
            let first_arg_start = call.arguments[0].span().start;
            let has_leading_comments = self.has_comments_between(paren_open, first_arg_start);

            if has_leading_comments {
                let mut arg_docs: Vec<_> = call
                    .arguments
                    .iter()
                    .map(|arg| self.build_expression_doc(arg))
                    .collect();

                // Build first arg with leading comments
                let first_with_comments = doc::concat(vec![
                    self.build_inline_comments_between_doc_no_leading_space(
                        paren_open,
                        first_arg_start,
                    ),
                    doc::text(" "),
                    arg_docs.remove(0),
                ]);

                let mut all_args = vec![first_with_comments];
                all_args.extend(arg_docs);
                let arg_parts = doc::join_doc(all_args, doc::comma_line());

                return doc::group(doc::concat(vec![
                    callee,
                    doc::text("("),
                    doc::indent_softline(doc::concat(vec![arg_parts, doc::trailing_comma()])),
                    doc::softline(),
                    doc::text(")"),
                ]));
            }
        }

        // Build args with line separators (one per line when broken)
        let arg_docs: Vec<_> = call
            .arguments
            .iter()
            .map(|arg| self.build_expression_doc(arg))
            .collect();
        let arg_parts = doc::join_doc(arg_docs, doc::comma_line());

        // Wrap in group with parens
        doc::group(doc::concat(vec![
            callee,
            doc::text("("),
            doc::indent_softline(doc::concat(vec![arg_parts, doc::trailing_comma()])),
            doc::softline(),
            doc::text(")"),
        ]))
    }

    /// Build a Doc for a chain (method chain or member chain) with wrapping
    ///
    /// Uses the chain module's grouping and doc building logic for proper
    /// member chain formatting, including the 3+ calls rule.
    fn build_chain_doc_with_wrapping(&self, expr: &internal::Expression) -> Doc {
        let nodes = chain::linearize_chain(expr);
        let groups = chain::group_chain_nodes(nodes);
        chain::build_chain_doc(&groups, self)
    }

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
                // Block arrow function: check if first line fits before hugging
                internal::Expression::ArrowFunctionExpression(arrow)
                    if !arrow.body.is_expression() =>
                {
                    // Estimate first line length: "new " + callee + "(" + signature + "{"
                    let sig_end = match &arrow.body {
                        internal::ArrowFunctionBody::BlockStatement(block) => block.span.start,
                        _ => arrow.span.end,
                    };
                    let sig_length = (sig_end - arrow.span.start) as usize;
                    let callee_length = (new_expr.callee.span().end - new_expr.callee.span().start)
                        as usize
                        + new_expr
                            .type_arguments
                            .as_ref()
                            .map_or(0, |ta| (ta.span.end - ta.span.start) as usize);
                    // Add buffer for potential indentation (base_indent_offset + 1 indent level)
                    let indent_buffer =
                        (self.config.base_indent_offset + 1) * self.config.tab_width;
                    let total_first_line = indent_buffer + 4 + callee_length + 1 + sig_length + 1; // indent + "new " + callee + "(" + sig + "{"

                    if total_first_line < self.config.print_width {
                        // Fits - keep hugged
                        return doc::concat(vec![
                            callee_with_types,
                            doc::text("("),
                            self.build_expression_doc(&new_expr.arguments[0]),
                            doc::text(")"),
                        ]);
                    }
                    // Doesn't fit - fall through to wrap
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

            return doc::concat(vec![
                callee_with_types,
                doc::text("("),
                doc::indent(doc::concat(vec![
                    doc::hardline(),
                    arg_parts,
                    doc::text(","),
                ])),
                doc::hardline(),
                doc::text(")"),
            ]);
        }

        // Build args with line separators (one per line when broken)
        let arg_docs: Vec<_> = new_expr
            .arguments
            .iter()
            .map(|arg| self.build_expression_doc(arg))
            .collect();
        let arg_parts = doc::join_doc(arg_docs, doc::comma_line());

        // Wrap in group with parens
        doc::group(doc::concat(vec![
            callee_with_types,
            doc::text("("),
            doc::indent_softline(doc::concat(vec![arg_parts, doc::trailing_comma()])),
            doc::softline(),
            doc::text(")"),
        ]))
    }

    /// Build a Doc for a new expression (for nested contexts)
    pub(super) fn build_new_doc(&self, new_expr: &internal::NewExpression) -> Doc {
        self.build_new_doc_with_wrapping(new_expr)
    }

    /// Build a Doc for a conditional expression with wrapping support
    pub(super) fn build_conditional_doc_with_wrapping(
        &self,
        cond: &internal::ConditionalExpression,
    ) -> Doc {
        let test = self.build_expression_doc(&cond.test);
        let consequent = self.build_expression_doc(&cond.consequent);
        let alternate = self.build_expression_doc(&cond.alternate);

        // Check for comments between test and ? (before consequent)
        let test_end = cond.test.span().end;
        let consequent_start = cond.consequent.span().start;
        let comments_after_test =
            self.build_inline_comments_between_doc(test_end, consequent_start);

        // Wrap expressions that need parens in ternary branches:
        // - Nested conditionals avoid right-associativity confusion: `a ? (b ? c : d) : e`
        // - `as`/`satisfies` avoid `:` ambiguity: `a ? (b as T) : c` (not `a ? b as T : c`)
        let needs_parens = |expr: &internal::Expression| {
            matches!(
                expr,
                internal::Expression::ConditionalExpression(_)
                    | internal::Expression::TSAsExpression(_)
                    | internal::Expression::TSSatisfiesExpression(_)
            )
        };

        let consequent = if needs_parens(&cond.consequent) {
            doc::parens(consequent)
        } else {
            consequent
        };

        let alternate = if needs_parens(&cond.alternate) {
            doc::parens(alternate)
        } else {
            alternate
        };

        // When flat: `test ? consequent : alternate`
        // When broken: `test\n\t? consequent\n\t: alternate`
        doc::group(doc::concat(vec![
            test,
            comments_after_test,
            doc::indent(doc::concat(vec![
                doc::line(),
                doc::text("? "),
                consequent,
                doc::line(),
                doc::text(": "),
                alternate,
            ])),
        ]))
    }

    /// Build a Doc for a call expression (for nested contexts)
    ///
    /// Checks for chain patterns (like `a().b().c()`) and uses chain module
    /// for proper member chain formatting.
    pub(super) fn build_call_doc(&self, call: &internal::CallExpression) -> Doc {
        // Check if this is a true chain (callee contains calls, like `a().b()`)
        // vs a simple method call (callee is just member access, like `obj.method()`)
        let is_true_chain = chain_has_calls(&call.callee);

        if is_true_chain {
            // True chain like `arr.filter().map()` - use chain wrapping
            self.build_chain_doc_with_wrapping(&internal::Expression::CallExpression(call.clone()))
        } else {
            // Simple call or simple method call - wrap args, keep callee together
            self.build_call_doc_with_wrapping(call)
        }
    }

    /// Build a Doc for a member expression with optional breaking at dots
    ///
    /// Uses the new chain architecture based on prettier's member-chain.js:
    /// 1. Linearize AST into flat list of chain nodes
    /// 2. Group nodes by natural break points
    /// 3. Build doc with conditionalGroup for oneLine/expanded alternatives
    pub(super) fn build_member_doc(&self, member: &internal::MemberExpression) -> Doc {
        // Use new chain-based implementation
        let expr = internal::Expression::MemberExpression(member.clone());
        let nodes = chain::linearize_chain(&expr);
        let groups = chain::group_chain_nodes(nodes);
        chain::build_chain_doc(&groups, self)
    }

    // =========================================================================
    // Import Expression
    // =========================================================================

    /// Build a Doc for a dynamic import expression: `import('module')` or `import('module', options)`
    ///
    /// Uses "expand last arg" pattern when options is an object:
    /// - First arg stays on same line as `import(`
    /// - Only the options object expands with its properties indented
    pub(super) fn build_import_expression_doc(
        &self,
        import_expr: &internal::ImportExpression,
    ) -> Doc {
        let source_doc = self.build_expression_doc(&import_expr.source);

        // If no options, simple case
        let Some(options) = &import_expr.options else {
            return doc::concat(vec![doc::text("import"), doc::parens(source_doc)]);
        };

        let options_doc = self.build_expression_doc(options);

        // "Expand last arg" pattern for objects: keep first arg inline, only expand the object
        // Result: import(source, {\n\twith: {...},\n})
        if is_expandable_object(options) {
            doc::concat(vec![
                doc::text("import"),
                doc::text("("),
                source_doc,
                doc::text(", "),
                options_doc,
                doc::text(")"),
            ])
        } else {
            // Standard wrapping for non-object options
            let arg_parts = doc::join_doc(vec![source_doc, options_doc], doc::comma_line());
            doc::group(doc::concat(vec![
                doc::text("import"),
                doc::text("("),
                doc::indent_softline(arg_parts),
                doc::softline(),
                doc::text(")"),
            ]))
        }
    }

    /// Build a Doc for a meta property: `import.meta`, `new.target`
    pub(super) fn build_meta_property_doc(&self, meta: &internal::MetaProperty) -> Doc {
        let meta_name = self.resolve_symbol(meta.meta.name);
        let prop_name = self.resolve_symbol(meta.property.name);
        doc::text_owned(format!("{meta_name}.{prop_name}"))
    }

    /// Build a Doc for call arguments only (for chain printing)
    fn build_call_args_doc_for_chain(
        &self,
        call: &internal::CallExpression,
        optional: bool,
    ) -> Doc {
        // Build type arguments if present: `<T, U>`
        let type_args_doc = call
            .type_arguments
            .as_ref()
            .map(|ta| self.build_type_parameter_instantiation_doc(ta));

        // Build the argument list using the existing helper
        let args: Vec<Doc> = call
            .arguments
            .iter()
            .map(|arg| self.build_expression_doc(arg))
            .collect();

        let prefix = if optional { "?.(" } else { "(" };

        let mut parts = Vec::new();
        if let Some(ta_doc) = type_args_doc {
            parts.push(ta_doc);
        }

        if args.is_empty() {
            parts.push(doc::text_owned(format!("{prefix})")));
            doc::concat(parts)
        } else {
            // For simplicity, just join with comma+space
            // Full argument formatting with breaks is handled elsewhere
            parts.push(doc::text(prefix));
            parts.push(doc::join(args, ", "));
            parts.push(doc::text(")"));
            doc::concat(parts)
        }
    }
}

// =============================================================================
// ChainPrinter trait implementation for Printer
// =============================================================================

impl<'a> SymbolLookup for Printer<'a> {
    fn lookup(&self, symbol: DefaultSymbol) -> Option<String> {
        self.interner
            .borrow()
            .resolve(symbol)
            .map(ToString::to_string)
    }
}

impl<'a> ChainPrinter for Printer<'a> {
    fn print_expression(&self, expr: &internal::Expression) -> Doc {
        self.build_expression_doc(expr)
    }

    fn print_parenthesized_base(&self, expr: &internal::Expression) -> Doc {
        // Build the inner expression without a group wrapper
        // This ensures LINE elements are at the right level for the chain's outer group
        let inner = match expr {
            internal::Expression::BinaryExpression(binary) => {
                // Use the no-group version so LINE elements are at outer level
                self.build_binary_chain_parts_indented(binary)
            }
            _ => self.build_expression_doc(expr),
        };
        doc::parens(inner)
    }

    fn print_call_args(&self, call: &internal::CallExpression, optional: bool) -> Doc {
        self.build_call_args_doc_for_chain(call, optional)
    }

    fn lookup_symbol(&self, symbol: DefaultSymbol) -> String {
        self.interner
            .borrow()
            .resolve(symbol)
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    fn build_block_comments_doc(
        &self,
        start: u32,
        end: u32,
        spacing: super::CommentSpacing,
    ) -> Doc {
        self.build_comments_between_filtered(start, end, spacing, super::CommentFilter::BlockOnly)
    }

    fn get_property_span(&self, expr: &internal::Expression) -> tsv_lang::Span {
        expr.span()
    }
}
