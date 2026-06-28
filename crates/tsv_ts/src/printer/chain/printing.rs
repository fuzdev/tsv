// Chain node and group printing for TypeScript member chain formatting
//
// This module handles rendering chain nodes and groups to Docs:
// - print_node: Basic node printing (with optional expansion flag)
// - print_group: Group printing (with optional expansion and comment skipping)
// - ChainPrinter trait: Interface for the printer

use super::analysis::SymbolLookup;
use super::types::{ChainGroup, ChainNode};
use crate::ast::internal::{self, Expression};
use string_interner::DefaultSymbol;
use tsv_lang::doc::{
    DocBuf,
    arena::{DocArena, DocId},
};
use tsv_lang::printing::has_blank_line_between_strict;
use tsv_lang::{ClassifiedComments, Span, SymbolToU32};

//
// Trait Definition
//

/// Trait for printing chain elements (abstraction over Printer)
pub trait ChainPrinter: SymbolLookup {
    /// Get a reference to the doc arena
    fn arena(&self) -> &DocArena;

    /// Print an expression as a DocId
    fn print_expression(&self, expr: &Expression<'_>) -> DocId;

    /// Build inner doc for logical binary in parenthesized chain base
    fn build_parenthesized_base_inner_logical(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId;

    /// Build inner doc for arithmetic binary in parenthesized chain base
    fn build_parenthesized_base_inner_binary(
        &self,
        binary: &internal::BinaryExpression<'_>,
    ) -> DocId;

    /// Print call arguments: () or (arg1, arg2)
    fn print_call_args(&self, call: &internal::CallExpression<'_>, optional: bool) -> DocId;

    /// Print call arguments with forced expansion (hardlines)
    /// Used for the "args broken, chain inline" state in conditionalGroup
    fn print_call_args_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId;

    /// Print call arguments with standard forced expansion (hardlines, no arrow hugging)
    /// Always uses `(\n  args,\n)`, never `(sig =>\n  body,\n)`
    fn print_call_args_standard_expanded(
        &self,
        call: &internal::CallExpression<'_>,
        optional: bool,
    ) -> DocId;

    /// Build a doc for block comments between two positions.
    ///
    /// When `same_line_only` is true, only includes comments on the same line as `start`.
    /// When false, includes all block comments in range.
    /// Note: Only block comments are included (line comments are filtered out).
    fn build_block_comments_doc(
        &self,
        start: u32,
        end: u32,
        spacing: crate::printer::CommentSpacing,
        same_line_only: bool,
    ) -> DocId;

    /// Get the span for a given expression
    fn get_property_span(&self, expr: &Expression<'_>) -> Span;

    /// Check if the chain is the direct child of an ExpressionStatement
    ///
    /// Used to determine if short identifier names should be merged with their
    /// first call (e.g., `a.fn().b()` → merge `a` with `.fn()` only in statements).
    fn is_expression_statement(&self) -> bool;

    /// Reset `is_expression_statement` to false.
    ///
    /// Called after `should_merge` captures the flag, so that sub-expressions
    /// (call arguments, etc.) don't inherit the expression statement context.
    /// Prettier checks `path.parent.type === "ExpressionStatement"` per-chain,
    /// so only the outermost chain should see `true`.
    fn clear_expression_statement(&self);

    /// Get the precomputed line breaks table for O(log n) line boundary lookups
    fn get_line_breaks(&self) -> &[u32];

    /// Check if there are any comments between two positions
    fn has_comments_between(&self, start: u32, end: u32) -> bool;

    /// Classify all comments in a range by position and type in a single pass.
    ///
    /// Returns comments organized into 4 buckets (trailing_block, trailing_line,
    /// leading_block, leading_line) using a single binary search instead of 4
    /// separate filter calls.
    fn classify_comments(&self, start: u32, end: u32) -> ClassifiedComments<'_>;

    /// Build doc for trailing block comments from a pre-classified slice.
    /// Emits space before each comment: `method() /* c */`
    fn build_trailing_block_doc(&self, comments: &[&tsv_lang::Comment]) -> DocId;

    /// Build doc for trailing line comments from a pre-classified slice.
    /// Uses line_suffix to keep comments with preceding element.
    fn build_trailing_line_doc(&self, comments: &[&tsv_lang::Comment]) -> DocId;

    /// Build doc for a pre-classified slice of leading comments, each on its
    /// own line (hardline after each).
    fn build_leading_comments_doc(&self, comments: &[&tsv_lang::Comment]) -> DocId;

    /// Build line_suffix docs for line comments WITHOUT a trailing boundary.
    ///
    /// Used for inline chain formatting where we want comments to defer to end
    /// of line without being flushed immediately. Unlike `build_trailing_line_doc`,
    /// this doesn't add a `line_suffix_boundary()` at the end.
    fn build_line_comments_no_boundary(&self, comments: &[&tsv_lang::Comment]) -> DocId;

    /// Get the source code string
    fn get_source(&self) -> &str;

    /// Check if chain expansion should be forced
    ///
    /// Used when inside template expressions with original breaks, where the
    /// expression is too long for the remaining print width.
    fn should_force_expand(&self) -> bool;
}

//
// Node Printing
//

/// Print a single chain node
///
/// Parameters:
/// - `node`: The chain node to print
/// - `printer`: The printer implementation
/// - `expanded`: If true, use forced call expansion (hardlines)
/// - `skip_comments`: If true, skip comments for member nodes (used when comments
///   are handled separately by add_comments_and_break)
pub(crate) fn print_node_inner<'a, P: ChainPrinter>(
    node: &ChainNode<'a>,
    printer: &P,
    expanded: bool,
    skip_comments: bool,
) -> DocId {
    let d = printer.arena();
    match node {
        ChainNode::Base {
            expr,
            needs_parens,
            paren_comment_end,
        } => {
            if *needs_parens {
                let inner = printer.print_expression(expr);
                // Match Prettier: inner group handles indent-on-break,
                // bare parens outside so chain conditionalGroup drives breaking
                let inner_group = match expr {
                    Expression::AwaitExpression(_) => {
                        // Prettier: group([indent([softline, inner]), softline])
                        d.group(
                            d.concat(&[d.indent(d.concat(&[d.softline(), inner])), d.softline()]),
                        )
                    }
                    Expression::BinaryExpression(binary) if binary.operator.is_logical() => {
                        // Logical: keep existing indented structure
                        printer.build_parenthesized_base_inner_logical(binary)
                    }
                    Expression::BinaryExpression(binary) => {
                        // Arithmetic: same indent-on-break as await
                        let bin_inner = printer.build_parenthesized_base_inner_binary(binary);
                        d.group(d.concat(&[
                            d.indent(d.concat(&[d.softline(), bin_inner])),
                            d.softline(),
                        ]))
                    }
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                        // IIFE / function callee or arrow member-object: the parens
                        // hug the function — its own body drives breaking, prettier
                        // never breaks after the `(` here. `(() => {...})().catch()`,
                        // `(function () {})().p`. Matches the bare-callee path
                        // (`call_formatting.rs`), which wraps with hugging parens.
                        inner
                    }
                    _ => {
                        // All other expressions: same indent-on-break as await
                        // so chain conditionalGroup can try flat first
                        d.group(
                            d.concat(&[d.indent(d.concat(&[d.softline(), inner])), d.softline()]),
                        )
                    }
                };
                // Preserve a comment from the stripped grouping parens inside them,
                // before `)` (`(x + y /* c */)!.foo`) — prettier relocates it past
                // `)`; tsv keeps it where the author wrote it.
                if let Some(end) = *paren_comment_end {
                    let start = expr.span().end;
                    let classified = printer.classify_comments(start, end);
                    let has_line =
                        !classified.trailing_line.is_empty() || !classified.leading_line.is_empty();
                    if has_line {
                        // A line comment can't trail inline before `)` (the `//` would
                        // swallow it), so force the multiline operand layout, keeping
                        // the comment inside — the same shape as a unary line-comment
                        // operand (`!(\n\tx + y // c\n)`).
                        let leading_block =
                            printer.build_leading_comments_doc(&classified.leading_block);
                        let leading_line =
                            printer.build_leading_comments_doc(&classified.leading_line);
                        let trailing_block =
                            printer.build_trailing_block_doc(&classified.trailing_block);
                        let trailing_line =
                            printer.build_trailing_line_doc(&classified.trailing_line);
                        return d.concat(&[
                            d.text("("),
                            d.indent(d.concat(&[
                                d.hardline(),
                                leading_block,
                                leading_line,
                                inner,
                                trailing_block,
                                trailing_line,
                            ])),
                            d.hardline(),
                            d.text(")"),
                        ]);
                    }
                    if printer.has_comments_between(start, end) {
                        let trailing = printer.build_block_comments_doc(
                            start,
                            end,
                            crate::printer::CommentSpacing::Leading,
                            false,
                        );
                        return d.concat(&[d.text("("), inner_group, trailing, d.text(")")]);
                    }
                }
                d.parens(inner_group)
            } else {
                printer.print_expression(expr)
            }
        }

        ChainNode::Call { call, optional } => {
            if expanded {
                printer.print_call_args_expanded(call, *optional)
            } else {
                printer.print_call_args(call, *optional)
            }
        }

        ChainNode::Member {
            property,
            optional,
            object_end,
            property_start,
        } => print_member_access(
            printer,
            *property,
            *optional,
            *object_end,
            *property_start,
            false,
            skip_comments,
        ),

        ChainNode::PrivateMember {
            property,
            optional,
            object_end,
            property_start,
        } => print_member_access(
            printer,
            *property,
            *optional,
            *object_end,
            *property_start,
            true,
            skip_comments,
        ),

        ChainNode::ComputedMember {
            expr,
            optional,
            object_end,
            bracket_end,
        } => {
            let inner = printer.print_expression(expr);
            let prop_span = printer.get_property_span(expr);

            // Find the opening bracket position by scanning from object_end,
            // skipping over comments to find the actual `[` (or `?.[` for optional)
            let bracket_open_pos = find_bracket_position(printer, *object_end, prop_span.start);

            // Comments between object and `[` stay OUTSIDE brackets
            // Comments between `[` and property go INSIDE brackets
            let pre_bracket = printer.classify_comments(*object_end, bracket_open_pos);
            let pre_trailing_line =
                printer.build_line_comments_no_boundary(&pre_bracket.trailing_line);
            let pre_leading_line =
                printer.build_line_comments_no_boundary(&pre_bracket.leading_line);
            // Block comments before the bracket (e.g., `a /* c */[0]`)
            let pre_bracket_block_doc = printer.build_block_comments_doc(
                *object_end,
                bracket_open_pos,
                crate::printer::CommentSpacing::Leading,
                true,
            );

            // Block comments inside brackets: [/* c */ key] and [key /* c */]
            // No same-line filter because when the input is already broken across
            // lines, the comment may be on a different line from the bracket opening
            // (e.g., `?.[` on one line, `/** @type {string} */ d` on the next).
            let inside_start = bracket_open_pos + if *optional { 3 } else { 1 };
            let leading_comments_doc = printer.build_block_comments_doc(
                inside_start,
                prop_span.start,
                crate::printer::CommentSpacing::Trailing,
                false,
            );
            let trailing_comments_doc = printer.build_block_comments_doc(
                prop_span.end,
                *bracket_end,
                crate::printer::CommentSpacing::Leading,
                false,
            );

            let inner_with_comments =
                d.concat(&[leading_comments_doc, inner, trailing_comments_doc]);

            // When there are block comments inside brackets (e.g., `?.[/** @type {string} */ d]`),
            // use a group with indent/softline so the bracket content can break:
            //   obj.chain?.[
            //       /** @type {string} */ d
            //   ]
            // Without comments, keep the flat form (existing behavior).
            let has_inside_comments = printer.has_comments_between(inside_start, prop_span.start)
                || printer.has_comments_between(prop_span.end, *bracket_end);
            let bracket_doc = if has_inside_comments {
                let open = if *optional { "?.[" } else { "[" };
                let sl = d.softline();
                let content = d.concat(&[sl, inner_with_comments]);
                let indented = d.indent(content);
                let sl2 = d.softline();
                d.group(d.concat(&[d.text(open), indented, sl2, d.text("]")]))
            } else if *optional {
                d.concat(&[d.text("?.["), inner_with_comments, d.text("]")])
            } else {
                d.brackets(inner_with_comments)
            };

            // Emit: pre-bracket line comments, pre-bracket block comments, then brackets
            d.concat(&[
                pre_trailing_line,
                pre_leading_line,
                pre_bracket_block_doc,
                bracket_doc,
            ])
        }

        ChainNode::NonNull => d.text("!"),
    }
}

/// Print a single chain node (normal mode)
pub(crate) fn print_node<'a, P: ChainPrinter>(node: &ChainNode<'a>, printer: &P) -> DocId {
    print_node_inner(node, printer, false, false)
}

//
// Group Printing
//

/// Print a single chain group
pub(crate) fn print_group<'a, P: ChainPrinter>(group: &ChainGroup<'a>, printer: &P) -> DocId {
    print_group_inner(group, printer, false, false)
}

/// Print a chain group with forced call expansion
pub(crate) fn print_group_expanded<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> DocId {
    print_group_inner(group, printer, true, false)
}

/// Print a chain group with standard forced call expansion (no arrow hugging)
///
/// Like `print_group_expanded`, but uses `(\n  args,\n)` instead of `(sig =>\n  body,\n)`
/// for single-arg arrows with breakable bodies. Used in short chain states where the
/// chain doesn't break between groups.
pub(crate) fn print_group_standard_expanded<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> DocId {
    let d = printer.arena();
    let docs: DocBuf = group
        .nodes
        .iter()
        .map(|n| match n {
            ChainNode::Call { call, optional } => {
                printer.print_call_args_standard_expanded(call, *optional)
            }
            _ => print_node_inner(n, printer, false, false),
        })
        .collect();
    d.concat(&docs)
}

/// Print a chain group, skipping block comments for the first member node
///
/// Used by ChainPartsBuilder in expanded path where `add_comments_and_break`
/// already handles comments for the first member (emitting before the line break).
pub(crate) fn print_group_skip_first_comments<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> DocId {
    print_group_inner(group, printer, false, true)
}

/// Print a chain group with forced call expansion, skipping block comments for the first member
pub(crate) fn print_group_expanded_skip_first_comments<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> DocId {
    print_group_inner(group, printer, true, true)
}

/// Internal implementation for printing a chain group
fn print_group_inner<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
    expanded: bool,
    skip_first_comments: bool,
) -> DocId {
    let d = printer.arena();
    let docs: DocBuf = group
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            // Skip comments only for the first member node
            let skip_comments = skip_first_comments && i == 0 && n.is_member();
            print_node_inner(n, printer, expanded, skip_comments)
        })
        .collect();
    d.concat(&docs)
}

//
// Member Access Printing
//

/// Print a member access (shared logic for Member and PrivateMember)
///
/// Emits comments before the member access:
/// - Block comments inline (e.g., `a /* comment */.b`)
/// - Line comments via line_suffix (moved to end of line, matching Prettier)
///
/// The `skip_comments` flag is used by the expanded path where `add_comments_and_break`
/// already handles comments for the first member of rest groups.
fn print_member_access<P: ChainPrinter>(
    printer: &P,
    property: DefaultSymbol,
    optional: bool,
    object_end: u32,
    property_start: u32,
    is_private: bool,
    skip_comments: bool,
) -> DocId {
    let d = printer.arena();
    // Build member doc without format! allocation - use doc::symbol for deferred resolution
    let prop_id = property.to_u32();
    let member_doc = match (optional, is_private) {
        (false, false) => d.concat(&[d.text("."), d.symbol(prop_id)]),
        (true, false) => d.concat(&[d.text("?."), d.symbol(prop_id)]),
        (false, true) => d.concat(&[d.text(".#"), d.symbol(prop_id)]),
        (true, true) => d.concat(&[d.text("?.#"), d.symbol(prop_id)]),
    };

    if skip_comments {
        return member_doc;
    }

    // Classify all comments in the range between object and property
    let classified = printer.classify_comments(object_end, property_start);

    // Trailing block comments: same line as previous element (e.g., `method() /* c */.prop`)
    let trailing_block = printer.build_trailing_block_doc(&classified.trailing_block);

    // Line comments (both trailing and leading) get moved to end of line via line_suffix.
    // This matches Prettier's behavior of hoisting mid-chain line comments.
    // NOTE: We use build_line_comments_no_boundary here (not build_trailing_line_doc) because
    // we don't want to flush the line_suffix immediately - it should stay deferred until
    // the actual end of line.
    let trailing_line = printer.build_line_comments_no_boundary(&classified.trailing_line);
    let leading_line = printer.build_line_comments_no_boundary(&classified.leading_line);

    // Leading block comments on their own line - emit inline (rare case)
    let leading_block = printer.build_trailing_block_doc(&classified.leading_block);

    d.concat(&[
        trailing_block,
        trailing_line,
        leading_block,
        leading_line,
        member_doc,
    ])
}

/// Check if a computed member node has block comments inside its brackets.
///
/// Used by the chain builder to route chains with inside-bracket comments
/// through the conditional_group path (instead of fill), so the bracket
/// content can break internally.
pub(crate) fn has_inside_bracket_comments<'a, P: ChainPrinter>(
    node: &ChainNode<'a>,
    printer: &P,
) -> bool {
    if let ChainNode::ComputedMember {
        expr,
        optional,
        object_end,
        bracket_end,
    } = node
    {
        let prop_span = printer.get_property_span(expr);
        let bracket_open_pos = find_bracket_position(printer, *object_end, prop_span.start);
        let inside_start = bracket_open_pos + if *optional { 3 } else { 1 };
        printer.has_comments_between(inside_start, prop_span.start)
            || printer.has_comments_between(prop_span.end, *bracket_end)
    } else {
        false
    }
}

/// Find the position of `[` (or `?.[` for optional) in the source,
/// skipping over comments to avoid matching `[` inside comments.
///
/// For `?.[`, returns the position of `?` (the start of the optional chain syntax).
fn find_bracket_position<P: ChainPrinter>(printer: &P, start: u32, end: u32) -> u32 {
    let source = printer.get_source();
    let bytes = source.as_bytes();
    let start_pos = start as usize;
    let end_pos = end as usize;
    let mut i = start_pos;

    while i < end_pos {
        if let Some(new_i) = tsv_lang::source_scan::skip_comment(bytes, i, end_pos) {
            i = new_i;
            continue;
        }
        // Check for `?.[` first (returns position of `?`)
        if bytes[i] == b'?' && i + 2 < end_pos && bytes[i + 1] == b'.' && bytes[i + 2] == b'[' {
            return i as u32;
        }
        // Check for plain `[`
        if bytes[i] == b'[' {
            return i as u32;
        }
        i += 1;
    }
    start // Fallback
}

//
// Helper Functions
//

/// Build a line break doc with optional blank line preservation
///
/// Returns:
/// - `softline` when `use_hardline` is false
/// - `hardline` when no blank line in source
/// - `literalline + hardline` when blank line should be preserved
pub(crate) fn build_chain_line_break<P: ChainPrinter>(
    printer: &P,
    object_end: u32,
    property_start: u32,
    use_hardline: bool,
) -> DocId {
    let d = printer.arena();
    if !use_hardline {
        return d.softline();
    }

    // Check for blank line preservation (only when no comments - comments handle their own spacing)
    // When there are comments between obj and property, the 2+ newlines (one before comment,
    // one after) should NOT be treated as a blank line.
    // Use strict check: verifies intermediate lines are truly blank (whitespace-only).
    // This avoids false positives when the parser strips grouping parens, leaving `)` between
    // newlines (e.g., `(fn({...}))\n.method()` → inner span ends before `)`, creating `\n)\n`).
    let source = printer.get_source();
    let has_comments = printer.has_comments_between(object_end, property_start);

    if !has_comments && has_blank_line_between_strict(source, object_end, property_start) {
        // Preserve blank line: literalline (no indent) + hardline (with indent for next content)
        d.concat(&[d.literalline(), d.hardline()])
    } else {
        d.hardline()
    }
}
