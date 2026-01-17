// Call and member expression printing for TypeScript
//
// Handles printing of:
// - Call expressions: `foo()`, `obj.method(arg1, arg2)`
// - Member expressions: `obj.prop`, `arr[0]`
// - Method chains: `arr.filter().map()`
// - Test function calls: `it()`, `test.skip()`, `describe()`, etc.
// - Import expressions: `import('module')`, `import('module', options)`

use super::chain::{self, ChainPrinter, SymbolLookup};
use super::utils::{
    could_expand_arrow_body, has_multiple_function_args, is_expandable_object,
    is_hopefully_short_arg, last_arg_is_array_or_object, preceding_args_are_short,
};
use super::{ParenContext, Printer, has_multiline_content, needs_parens, template_literal_has_newlines};
use crate::ast::internal;
use string_interner::DefaultSymbol;
use tsv_lang::SymbolResolver;
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::has_blank_line_between;

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

/// Check if this is a `Boolean(...)` call
///
/// Prettier doesn't add continuation indent for binary expressions inside Boolean() calls.
/// This appears to be a specific quirk, treating Boolean() like !!() for type coercion.
fn is_boolean_call(call: &internal::CallExpression, printer: &Printer) -> bool {
    if let internal::Expression::Identifier(id) = call.callee.as_ref() {
        return printer.resolve_symbol(id.name) == "Boolean";
    }
    false
}

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

/// Check if a call expression has comments between any of its arguments
fn has_inter_argument_comments(call: &internal::CallExpression, printer: &Printer) -> bool {
    has_inter_argument_comments_slice(&call.arguments, printer)
}

/// Check if there are comments between arguments in a slice
pub(super) fn has_inter_argument_comments_slice(
    arguments: &[internal::Expression],
    printer: &Printer,
) -> bool {
    if arguments.len() < 2 {
        return false;
    }

    for i in 0..arguments.len() - 1 {
        let arg_end = arguments[i].span().end;
        let next_arg_start = arguments[i + 1].span().start;
        if printer.has_comments_between(arg_end, next_arg_start) {
            return true;
        }
    }

    false
}

/// Check if comments between two positions should force multi-line layout
/// Returns true if there are line comments OR block comments on their own line
#[inline]
fn should_force_expansion_for_comments(printer: &Printer, start: u32, end: u32) -> bool {
    printer.has_line_comments_between(start, end) || printer.has_newline_before_comment(start, end)
}

/// Check if there are trailing line comments on any arguments
///
/// A trailing comment is one that appears after an argument's expression,
/// either between the arg and its comma, or between the last arg and the closing paren.
/// Example: `fn(a && b, // trailing)` - the `// trailing` is a trailing comment on `a && b`
fn has_trailing_comments_on_args(call: &internal::CallExpression, printer: &Printer) -> bool {
    if call.arguments.is_empty() {
        return false;
    }

    for (i, arg) in call.arguments.iter().enumerate() {
        let arg_end = arg.span().end;
        let next_boundary = if i < call.arguments.len() - 1 {
            call.arguments[i + 1].span().start
        } else {
            call.span.end
        };

        if printer.has_line_comments_between(arg_end, next_boundary) {
            return true;
        }
    }

    false
}

/// Partitioned comments between two positions
///
/// Separates comments into categories based on position relative to `reference_pos`:
/// - `trailing_line`: Line comments on the same line as reference_pos
/// - `trailing_block`: Block comments on the same line as reference_pos
/// - `leading`: Comments on their own lines (not on same line as reference_pos)
struct PartitionedComments<'a> {
    trailing_line: Vec<&'a internal::Comment>,
    trailing_block: Vec<&'a internal::Comment>,
    leading: Vec<&'a internal::Comment>,
}

impl<'a> PartitionedComments<'a> {
    /// Partition comments in a range based on their position relative to `start`
    ///
    /// Comments on the same line as `start` are "trailing" (they follow content on that line).
    /// Comments on subsequent lines are "leading" (they precede content on the next line).
    fn new(comments: &'a [internal::Comment], source: &str, start: u32, end: u32) -> Self {
        let mut trailing_line = Vec::new();
        let mut trailing_block = Vec::new();
        let mut leading = Vec::new();

        for comment in tsv_lang::comments_in_range(comments, start, end) {
            if tsv_lang::printing::is_same_line(source, start, comment.span.start) {
                if comment.is_block {
                    trailing_block.push(comment);
                } else {
                    trailing_line.push(comment);
                }
            } else {
                leading.push(comment);
            }
        }

        Self {
            trailing_line,
            trailing_block,
            leading,
        }
    }

    fn has_trailing_line(&self) -> bool {
        !self.trailing_line.is_empty()
    }

    fn has_trailing_block(&self) -> bool {
        !self.trailing_block.is_empty()
    }
}

/// Build argument docs split into head parts (with commas), last arg, and broken form
///
/// Used for patterns that keep short args inline with the last arg.
/// Returns (head_parts, last_arg_doc, all_args_broken) where:
/// - head_parts: all but last arg with ", " separators
/// - last_arg_doc: the last argument doc
/// - all_args_broken: all args joined with comma_line() for fallback
pub(super) fn build_args_split_last(
    arguments: &[internal::Expression],
    printer: &Printer,
) -> (Vec<Doc>, Doc, Doc) {
    // Build all args
    let arg_docs: Vec<_> = arguments
        .iter()
        .map(|arg| printer.build_expression_doc(arg))
        .collect();

    // Build head docs (all but last) with commas
    let mut head_parts = Vec::new();
    for doc in arg_docs.iter().take(arg_docs.len() - 1) {
        head_parts.push(doc.clone());
        head_parts.push(doc::text(", "));
    }
    let last_arg_doc = arg_docs[arg_docs.len() - 1].clone();
    let all_args_broken = doc::join_doc(arg_docs, doc::comma_line());

    (head_parts, last_arg_doc, all_args_broken)
}

/// Break style for call expression wrapping
enum CallBreakStyle {
    /// Soft breaks (can collapse to single line if it fits)
    Soft,
    /// Hard breaks (always multiline)
    Hard,
}

/// Wrap arguments in a call expression: `callee(args)`
///
/// With `Soft` breaks: `callee(args)` can collapse to a single line if it fits
/// With `Hard` breaks: Always uses multiline layout `callee(\n\targs,\n)`
#[inline]
fn wrap_call(callee: Doc, args: Doc, style: CallBreakStyle) -> Doc {
    match style {
        CallBreakStyle::Soft => doc::group(doc::concat(vec![
            callee,
            doc::text("("),
            doc::indent_softline(doc::concat(vec![args, doc::trailing_comma()])),
            doc::softline(),
            doc::text(")"),
        ])),
        CallBreakStyle::Hard => doc::concat(vec![
            callee,
            doc::text("("),
            doc::indent(doc::concat(vec![doc::hardline(), args, doc::text(",")])),
            doc::hardline(),
            doc::text(")"),
        ]),
    }
}

/// Wrap arguments in a groupable call expression: `callee(args)`
/// Uses soft breaks so the call can collapse to a single line if it fits
#[inline]
pub(super) fn wrap_call_with_soft_breaks(callee: Doc, args: Doc) -> Doc {
    wrap_call(callee, args, CallBreakStyle::Soft)
}

/// Wrap arguments in an expanded call expression: `callee(\n\targs,\n)`
/// Uses hard breaks to force multi-line layout
#[inline]
pub(super) fn wrap_call_with_hard_breaks(callee: Doc, args: Doc) -> Doc {
    wrap_call(callee, args, CallBreakStyle::Hard)
}

/// Check if a single argument needs soft-break wrapping (not huggable)
///
/// Call expressions, member expressions, new expressions, and identifiers should
/// allow breaking after "(" so the outer call can break before the inner expression.
/// Objects and arrays are "huggable" and don't need soft wrapping.
fn arg_needs_soft_wrap(arg: &internal::Expression) -> bool {
    matches!(
        arg,
        internal::Expression::CallExpression(_)
            | internal::Expression::MemberExpression(_)
            | internal::Expression::NewExpression(_)
            | internal::Expression::Identifier(_)
    )
}

/// Wrap arguments with soft breaks (no callee, just prefix like "(" or "?.(")
///
/// Used in chain context where the callee is handled separately.
/// Structure: `prefix + softline + args + trailing_comma + softline + ")"`
#[inline]
fn wrap_args_with_soft_breaks(prefix: &'static str, args: Doc) -> Doc {
    doc::group(doc::concat(vec![
        doc::text(prefix),
        doc::indent_softline(doc::concat(vec![args, doc::trailing_comma()])),
        doc::softline(),
        doc::text(")"),
    ]))
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

                let arg_doc = doc::concat(vec![
                    doc::concat(comment_parts),
                    self.build_expression_doc(first_arg),
                ]);

                return wrap_call_with_hard_breaks(callee, arg_doc);
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
            let arg = &call.arguments[0];

            // Non-huggable arguments: use soft-break wrapping so outer call can break first
            // (call expressions, member expressions, new expressions, identifiers)
            if arg_needs_soft_wrap(arg) {
                let arg_doc = self.build_expression_doc(arg);
                return wrap_call_with_soft_breaks(callee, arg_doc);
            }

            match arg {
                // Block arrow: use conditional_group to let Doc decide hug vs wrap
                internal::Expression::ArrowFunctionExpression(arrow)
                    if !arrow.body.is_expression() =>
                {
                    let arrow_doc = self.build_expression_doc(arg);
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
                        self.build_expression_doc(arg),
                        doc::text(")"),
                    ]);
                }

                // Object literal: hug it (object handles its own internal wrapping)
                // e.g., @decorator({...}) or fn({prop: value})
                internal::Expression::ObjectExpression(_) => {
                    return doc::concat(vec![
                        callee,
                        doc::text("("),
                        self.build_expression_doc(arg),
                        doc::text(")"),
                    ]);
                }

                // Array literal: hug it (array handles its own internal wrapping)
                internal::Expression::ArrayExpression(_) => {
                    return doc::concat(vec![
                        callee,
                        doc::text("("),
                        self.build_expression_doc(arg),
                        doc::text(")"),
                    ]);
                }

                // Short literals (non-string or short string): hug them
                // Long string literals and multiline strings should use standard wrapping
                internal::Expression::Literal(lit) => {
                    let span_len = (lit.span.end - lit.span.start) as usize;
                    let raw = lit.span.extract(self.source);
                    let is_multiline = raw.contains('\n');
                    // Hug short, single-line literals (<=25 chars)
                    if span_len <= 25 && !is_multiline {
                        return doc::concat(vec![
                            callee,
                            doc::text("("),
                            self.build_expression_doc(arg),
                            doc::text(")"),
                        ]);
                    }
                    // Long or multiline string - fall through to standard wrapping
                }

                // Expression arrow: check special cases
                internal::Expression::ArrowFunctionExpression(arrow) => {
                    if let internal::ArrowFunctionBody::Expression(body_expr) = &arrow.body {
                        // Object literal: hug it
                        if matches!(&**body_expr, internal::Expression::ObjectExpression(_)) {
                            return doc::concat(vec![
                                callee,
                                doc::text("("),
                                self.build_expression_doc(arg),
                                doc::text(")"),
                            ]);
                        }

                        // Call expression body with complex arguments: keep signature inline
                        // Pattern: call((params) => anotherCall(...complex...)) keeps params inline
                        // Only applies when:
                        // 1. The nested call has complex arguments (blocks, objects, etc.)
                        // 2. The arrow has NO type annotations (plain signature)
                        let has_no_type_annotations = arrow.return_type.is_none()
                            && arrow.params.iter().all(|p| {
                                match p {
                                    internal::Expression::Identifier(id) => {
                                        id.type_annotation.is_none()
                                    }
                                    internal::Expression::ArrayPattern(arr) => {
                                        arr.type_annotation.is_none()
                                    }
                                    internal::Expression::ObjectPattern(obj) => {
                                        obj.type_annotation.is_none()
                                    }
                                    internal::Expression::AssignmentPattern(assign) => {
                                        // Assignment patterns wrap another pattern/identifier
                                        match assign.left.as_ref() {
                                            internal::Expression::Identifier(id) => {
                                                id.type_annotation.is_none()
                                            }
                                            internal::Expression::ArrayPattern(arr) => {
                                                arr.type_annotation.is_none()
                                            }
                                            internal::Expression::ObjectPattern(obj) => {
                                                obj.type_annotation.is_none()
                                            }
                                            _ => true,
                                        }
                                    }
                                    _ => true,
                                }
                            });

                        let is_complex_call_body =
                            if let internal::Expression::CallExpression(nested_call) = &**body_expr
                            {
                                // Check if any argument is complex (block arrow with statements, non-empty objects/arrays, etc.)
                                nested_call.arguments.iter().any(|arg| {
                                    match arg {
                                        internal::Expression::ArrowFunctionExpression(arr) => {
                                            // Only consider block arrows with statements as complex
                                            if let internal::ArrowFunctionBody::BlockStatement(
                                                block,
                                            ) = &arr.body
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
                                    .push(self.build_function_parameter_doc(&arrow.params[0]));
                            } else {
                                // Multiple params or single param with parens - inline format
                                arrow_parts.push(doc::text("("));
                                let param_docs: Vec<_> = arrow
                                    .params
                                    .iter()
                                    .map(|p| self.build_function_parameter_doc(p))
                                    .collect();
                                arrow_parts.push(doc::join(param_docs, ", "));
                                arrow_parts.push(doc::text(")"));
                            }

                            // Return type if present
                            if let Some(return_type) = &arrow.return_type {
                                arrow_parts.push(self.build_type_annotation_doc(return_type));
                            }

                            // Build custom call wrapping that keeps arrow signature inline
                            // The outer group controls whether we break the whole call
                            // The inner group around the arrow body controls whether the body breaks
                            return doc::group(doc::concat(vec![
                                callee,
                                doc::text("("),
                                doc::concat(arrow_parts),
                                doc::text(" =>"),
                                doc::group(doc::concat(vec![
                                    doc::indent_line(self.build_expression_doc(body_expr)),
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
                            let sig_doc = doc::group(self.build_arrow_signature_doc(arrow));

                            // Build body expression
                            let body_doc = self.build_expression_doc(body_expr);

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
            if matches!(
                &call.arguments[0],
                internal::Expression::ArrowFunctionExpression(_)
            ) {
                let arg_doc = self.build_expression_doc(&call.arguments[0]);
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
                let arg_doc = self.build_expression_doc(&call.arguments[0]);
                return doc::concat(vec![callee, doc::text("("), arg_doc, doc::text(")")]);
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

            return wrap_call_with_hard_breaks(callee, arg_parts);
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

        // Multiple arrow/function arguments: always break (Prettier behavior)
        // e.g., fn((a) => a, (b) => b) → fn(\n\t(a) => a,\n\t(b) => b,\n)
        if has_multiple_function_args(&call.arguments) {
            let arg_docs: Vec<_> = call
                .arguments
                .iter()
                .map(|arg| self.build_expression_doc(arg))
                .collect();
            let arg_parts = doc::join_doc(arg_docs, doc::comma_hardline());

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

            if last_is_function {
                // Skip this pattern if there are inter-argument comments
                // (they'll be handled by the general inter-argument comment handler below)
                let has_inter_arg_comments = has_inter_argument_comments(call, self);

                if preceding_args_are_short(&call.arguments) && !has_inter_arg_comments {
                    let (head_parts, last_arg_doc, all_args_broken) =
                        build_args_split_last(&call.arguments, self);

                    // Try: inline, or break all args
                    // Note: last arg contains hardlines, so state 1 only succeeds if the whole
                    // line (including arrow signature) fits within print_width
                    return doc::conditional_group(vec![
                        // State 1: Keep all args inline (arrow breaks internally due to hardlines)
                        doc::concat(vec![
                            callee.clone(),
                            doc::text("("),
                            doc::concat(head_parts),
                            last_arg_doc,
                            doc::text(")"),
                        ]),
                        // State 2: All args broken out
                        doc::concat(vec![
                            callee,
                            doc::text("("),
                            doc::indent(doc::concat(vec![
                                doc::line(),
                                all_args_broken,
                                doc::text(","),
                            ])),
                            doc::line(),
                            doc::text(")"),
                        ]),
                    ]);
                }
            }
        }

        // Check for any comments in arguments (leading, inter-argument, or trailing)
        let paren_open = call.callee.span().end;
        let has_leading_comments = !call.arguments.is_empty()
            && self.has_comments_between(paren_open, call.arguments[0].span().start);
        let has_inter_arg_comments = has_inter_argument_comments(call, self);
        let has_trailing_arg_comments = has_trailing_comments_on_args(call, self);

        if has_leading_comments || has_inter_arg_comments || has_trailing_arg_comments {
            // Build arguments with leading and/or inter-argument comments
            let mut arg_parts = Vec::new();
            let mut force_expansion = false;
            let mut has_trailing_comma_on_last = false;

            for (i, arg) in call.arguments.iter().enumerate() {
                // Handle leading comments before first argument
                if i == 0 && has_leading_comments {
                    let first_arg_start = arg.span().start;

                    if should_force_expansion_for_comments(self, paren_open, first_arg_start) {
                        force_expansion = true;
                    }

                    arg_parts.push(self.build_inline_comments_between_doc_no_leading_space(
                        paren_open,
                        first_arg_start,
                    ));
                    arg_parts.push(doc::line());
                }

                // Build the argument
                arg_parts.push(self.build_expression_doc(arg));

                // Check for comments after this argument (before next arg or closing paren)
                if i < call.arguments.len() - 1 {
                    let arg_end = arg.span().end;
                    let next_arg_start = call.arguments[i + 1].span().start;

                    if self.has_comments_between(arg_end, next_arg_start) {
                        if should_force_expansion_for_comments(self, arg_end, next_arg_start) {
                            force_expansion = true;
                        }

                        let pc = PartitionedComments::new(
                            self.comments,
                            self.source,
                            arg_end,
                            next_arg_start,
                        );

                        if pc.has_trailing_line() {
                            // Trailing line comments: comma, comment, hardline
                            force_expansion = true;
                            arg_parts.push(doc::text(","));
                            for comment in &pc.trailing_line {
                                arg_parts.push(doc::text(" "));
                                arg_parts.push(self.build_comment_doc(comment));
                            }
                            arg_parts.push(doc::hardline());
                        } else if pc.has_trailing_block() {
                            // Trailing block comments: comma, space, comment, line
                            arg_parts.push(doc::text(","));
                            for comment in &pc.trailing_block {
                                arg_parts.push(doc::text(" "));
                                arg_parts.push(self.build_comment_doc(comment));
                            }
                            arg_parts.push(doc::line());
                        } else {
                            // No trailing comments, add comma and line
                            arg_parts.push(doc::text(","));
                            arg_parts.push(doc::line());
                        }

                        // Add leading comments (each on own line)
                        for comment in &pc.leading {
                            arg_parts.push(self.build_comment_doc(comment));
                            arg_parts.push(doc::hardline());
                        }
                    } else {
                        // No comments, just comma and line
                        arg_parts.push(doc::comma_line());
                    }
                } else {
                    // Last argument - check for trailing line comments before closing paren
                    let arg_end = arg.span().end;
                    let paren_close = call.span.end;

                    let pc =
                        PartitionedComments::new(self.comments, self.source, arg_end, paren_close);

                    if pc.has_trailing_line() {
                        force_expansion = true;
                        // Add comma before trailing comments (this serves as the trailing comma)
                        arg_parts.push(doc::text(","));
                        for comment in &pc.trailing_line {
                            arg_parts.push(doc::text(" "));
                            arg_parts.push(self.build_comment_doc(comment));
                        }
                        // Mark that we already added trailing comma
                        has_trailing_comma_on_last = true;
                    }
                }
            }

            let arg_doc = doc::concat(arg_parts);

            // Force hard breaks if needed, otherwise allow collapsing
            if force_expansion {
                // Build manually when we have trailing comments (we already added our commas)
                // Add trailing comma after last arg ONLY if we didn't already add one
                let trailing = if has_trailing_comma_on_last {
                    doc::empty()
                } else {
                    doc::text(",")
                };
                return doc::concat(vec![
                    callee,
                    doc::text("("),
                    doc::indent(doc::concat(vec![doc::hardline(), arg_doc, trailing])),
                    doc::hardline(),
                    doc::text(")"),
                ]);
            }

            return wrap_call_with_soft_breaks(callee, arg_doc);
        }

        // "First args inline with last array/object" pattern:
        // When last arg is array/object and preceding args are short,
        // keep short args inline with the opening bracket/brace
        // e.g., fn('x', [Item1, Item2]) stays as fn('x', [\n\tItem1,\n\tItem2,\n])
        if call.arguments.len() >= 2 && last_arg_is_array_or_object(&call.arguments) {
            // Skip this pattern if there are inter-argument comments
            // (they'll be handled by the general inter-argument comment handler below)
            let has_inter_arg_comments = has_inter_argument_comments(call, self);

            if preceding_args_are_short(&call.arguments) && !has_inter_arg_comments {
                let (head_parts, last_arg_doc, _) = build_args_split_last(&call.arguments, self);

                // Keep short args inline with last arg's opener,
                // wrapping the entire call in a group so the array/object
                // only collapses if the WHOLE call fits
                return doc::group(doc::concat(vec![
                    callee,
                    doc::text("("),
                    doc::concat(head_parts),
                    last_arg_doc,
                    doc::text(")"),
                ]));
            }
        }

        // Check for blank lines between arguments (forces expansion and preservation)
        let has_blank_lines = call.arguments.windows(2).any(|window| {
            let prev_end = window[0].span().end;
            let curr_start = window[1].span().start;
            has_blank_line_between(self.source, prev_end, curr_start)
        });

        if has_blank_lines {
            // Build arguments with blank line preservation (forced expansion)
            let mut arg_parts = Vec::new();
            for (i, arg) in call.arguments.iter().enumerate() {
                // Check for blank line before this arg
                if i > 0 {
                    let prev_end = call.arguments[i - 1].span().end;
                    let curr_start = arg.span().start;
                    if has_blank_line_between(self.source, prev_end, curr_start) {
                        arg_parts.push(doc::literalline());
                        arg_parts.push(doc::hardline());
                    }
                }

                arg_parts.push(self.build_expression_doc(arg));

                // Add comma+hardline after each arg except the last
                if i < call.arguments.len() - 1 {
                    arg_parts.push(doc::text(","));

                    // Check if next arg has blank line before it
                    // If not, add hardline here
                    let next_start = call.arguments[i + 1].span().start;
                    let curr_end = arg.span().end;
                    if !has_blank_line_between(self.source, curr_end, next_start) {
                        arg_parts.push(doc::hardline());
                    }
                }
            }

            let arg_doc = doc::concat(arg_parts);
            return wrap_call_with_hard_breaks(callee, arg_doc);
        }

        // Build args with line separators (one per line when broken)
        // Boolean() calls don't get extra indent on binary continuation lines
        let use_arg_indent = !is_boolean_call(call, self);
        let arg_docs: Vec<_> = call
            .arguments
            .iter()
            .map(|arg| {
                if use_arg_indent {
                    self.build_arg_expression_doc(arg)
                } else {
                    self.build_expression_doc(arg)
                }
            })
            .collect();
        let arg_parts = doc::join_doc(arg_docs, doc::comma_line());

        // Wrap in group with parens
        wrap_call_with_soft_breaks(callee, arg_parts)
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
    ///
    /// Uses proper group wrapping so args can break independently from the chain.
    /// This allows the chain's conditionalGroup to try:
    /// 1. Everything inline
    /// 2. Args broken but chain inline (if args are in their own group)
    /// 3. Chain broken (if args broken still doesn't fit)
    fn build_call_args_doc_for_chain(
        &self,
        call: &internal::CallExpression,
        optional: bool,
    ) -> Doc {
        self.build_call_args_doc_for_chain_impl(call, optional, false)
    }

    /// Build a Doc for call arguments with forced expansion (hardlines instead of softlines)
    ///
    /// Used for the "args expanded, chain inline" state in conditionalGroup.
    fn build_call_args_doc_for_chain_expanded(
        &self,
        call: &internal::CallExpression,
        optional: bool,
    ) -> Doc {
        self.build_call_args_doc_for_chain_impl(call, optional, true)
    }

    /// Implementation for call args doc building
    fn build_call_args_doc_for_chain_impl(
        &self,
        call: &internal::CallExpression,
        optional: bool,
        force_expand: bool,
    ) -> Doc {
        // Build type arguments if present: `<T, U>`
        let type_args_doc = call
            .type_arguments
            .as_ref()
            .map(|ta| self.build_type_parameter_instantiation_doc(ta));

        // Check for blank lines between arguments (forces expansion)
        let has_blank_lines = call.arguments.windows(2).any(|window| {
            let prev_end = window[0].span().end;
            let curr_start = window[1].span().start;
            has_blank_line_between(self.source, prev_end, curr_start)
        });

        let force_expand = force_expand || has_blank_lines;

        let prefix = if optional { "?.(" } else { "(" };

        let mut parts = Vec::new();
        if let Some(ta_doc) = type_args_doc {
            parts.push(ta_doc);
        }

        if call.arguments.is_empty() {
            parts.push(doc::text_owned(format!("{prefix})")));
            doc::concat(parts)
        } else if force_expand {
            // Forced expansion: use hardlines instead of softlines
            // Build arguments with blank line preservation
            let mut arg_parts = Vec::new();
            for (i, arg) in call.arguments.iter().enumerate() {
                // Check for blank line before this arg
                if i > 0 {
                    let prev_end = call.arguments[i - 1].span().end;
                    let curr_start = arg.span().start;
                    if has_blank_line_between(self.source, prev_end, curr_start) {
                        arg_parts.push(doc::literalline());
                        arg_parts.push(doc::hardline());
                    }
                }

                arg_parts.push(self.build_arg_expression_doc(arg));

                // Add comma+hardline after each arg except the last
                if i < call.arguments.len() - 1 {
                    arg_parts.push(doc::text(","));

                    // Check if next arg has blank line before it
                    // If not, add hardline here
                    let next_start = call.arguments[i + 1].span().start;
                    let curr_end = arg.span().end;
                    if !has_blank_line_between(self.source, curr_end, next_start) {
                        arg_parts.push(doc::hardline());
                    }
                }
            }

            parts.push(doc::text(prefix));
            parts.push(doc::indent(doc::concat(vec![
                doc::hardline(),
                doc::concat(arg_parts),
                doc::text(","),
            ])));
            parts.push(doc::hardline());
            parts.push(doc::text(")"));
            doc::concat(parts)
        } else {
            // Single argument that needs soft-break wrapping (not huggable)
            if call.arguments.len() == 1 && arg_needs_soft_wrap(&call.arguments[0]) {
                let arg_doc = self.build_arg_expression_doc(&call.arguments[0]);
                parts.push(wrap_args_with_soft_breaks(prefix, arg_doc));
                return doc::concat(parts);
            }

            // Simple join with comma+space - no group wrapping
            // This allows object/array arguments to hug the parens
            // The chain's conditionalGroup handles breaking decisions
            let args: Vec<Doc> = call
                .arguments
                .iter()
                .map(|arg| self.build_arg_expression_doc(arg))
                .collect();
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
        // Build the inner expression with proper grouping for parenthesized chains
        match expr {
            internal::Expression::BinaryExpression(binary) => {
                // Different structures for different operator types:
                // - Arithmetic chains (/, *, etc.): group(["(", indent([softline, content]), softline, ")"])
                //   In flat: (a / b / c)
                //   In break: (\n\ta /\n\tb /\n\tc\n)
                // - Logical chains (&&, ||): Keep original structure with continuation indent
                //   In flat: (a || b)
                //   In break: (a ||\n\tb)
                if binary.operator.is_logical() {
                    // For logical operators, use the original structure
                    let inner = self.build_binary_chain_parts_indented(binary);
                    doc::group(doc::parens(inner))
                } else {
                    // For arithmetic operators, use the parens-on-own-line structure
                    let inner = self.build_binary_chain_for_parens(binary);
                    doc::group(doc::concat(vec![
                        doc::text("("),
                        doc::indent(doc::concat(vec![doc::softline(), inner])),
                        doc::softline(),
                        doc::text(")"),
                    ]))
                }
            }
            _ => doc::parens(self.build_expression_doc(expr)),
        }
    }

    fn print_call_args(&self, call: &internal::CallExpression, optional: bool) -> Doc {
        self.build_call_args_doc_for_chain(call, optional)
    }

    fn print_call_args_expanded(&self, call: &internal::CallExpression, optional: bool) -> Doc {
        self.build_call_args_doc_for_chain_expanded(call, optional)
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

    fn build_trailing_line_comments_doc(&self, start: u32, end: u32) -> Doc {
        // Find line comments on the SAME line as start
        let line_comments = self.filter_line_comments(start, end, true);

        if line_comments.is_empty() {
            return doc::empty();
        }

        // Line comments in chains need special handling:
        // Use line_suffix_boundary + line_suffix to keep comment with preceding call
        // The boundary ensures the comment is flushed before the next softline
        let mut parts = Vec::new();
        for comment in line_comments {
            parts.push(doc::line_suffix(doc::concat(vec![
                doc::text(" "),
                self.build_comment_doc(comment),
            ])));
        }
        // Add boundary to flush the line_suffix before any following softline
        parts.push(doc::line_suffix_boundary());
        doc::concat(parts)
    }

    fn build_leading_line_comments_doc(&self, start: u32, end: u32) -> Doc {
        // Find line comments NOT on the same line as start (on their own lines)
        let line_comments = self.filter_line_comments(start, end, false);

        if line_comments.is_empty() {
            return doc::empty();
        }

        // Emit line comments on their own lines (with hardline after each)
        let mut parts = Vec::new();
        for comment in line_comments {
            parts.push(self.build_comment_doc(comment));
            parts.push(doc::hardline());
        }
        doc::concat(parts)
    }

    fn get_property_span(&self, expr: &internal::Expression) -> tsv_lang::Span {
        expr.span()
    }

    fn is_expression_statement(&self) -> bool {
        self.is_expression_statement.get()
    }

    fn get_source(&self) -> &str {
        self.source
    }

    fn has_comments_between(&self, start: u32, end: u32) -> bool {
        super::comments_in_range(self.comments, start, end)
            .next()
            .is_some()
    }

    fn get_tab_width(&self) -> usize {
        self.config.tab_width
    }

    fn should_force_expand(&self) -> bool {
        self.force_chain_expand.get()
    }
}
