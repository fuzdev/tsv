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
use tsv_lang::doc::{self, Doc};
use tsv_lang::printing::has_blank_line_between_fast;
use tsv_lang::{ClassifiedComments, Span, SymbolToU32};

//
// Trait Definition
//

/// Trait for printing chain elements (abstraction over Printer)
pub trait ChainPrinter: SymbolLookup {
    /// Print an expression as a Doc
    fn print_expression(&self, expr: &Expression) -> Doc;

    /// Print a parenthesized base expression with indent-on-break behavior
    fn print_parenthesized_base(&self, expr: &Expression) -> Doc;

    /// Print a parenthesized base expression with forced expansion (hardlines)
    /// Used for `args_break` state in conditional_group so fits() can measure correctly
    fn print_parenthesized_base_expanded(&self, expr: &Expression) -> Doc;

    /// Print call arguments: () or (arg1, arg2)
    fn print_call_args(&self, call: &internal::CallExpression, optional: bool) -> Doc;

    /// Print call arguments with forced expansion (hardlines)
    /// Used for the "args broken, chain inline" state in conditionalGroup
    fn print_call_args_expanded(&self, call: &internal::CallExpression, optional: bool) -> Doc;

    /// Build a doc for inline block comments between two positions
    /// Returns a doc with block comments using the specified spacing
    /// Note: Only block comments are included (line comments are filtered out)
    fn build_block_comments_doc(
        &self,
        start: u32,
        end: u32,
        spacing: crate::printer::CommentSpacing,
    ) -> Doc;

    /// Get the span for a given expression
    fn get_property_span(&self, expr: &Expression) -> Span;

    /// Check if the chain is the direct child of an ExpressionStatement
    ///
    /// Used to determine if short identifier names should be merged with their
    /// first call (e.g., `a.fn().b()` → merge `a` with `.fn()` only in statements).
    fn is_expression_statement(&self) -> bool;

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
    fn build_trailing_block_doc(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Build doc for trailing line comments from a pre-classified slice.
    /// Uses line_suffix to keep comments with preceding element.
    fn build_trailing_line_doc(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Build doc for leading block comments from a pre-classified slice.
    /// Comments are on their own lines, no surrounding spaces.
    fn build_leading_block_doc(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Build doc for leading line comments from a pre-classified slice.
    /// Emits hardline after each comment.
    fn build_leading_line_doc(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Build line_suffix docs for line comments WITHOUT a trailing boundary.
    ///
    /// Used for inline chain formatting where we want comments to defer to end
    /// of line without being flushed immediately. Unlike `build_trailing_line_doc`,
    /// this doesn't add a `line_suffix_boundary()` at the end.
    fn build_line_comments_no_boundary(&self, comments: &[&tsv_lang::Comment]) -> Doc;

    /// Get the source code string
    fn get_source(&self) -> &str;

    /// Get the tab width from config
    fn get_tab_width(&self) -> usize;

    /// Get the print width from config
    fn get_print_width(&self) -> usize;

    /// Check if chain expansion should be forced
    ///
    /// Used when inside template expressions with original breaks, where the
    /// expression is too long for the remaining print width.
    fn should_force_expand(&self) -> bool;

    /// Check if a doc fits in the available width (for chain break decisions)
    ///
    /// Used when deciding between args_break and chain_break states.
    /// Performs accurate width measurement via fits() with symbol resolution.
    fn fits_chain_tail(&self, doc: &Doc, available: usize) -> bool;
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
) -> Doc {
    match node {
        ChainNode::Base { expr, needs_parens } => {
            if *needs_parens {
                printer.print_parenthesized_base(expr)
            } else {
                printer.print_expression(expr)
            }
        }

        ChainNode::Call { expr, optional } => {
            if let Expression::CallExpression(call) = expr {
                if expanded {
                    printer.print_call_args_expanded(call, *optional)
                } else {
                    printer.print_call_args(call, *optional)
                }
            } else {
                doc::text("()")
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
            );

            // Block comments inside brackets: [/* c */ key] and [key /* c */]
            let inside_bracket_start = if *optional {
                bracket_open_pos + 3 // after `?.[`
            } else {
                bracket_open_pos + 1 // after `[`
            };
            let leading_comments_doc = printer.build_block_comments_doc(
                inside_bracket_start,
                prop_span.start,
                crate::printer::CommentSpacing::Trailing,
            );
            let trailing_comments_doc = printer.build_block_comments_doc(
                prop_span.end,
                *bracket_end,
                crate::printer::CommentSpacing::Leading,
            );

            let inner_with_comments =
                doc::concat(vec![leading_comments_doc, inner, trailing_comments_doc]);
            let bracket_doc = if *optional {
                doc::concat(vec![doc::text("?.["), inner_with_comments, doc::text("]")])
            } else {
                doc::brackets(inner_with_comments)
            };

            // Emit: pre-bracket line comments, pre-bracket block comments, then brackets
            doc::concat(vec![
                pre_trailing_line,
                pre_leading_line,
                pre_bracket_block_doc,
                bracket_doc,
            ])
        }

        ChainNode::NonNull => doc::text("!"),
    }
}

/// Print a single chain node (normal mode)
pub(crate) fn print_node<'a, P: ChainPrinter>(node: &ChainNode<'a>, printer: &P) -> Doc {
    print_node_inner(node, printer, false, false)
}

//
// Group Printing
//

/// Print a single chain group
pub(crate) fn print_group<'a, P: ChainPrinter>(group: &ChainGroup<'a>, printer: &P) -> Doc {
    print_group_inner(group, printer, false, false)
}

/// Print a chain group with forced call expansion
pub(crate) fn print_group_expanded<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> Doc {
    print_group_inner(group, printer, true, false)
}

/// Print a chain group, skipping block comments for the first member node
///
/// Used by ChainPartsBuilder in expanded path where `add_comments_and_break`
/// already handles comments for the first member (emitting before the line break).
pub(crate) fn print_group_skip_first_comments<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> Doc {
    print_group_inner(group, printer, false, true)
}

/// Print a chain group with forced call expansion, skipping block comments for the first member
pub(crate) fn print_group_expanded_skip_first_comments<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
) -> Doc {
    print_group_inner(group, printer, true, true)
}

/// Internal implementation for printing a chain group
fn print_group_inner<'a, P: ChainPrinter>(
    group: &ChainGroup<'a>,
    printer: &P,
    expanded: bool,
    skip_first_comments: bool,
) -> Doc {
    let docs: Vec<Doc> = group
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            // Skip comments only for the first member node
            let skip_comments = skip_first_comments && i == 0 && n.is_member();
            print_node_inner(n, printer, expanded, skip_comments)
        })
        .collect();
    doc::concat(docs)
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
) -> Doc {
    // Build member doc without format! allocation - use doc::symbol for deferred resolution
    let prop_id = property.to_u32();
    let member_doc = match (optional, is_private) {
        (false, false) => doc::concat(vec![doc::text("."), doc::symbol(prop_id)]),
        (true, false) => doc::concat(vec![doc::text("?."), doc::symbol(prop_id)]),
        (false, true) => doc::concat(vec![doc::text(".#"), doc::symbol(prop_id)]),
        (true, true) => doc::concat(vec![doc::text("?.#"), doc::symbol(prop_id)]),
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

    doc::concat(vec![
        trailing_block,
        trailing_line,
        leading_block,
        leading_line,
        member_doc,
    ])
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
        if let Some(new_i) = crate::printer::analysis::skip_comment(bytes, i, end_pos) {
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
) -> Doc {
    if !use_hardline {
        return doc::softline();
    }

    // Check for blank line preservation (only when no comments - comments handle their own spacing)
    // When there are comments between obj and property, the 2+ newlines (one before comment,
    // one after) should NOT be treated as a blank line.
    let line_breaks = printer.get_line_breaks();
    let has_comments = printer.has_comments_between(object_end, property_start);

    if !has_comments && has_blank_line_between_fast(line_breaks, object_end, property_start) {
        // Preserve blank line: literalline (no indent) + hardline (with indent for next content)
        doc::concat(vec![doc::literalline(), doc::hardline()])
    } else {
        doc::hardline()
    }
}
