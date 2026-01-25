// Comment handling for call expression arguments
//
// Handles detection and partitioning of comments in argument lists:
// - Inter-argument comments (between arguments)
// - Trailing comments on arguments
// - Leading comments before arguments

use smallvec::SmallVec;

use super::super::Printer;
use crate::ast::internal;

//
// Comma-relative comment helpers
//

/// Find the comma position between two argument spans
///
/// Returns the absolute position of the comma in the source, or None if not found.
#[inline]
pub(super) fn find_comma_pos(source: &str, start: u32, end: u32) -> Option<usize> {
    let between = &source[start as usize..end as usize];
    between.find(',').map(|offset| start as usize + offset)
}

/// Check if a comment is before the comma position
#[inline]
pub(super) fn is_comment_before_comma(comment: &internal::Comment, comma_pos: usize) -> bool {
    (comment.span.start as usize) < comma_pos
}

/// Check if a comment is after the comma position
#[inline]
pub(super) fn is_comment_after_comma(comment: &internal::Comment, comma_pos: usize) -> bool {
    (comment.span.start as usize) > comma_pos
}

/// Check if a comment is an inline block comment before the comma
///
/// Returns true if the comment is:
/// - A block comment (not line comment)
/// - Positioned before the comma
/// - On the same line as `ref_pos` (typically the previous arg's end)
#[inline]
pub(super) fn is_inline_block_before_comma(
    comment: &internal::Comment,
    comma_pos: usize,
    line_breaks: &[u32],
    ref_pos: u32,
) -> bool {
    comment.is_block
        && is_comment_before_comma(comment, comma_pos)
        && tsv_lang::printing::is_same_line_fast(line_breaks, ref_pos, comment.span.start)
}

/// Check if a comment is an inline block comment after the comma
///
/// Returns true if the comment is:
/// - A block comment (not line comment)
/// - Positioned after the comma
/// - On the same line as `ref_pos` (typically the previous arg's end)
#[inline]
pub(super) fn is_inline_block_after_comma(
    comment: &internal::Comment,
    comma_pos: usize,
    line_breaks: &[u32],
    ref_pos: u32,
) -> bool {
    comment.is_block
        && is_comment_after_comma(comment, comma_pos)
        && tsv_lang::printing::is_same_line_fast(line_breaks, ref_pos, comment.span.start)
}

//
// Inter-argument comment detection
//

/// Check if a call expression has comments between any of its arguments
pub(super) fn has_inter_argument_comments(
    call: &internal::CallExpression,
    printer: &Printer,
) -> bool {
    has_inter_argument_comments_slice(&call.arguments, printer)
}

/// Check if there are comments between arguments in a slice
pub(crate) fn has_inter_argument_comments_slice(
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
pub(super) fn should_force_expansion_for_comments(printer: &Printer, start: u32, end: u32) -> bool {
    printer.has_line_comments_between(start, end) || printer.has_newline_before_comment(start, end)
}

/// Check if any comments in a call's arguments force expansion
///
/// Checks leading comments (before first arg), inter-argument comments, and trailing comments.
/// Returns true if:
/// - Any line comments exist
/// - Block comments are on their own line
/// - Block comments are AFTER the comma (semantically leading on next arg)
///
/// Inline block comments BEFORE the comma (trailing on current arg) do not force expansion.
pub(super) fn any_comment_forces_expansion(
    call: &internal::CallExpression,
    printer: &Printer,
    paren_open: u32,
) -> bool {
    if call.arguments.is_empty() {
        return false;
    }

    // Check leading comments before first arg
    let first_arg_start = call.arguments[0].span().start;
    if printer.has_comments_between(paren_open, first_arg_start)
        && should_force_expansion_for_comments(printer, paren_open, first_arg_start)
    {
        return true;
    }

    // Check inter-argument and trailing comments
    for (i, arg) in call.arguments.iter().enumerate() {
        let arg_end = arg.span().end;
        let next_boundary = if i < call.arguments.len() - 1 {
            call.arguments[i + 1].span().start
        } else {
            call.span.end
        };

        if !printer.has_comments_between(arg_end, next_boundary) {
            continue;
        }

        // Line comments or block comments on own line always force expansion
        if should_force_expansion_for_comments(printer, arg_end, next_boundary) {
            return true;
        }

        // For inter-argument block comments (not trailing after last arg),
        // check if ANY comment is AFTER the comma (leading on next arg).
        // If so, it forces expansion even if it's an inline block comment.
        if i < call.arguments.len() - 1
            && let Some(comma_pos) = find_comma_pos(printer.source, arg_end, next_boundary) {
                for comment in tsv_lang::comments_in_range(printer.comments, arg_end, next_boundary)
                {
                    if is_comment_after_comma(comment, comma_pos) {
                        return true;
                    }
                }
            }
    }

    false
}

/// Check if there are trailing line comments on any arguments
///
/// A trailing comment is one that appears after an argument's expression,
/// either between the arg and its comma, or between the last arg and the closing paren.
/// Example: `fn(a && b, // trailing)` - the `// trailing` is a trailing comment on `a && b`
pub(super) fn has_trailing_comments_on_args(
    call: &internal::CallExpression,
    printer: &Printer,
) -> bool {
    has_trailing_line_comments_slice(&call.arguments, call.span.end, printer)
}

/// Check if there are trailing line comments on any arguments (generic version)
///
/// Used by both CallExpression and NewExpression.
pub(crate) fn has_trailing_line_comments_slice(
    arguments: &[internal::Expression],
    call_span_end: u32,
    printer: &Printer,
) -> bool {
    if arguments.is_empty() {
        return false;
    }

    for (i, arg) in arguments.iter().enumerate() {
        let arg_end = arg.span().end;
        let next_boundary = if i < arguments.len() - 1 {
            arguments[i + 1].span().start
        } else {
            call_span_end
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
///
/// Uses `SmallVec` to avoid heap allocations for the common case (0-2 comments per range).
pub(crate) struct PartitionedComments<'a> {
    pub trailing_line: SmallVec<[&'a internal::Comment; 2]>,
    pub trailing_block: SmallVec<[&'a internal::Comment; 2]>,
    pub leading: SmallVec<[&'a internal::Comment; 2]>,
}

impl<'a> PartitionedComments<'a> {
    /// Partition comments in a range based on their position relative to `start`
    ///
    /// Comments on the same line as `start` are "trailing" (they follow content on that line).
    /// Comments on subsequent lines are "leading" (they precede content on the next line).
    pub fn new(
        comments: &'a [internal::Comment],
        line_breaks: &[u32],
        start: u32,
        end: u32,
    ) -> Self {
        let mut trailing_line = SmallVec::new();
        let mut trailing_block = SmallVec::new();
        let mut leading = SmallVec::new();

        for comment in tsv_lang::comments_in_range(comments, start, end) {
            if tsv_lang::printing::is_same_line_fast(line_breaks, start, comment.span.start) {
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

    pub fn has_trailing_line(&self) -> bool {
        !self.trailing_line.is_empty()
    }

    pub fn has_trailing_block(&self) -> bool {
        !self.trailing_block.is_empty()
    }

    #[allow(dead_code)]
    pub fn has_leading(&self) -> bool {
        !self.leading.is_empty()
    }

    /// Check if there are any comments at all
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.trailing_line.is_empty() && self.trailing_block.is_empty() && self.leading.is_empty()
    }

    /// Emit trailing comments (block then line) with leading spaces to a parts vector.
    ///
    /// Used for comments that follow an argument, formatted as ` /* block */ // line`.
    pub fn emit_trailing_comments(&self, parts: &mut Vec<tsv_lang::doc::Doc>, printer: &Printer) {
        for comment in &self.trailing_block {
            parts.push(tsv_lang::doc::text(" "));
            parts.push(printer.build_comment_doc(comment));
        }
        for comment in &self.trailing_line {
            parts.push(tsv_lang::doc::text(" "));
            parts.push(printer.build_comment_doc(comment));
        }
    }

    /// Emit leading comments (on their own lines) with hardlines after each.
    ///
    /// Used for comments that precede an argument on separate lines.
    pub fn emit_leading_comments(&self, parts: &mut Vec<tsv_lang::doc::Doc>, printer: &Printer) {
        for comment in &self.leading {
            parts.push(printer.build_comment_doc(comment));
            parts.push(tsv_lang::doc::hardline());
        }
    }

    /// Emit leading comments, keeping inline block comments on the same line as `next_pos`.
    ///
    /// For comments on the same line as `next_pos`, emits them inline (comment + space).
    /// For comments on their own line, emits them with hardline after.
    pub fn emit_leading_comments_inline_aware(
        &self,
        parts: &mut Vec<tsv_lang::doc::Doc>,
        printer: &Printer,
        next_pos: u32,
    ) {
        for comment in &self.leading {
            parts.push(printer.build_comment_doc(comment));
            // If comment is on same line as next element, keep it inline
            if comment.is_block
                && tsv_lang::printing::is_same_line_fast(
                    printer.line_breaks,
                    comment.span.end,
                    next_pos,
                )
            {
                parts.push(tsv_lang::doc::text(" "));
            } else {
                parts.push(tsv_lang::doc::hardline());
            }
        }
    }
}
