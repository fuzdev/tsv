// Pure AST analysis functions for TypeScript printer
//
// These functions analyze AST structures without needing Printer state.
// They determine formatting decisions like whether expressions need
// special handling, wrapping, or expansion.

use crate::ast::internal;
use crate::printer::Printer;
use string_interner::DefaultStringInterner;
use tsv_lang::Span;
use tsv_lang::doc::arena::DocId;

/// Skip past identifier characters (alphanumeric, `_`, `$`, non-ASCII) starting at `pos`.
///
/// Returns the position after the last identifier character, or `pos` if none found.
/// Handles multi-byte UTF-8 sequences for Unicode identifiers.
pub(crate) fn skip_identifier_at(bytes: &[u8], pos: usize, end: usize) -> usize {
    let mut i = pos;
    while i < end
        && (bytes[i].is_ascii_alphanumeric()
            || bytes[i] == b'_'
            || bytes[i] == b'$'
            || bytes[i] > 127)
    {
        i += 1;
    }
    i
}

/// Whether a `{ }`-delimited statement list prints no visible content.
///
/// A body containing only standalone `EmptyStatement`s (bare `;`) prints
/// nothing — Prettier's `printStatementSequence` drops them — so it's treated
/// the same as a genuinely empty body (comments attached to those statements
/// are picked up separately, by scanning the full brace range rather than the
/// statement list). Used by block-statement and namespace/module bodies to
/// decide between the empty-body and normal rendering paths.
pub(crate) fn is_effectively_empty_body(body: &[internal::Statement<'_>]) -> bool {
    body.iter()
        .all(|s| matches!(s, internal::Statement::EmptyStatement(_)))
}

/// The start of the next statement after `index` that will actually be printed
/// — the first one that isn't a dropped standalone `EmptyStatement` — falling
/// back to `body_end` when only dropped statements (or nothing) follow.
///
/// Used to bound a statement's trailing same-line comment scan. A comment
/// physically trailing a dropped `;` on the statement's own line (`a();; // c`)
/// must attach to that statement: the `;` it follows emits nothing, so bounding
/// the scan at the `;` — rather than the next *printed* statement — would strand
/// the comment (neither the statement's trailing scan nor the dropped `;`'s
/// leading-comment collection claims it) and silently drop it.
pub(crate) fn next_printed_stmt_start(
    body: &[internal::Statement<'_>],
    index: usize,
    body_end: u32,
) -> u32 {
    body[index + 1..]
        .iter()
        .find(|s| !matches!(s, internal::Statement::EmptyStatement(_)))
        .map_or(body_end, |s| s.span().start)
}

/// Check if an expression is a module path call that should use fluid assignment wrapping
/// (break after `=` if too long, keeping the call together).
///
/// Patterns:
/// - `require.resolve(stringLiteral)`
/// - `await import(stringLiteral)`
pub(crate) fn is_module_path_fluid_call(
    expr: &internal::Expression<'_>,
    source: &str,
    interner: &DefaultStringInterner,
) -> bool {
    // Check for `await import(string)` — single-arg only (no options)
    if let internal::Expression::AwaitExpression(await_expr) = expr
        && let internal::Expression::ImportExpression(import_expr) = await_expr.argument
        && import_expr.options.is_none()
    {
        return is_string_literal(import_expr.source);
    }

    let internal::Expression::CallExpression(call) = expr else {
        return false;
    };

    // Must have exactly 1 argument that is a string literal
    if call.arguments.len() != 1 || !is_string_literal(&call.arguments[0]) {
        return false;
    }

    // Check for `require.resolve()`
    if let internal::Expression::MemberExpression(member) = call.callee
        && !member.computed
        && !member.optional
        && let internal::Expression::Identifier(resolve_id) = member.property
        && resolve_id.name(source, interner) == "resolve"
        && let internal::Expression::Identifier(require_id) = member.object
        && require_id.name(source, interner) == "require"
    {
        return true;
    }

    false
}

/// Check if an expression is a string literal
pub(crate) fn is_string_literal(expr: &internal::Expression<'_>) -> bool {
    matches!(
        expr,
        internal::Expression::Literal(lit) if matches!(lit.value, internal::LiteralValue::String { .. })
    )
}

/// Check if an expression is a pure property chain (member expressions without calls)
///
/// Pure property chains like `obj.a.b.c` or `obj!.a!.b!` should use fluid assignment wrapping
/// (break after `=` if doesn't fit). Expressions containing calls, objects,
/// arrays, or ternaries handle their own wrapping internally.
pub(crate) fn is_pure_property_chain(expr: &internal::Expression<'_>) -> bool {
    match expr {
        // A member expression is a property chain if its object is also a pure chain
        internal::Expression::MemberExpression(member) => is_pure_property_chain(member.object),
        // TSNonNullExpression is transparent - recurse through it
        internal::Expression::TSNonNullExpression(non_null) => {
            is_pure_property_chain(non_null.expression)
        }
        // Base case: identifiers, this, super are valid chain roots
        internal::Expression::Identifier(_)
        | internal::Expression::ThisExpression(_)
        | internal::Expression::Super(_) => true,
        // Everything else (calls, objects, arrays, ternaries, etc.) is NOT a pure chain
        _ => false,
    }
}

/// Check if a ConditionalExpression needs break-after-operator layout.
///
/// Prettier uses "break-after-operator" when the ternary's test expression
/// is binaryish (BinaryExpression or LogicalExpression). This causes the pattern:
/// ```typescript
/// const value =
///     condition === 'value'
///         ? consequent
///         : alternate;
/// ```
///
/// Prettier ref: shouldBreakAfterOperator (assignment.js:216-219)
pub fn conditional_should_break_after_op(expr: &internal::Expression<'_>) -> bool {
    if let internal::Expression::ConditionalExpression(cond) = expr {
        // Check if test is binaryish (BinaryExpression includes logical operators like &&, ||),
        // but exclude logical expressions with inline-able RHS (non-empty object/array).
        // Prettier ref: assignment.js:219 `isBinaryish(test) && !shouldInlineLogicalExpression(test)`
        if let internal::Expression::BinaryExpression(binary) = cond.test {
            !super::expressions::assignment::should_inline_logical_expression(binary)
        } else {
            false
        }
    } else {
        false
    }
}

/// Check if an expression is a multiline string literal (contains line continuations).
///
/// Strings with `\<newline>` need fluid layout because:
/// 1. They span multiple lines in source
/// 2. Prettier always wraps the declaration for these
pub(crate) fn is_multiline_string_literal(expr: &internal::Expression<'_>, source: &str) -> bool {
    if let internal::Expression::Literal(lit) = expr
        && let internal::LiteralValue::String { .. } = &lit.value
    {
        let raw = lit.span.extract(source);
        // Check for line continuation: backslash followed by newline
        raw.contains("\\\n") || raw.contains("\\\r")
    } else {
        false
    }
}

/// Check if a brace-delimited block was written as multiline in source
///
/// Detects newline immediately after opening brace: `{\n  ...}` vs `{ ... }`
/// Used for type literals, mapped types, and other brace-delimited constructs.
pub(crate) fn is_brace_block_multiline(source: &str, span: Span) -> bool {
    let source_text = span.extract(source);
    let after_brace = source_text.strip_prefix('{').unwrap_or("");
    after_brace.starts_with('\n')
        || after_brace.starts_with("\r\n")
        || after_brace.trim_start_matches(' ').starts_with('\n')
}

/// Context for object pattern expansion decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatternContext {
    /// Pattern in function parameter position
    FunctionParameter,
    /// Pattern in standalone context (variable declaration, assignment)
    Standalone,
    /// The left of a destructuring default (`{ a } = …`). Prettier's `shouldBreak`
    /// excludes an `AssignmentPattern` parent (object.js), so such a pattern never
    /// expands on nesting — `{ a: { b } = {} }` stays inline (width-based breaking
    /// still applies elsewhere).
    AssignmentDefault,
}

/// Check if an object pattern should expand (print across multiple lines).
///
/// Prettier expands object patterns when any property has a nested pattern
/// (ObjectPattern or ArrayPattern) as its value. This is different from the
/// multiline content rule used for object expressions.
///
/// Prettier expands based on nesting depth and context:
/// - In function parameters: depth 3+ expands (e.g., {a: {b}} stays, {a: {b: {c}}} expands)
/// - In standalone contexts: depth 2+ expands (e.g., {a: {b}} expands)
pub(crate) fn object_pattern_should_expand(
    obj: &internal::ObjectPattern<'_>,
    context: PatternContext,
) -> bool {
    let depth = pattern_nesting_depth(obj);
    match context {
        PatternContext::FunctionParameter => depth >= 3,
        PatternContext::Standalone => depth >= 2,
        // A destructuring default's left never expands on nesting (prettier
        // excludes AssignmentPattern parents from shouldBreak).
        PatternContext::AssignmentDefault => false,
    }
}

/// Calculate the maximum nesting depth of an object pattern
/// Depth 1 = simple pattern like {a}
/// Depth 2 = one level of nesting like {a: {b}}
/// Depth 3 = two levels of nesting like {a: {b: {c}}}
fn pattern_nesting_depth(obj: &internal::ObjectPattern<'_>) -> usize {
    let mut max_depth = 1;

    for prop in obj.properties {
        match prop {
            internal::ObjectPatternProperty::Property(p) => {
                let nested_depth = match &p.value {
                    internal::Expression::ObjectPattern(nested_obj) => {
                        1 + pattern_nesting_depth(nested_obj)
                    }
                    internal::Expression::ArrayPattern(nested_arr) => {
                        1 + array_pattern_nesting_depth(nested_arr)
                    }
                    // A property with a DEFAULT (`x: { y } = …`) does NOT count its
                    // nested pattern toward the expansion depth — prettier's shouldBreak
                    // fires only on a value that is *directly* an Object/Array pattern and
                    // excludes an `AssignmentPattern` parent (object.js), so
                    // `{ a: { b } = {} }` stays inline. Treat it as depth 1, same as a
                    // plain binding (the `_ => 1` arm). (Surfaced by cargo-mutants: the
                    // recurse-into-`ap.left` arms survived because no fixture covered a
                    // defaulted nested pattern — the survivor was a real over-expansion bug.)
                    _ => 1,
                };
                max_depth = max_depth.max(nested_depth);
            }
            internal::ObjectPatternProperty::RestElement(_) => {}
        }
    }

    max_depth
}

/// Calculate the maximum nesting depth of an array pattern
fn array_pattern_nesting_depth(arr: &internal::ArrayPattern<'_>) -> usize {
    let mut max_depth = 1;

    for elem in arr.elements.iter().flatten() {
        let nested_depth = match elem {
            internal::Expression::ObjectPattern(nested_obj) => {
                1 + pattern_nesting_depth(nested_obj)
            }
            internal::Expression::ArrayPattern(nested_arr) => {
                1 + array_pattern_nesting_depth(nested_arr)
            }
            // A defaulted element (`[{ y } = …]`) doesn't count its nested pattern
            // toward the expansion depth — see the object-pattern counterpart above
            // (prettier's shouldBreak excludes `AssignmentPattern` parents).
            _ => 1,
        };
        max_depth = max_depth.max(nested_depth);
    }

    max_depth
}

/// Check if a template literal contains newlines in its content
///
/// Matches Prettier's `templateLiteralHasNewLines` function
pub(crate) fn template_literal_has_newlines(template: &internal::TemplateLiteral<'_>) -> bool {
    template.quasis.iter().any(|q| q.has_newline)
}

/// Check if an expression is a template literal (or tagged template) with embedded newlines.
///
/// Combines the TemplateLiteral/TaggedTemplateExpression dispatch with the newline check.
/// Used by call_formatting, new_expression, and arrow body formatting.
pub(crate) fn is_multiline_template_expression(expr: &internal::Expression<'_>) -> bool {
    match expr {
        internal::Expression::TemplateLiteral(t) => template_literal_has_newlines(t),
        internal::Expression::TaggedTemplateExpression(t) => {
            template_literal_has_newlines(&t.quasi)
        }
        _ => false,
    }
}

/// Check if there's a newline immediately before a position (skipping spaces/tabs).
///
/// Walks backwards from `pos` in the source, skipping horizontal whitespace.
/// Returns true if a newline is found before any non-whitespace character.
///
/// Mirrors Prettier's `!hasNewline(text, locStart(node), { backwards: true })`
/// used by `isTemplateOnItsOwnLine` to detect if a template literal was placed
/// on its own line by the author.
pub(crate) fn has_newline_before_position(source: &str, pos: u32) -> bool {
    let pos = pos as usize;
    for &b in source.as_bytes()[..pos].iter().rev() {
        match b {
            b' ' | b'\t' => continue,
            b'\n' | b'\r' => return true,
            _ => return false,
        }
    }
    false
}

/// Check if there's a newline immediately after a position (skipping spaces/tabs).
///
/// Walks forward from `pos` in the source, skipping horizontal whitespace.
/// Returns true if a newline is found before any non-whitespace character.
///
/// Mirrors Prettier's `hasNewline(text, locEnd(comment))` used by
/// `printLeadingComment` to choose the separator after a leading block comment.
pub(crate) fn has_newline_after_position(source: &str, pos: u32) -> bool {
    let pos = pos as usize;
    for &b in &source.as_bytes()[pos..] {
        match b {
            b' ' | b'\t' => continue,
            b'\n' | b'\r' => return true,
            _ => return false,
        }
    }
    false
}

/// Check if an expression contains multiline content (e.g., line continuation strings)
///
/// Recursively traverses nested structures (arrays, objects, calls) to find
/// multiline strings. Prettier expands ALL containing structures when a multiline
/// string is found anywhere in the tree.
pub(crate) fn has_multiline_content(expr: &internal::Expression<'_>, source: &str) -> bool {
    // Necessary-condition fast bail: a legacy line continuation is a backslash
    // immediately before a newline, so if the expression's source slice contains no
    // backslash at all, no descendant string literal can carry one — return false
    // without the recursive walk. This collapses the O(depth×size) re-scan on nested
    // object/array/call data (each enclosing container re-walked the whole subtree) to
    // one memchr-fast byte scan; a backslash anywhere (regex, string escape, path)
    // falls through to the precise recursive check, so the result is unchanged.
    if !expr.span().extract(source).contains('\\') {
        return false;
    }
    match expr {
        internal::Expression::Literal(_) => is_multiline_string_literal(expr, source),
        internal::Expression::ArrayExpression(arr) => arr
            .elements
            .iter()
            .flatten()
            .any(|elem| has_multiline_content(elem, source)),
        internal::Expression::ObjectExpression(obj) => {
            obj.properties.iter().any(|prop| match prop {
                internal::ObjectProperty::Property(p) => has_multiline_content(&p.value, source),
                internal::ObjectProperty::SpreadElement(s) => {
                    has_multiline_content(s.argument, source)
                }
            })
        }
        internal::Expression::CallExpression(call) => call
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, source)),
        internal::Expression::NewExpression(new_expr) => new_expr
            .arguments
            .iter()
            .any(|arg| has_multiline_content(arg, source)),
        internal::Expression::UnaryExpression(unary) => {
            has_multiline_content(unary.argument, source)
        }
        internal::Expression::UpdateExpression(update) => {
            has_multiline_content(update.argument, source)
        }
        internal::Expression::BinaryExpression(binary) => {
            has_multiline_content(binary.left, source)
                || has_multiline_content(binary.right, source)
        }
        internal::Expression::ConditionalExpression(cond) => {
            has_multiline_content(cond.test, source)
                || has_multiline_content(cond.consequent, source)
                || has_multiline_content(cond.alternate, source)
        }
        internal::Expression::MemberExpression(member) => {
            has_multiline_content(member.object, source)
        }
        // A JSDoc cast is transparent for multiline-content detection: the wrapped
        // expression's content still forces expansion of containing structures.
        internal::Expression::JsdocCast(cast) => has_multiline_content(cast.inner, source),
        // Preserved grouping parens are likewise transparent.
        internal::Expression::ParenthesizedExpression(paren) => {
            has_multiline_content(paren.expression, source)
        }
        internal::Expression::ArrowFunctionExpression(arrow) => match &arrow.body {
            internal::ArrowFunctionBody::Expression(expr) => has_multiline_content(expr, source),
            internal::ArrowFunctionBody::BlockStatement(_) => false,
        },
        internal::Expression::SpreadElement(spread) => {
            has_multiline_content(spread.argument, source)
        }
        internal::Expression::Identifier(_) => false,
        // Private identifiers are just #name, no multiline content
        internal::Expression::PrivateIdentifier(_) => false,
        internal::Expression::TemplateLiteral(template) => {
            // Template literals with newlines count as multiline content.
            // For single-arg calls, there's a special case earlier in call_formatting.rs
            // that keeps them inline. But for multi-arg calls, they trigger expansion.
            template_literal_has_newlines(template)
                || template
                    .expressions
                    .iter()
                    .any(|e| has_multiline_content(e, source))
        }
        internal::Expression::TaggedTemplateExpression(tagged) => {
            // Tagged template literals with newlines count as multiline content.
            // Check tag, template quasis, and interpolated expressions.
            has_multiline_content(tagged.tag, source)
                || template_literal_has_newlines(&tagged.quasi)
                || tagged
                    .quasi
                    .expressions
                    .iter()
                    .any(|e| has_multiline_content(e, source))
        }
        // Function and class expressions don't contribute to multiline content detection
        // They have their own block formatting
        internal::Expression::FunctionExpression(_) => false,
        internal::Expression::ClassExpression(_) => false,
        internal::Expression::AwaitExpression(await_expr) => {
            has_multiline_content(await_expr.argument, source)
        }
        internal::Expression::YieldExpression(yield_expr) => yield_expr
            .argument
            .as_ref()
            .is_some_and(|arg| has_multiline_content(arg, source)),
        internal::Expression::SequenceExpression(seq) => seq
            .expressions
            .iter()
            .any(|e| has_multiline_content(e, source)),
        // Regex literals don't have multiline content
        internal::Expression::RegexLiteral(_) => false,
        // this/super are just keywords, no multiline content
        internal::Expression::ThisExpression(_) | internal::Expression::Super(_) => false,
        // Assignment expression: check both sides
        internal::Expression::AssignmentExpression(assign) => {
            has_multiline_content(assign.left, source)
                || has_multiline_content(assign.right, source)
        }
        // Patterns: check their contents
        internal::Expression::ObjectPattern(obj) => obj.properties.iter().any(|prop| match prop {
            internal::ObjectPatternProperty::Property(p) => has_multiline_content(&p.value, source),
            internal::ObjectPatternProperty::RestElement(r) => {
                has_multiline_content(r.argument, source)
            }
        }),
        internal::Expression::ArrayPattern(arr) => arr
            .elements
            .iter()
            .flatten()
            .any(|elem| has_multiline_content(elem, source)),
        internal::Expression::AssignmentPattern(pattern) => {
            has_multiline_content(pattern.left, source)
                || has_multiline_content(pattern.right, source)
        }
        internal::Expression::RestElement(rest) => has_multiline_content(rest.argument, source),
        // Type assertion expressions: check the inner expression
        internal::Expression::TSTypeAssertion(type_assert) => {
            has_multiline_content(type_assert.expression, source)
        }
        internal::Expression::TSAsExpression(as_expr) => {
            has_multiline_content(as_expr.expression, source)
        }
        internal::Expression::TSSatisfiesExpression(sat_expr) => {
            has_multiline_content(sat_expr.expression, source)
        }
        internal::Expression::TSInstantiationExpression(inst_expr) => {
            has_multiline_content(inst_expr.expression, source)
        }
        internal::Expression::TSNonNullExpression(non_null_expr) => {
            has_multiline_content(non_null_expr.expression, source)
        }
        internal::Expression::ImportExpression(import_expr) => {
            has_multiline_content(import_expr.source, source)
        }
        // Meta properties (import.meta, new.target) are never multiline
        internal::Expression::MetaProperty(_) => false,
        // Parameter properties don't contain multiline content
        internal::Expression::TSParameterProperty(_) => false,
    }
}

/// Build doc for TSEntityName (qualified names like `A.B.C`)
///
/// Needs the printer for the name-emission seam (span-identity source slices,
/// interner-deferred escaped names).
pub(crate) fn build_entity_name_doc(
    printer: &Printer<'_>,
    name: &internal::TSEntityName<'_>,
) -> DocId {
    match name {
        internal::TSEntityName::Identifier(id) => printer.identifier_name_doc(id),
        // A qualified name is a dotted pair, so both gaps around the `.` are positions an
        // author can comment in (`ns /* c */.Type`, `ns./* c */ Type`). The shared printer
        // emits them; concatenating the three pieces scans neither and drops what's there.
        // One printer serves every qualified name, so this reaches type references,
        // interface heritage, and import-equals module references alike.
        internal::TSEntityName::QualifiedName(qn) => printer.build_dotted_pair_doc(
            build_entity_name_doc(printer, &qn.left),
            printer.identifier_name_doc(&qn.right),
            qn.left.span().end,
            qn.right.span.start,
        ),
    }
}
