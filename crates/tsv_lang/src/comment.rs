// Shared comment type and utilities used across languages
use crate::Span;
use crate::printing;

#[derive(Debug, Clone)]
pub struct Comment {
    pub content: String,
    pub is_block: bool, // true for /* */ or <!-- -->, false for //
    pub span: Span,
}

// ============================================================================
// Comment Classification
// ============================================================================
//
// Comments between two nodes can be classified as:
// - Trailing: on same line as prev_end (belongs to previous node)
// - LeadingOwnLine: on its own line(s) before curr_start
// - LeadingInline: on same line as curr_start (inline before next node)

/// Classify a comment's relationship to surrounding nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentPosition {
    /// On same line as prev_end (trailing comment for previous node)
    Trailing,
    /// On own line(s) between nodes (leading comment for next node)
    LeadingOwnLine,
    /// On same line as curr_start (inline leading: `/* c */ node`)
    LeadingInline,
}

/// Classify a comment's position relative to prev_end and curr_start.
///
/// # Arguments
///
/// * `comment` - The comment to classify
/// * `prev_end` - End position of the previous element
/// * `curr_start` - Start position of the next element
/// * `source` - The source text
///
/// # Returns
///
/// The comment's position classification.
pub fn classify_comment(
    comment: &Comment,
    prev_end: u32,
    curr_start: u32,
    source: &str,
) -> CommentPosition {
    // Check if trailing (same line as prev_end)
    if printing::is_same_line(source, prev_end, comment.span.start) {
        return CommentPosition::Trailing;
    }

    // Check if inline leading (same line as curr_start)
    if printing::is_same_line(source, comment.span.end, curr_start) {
        return CommentPosition::LeadingInline;
    }

    // Otherwise, it's on its own line
    CommentPosition::LeadingOwnLine
}

/// Iterate over leading comments (excludes trailing).
///
/// Returns (comment, is_inline) pairs where is_inline is true for comments
/// on the same line as curr_start.
///
/// # Arguments
///
/// * `comments` - All comments sorted by span.start
/// * `prev_end` - End position of the previous element
/// * `curr_start` - Start position of the next element
/// * `source` - The source text
///
/// # Returns
///
/// An iterator yielding (comment, is_inline) pairs for leading comments only.
///
/// Uses binary search to find the starting point: O(log n + k) where k is result count.
pub fn leading_comments<'a>(
    comments: &'a [Comment],
    prev_end: u32,
    curr_start: u32,
    source: &'a str,
) -> impl Iterator<Item = (&'a Comment, bool)> + 'a {
    comments_in_range(comments, prev_end, curr_start).filter_map(move |comment| {
        match classify_comment(comment, prev_end, curr_start, source) {
            CommentPosition::Trailing => None,
            CommentPosition::LeadingOwnLine => Some((comment, false)),
            CommentPosition::LeadingInline => Some((comment, true)),
        }
    })
}

/// Iterate over trailing comments only.
///
/// Returns comments that are on the same line as prev_end.
///
/// # Arguments
///
/// * `comments` - All comments sorted by span.start
/// * `prev_end` - End position of the previous element
/// * `curr_start` - Start position of the next element
/// * `source` - The source text
///
/// # Returns
///
/// An iterator yielding trailing comments only.
///
/// Uses binary search to find the starting point: O(log n + k) where k is result count.
pub fn trailing_comments<'a>(
    comments: &'a [Comment],
    prev_end: u32,
    curr_start: u32,
    source: &'a str,
) -> impl Iterator<Item = &'a Comment> + 'a {
    comments_in_range(comments, prev_end, curr_start).filter(move |comment| {
        matches!(
            classify_comment(comment, prev_end, curr_start, source),
            CommentPosition::Trailing
        )
    })
}

// ============================================================================
// Efficient Comment Lookup Utilities
// ============================================================================
//
// Comments are collected in order during lexing, so they're naturally sorted
// by span.start. These functions use binary search for O(log n) range lookups.

/// Find the index of the first comment with span.start >= pos
///
/// Uses binary search: O(log n)
#[inline]
pub fn find_first_comment_from(comments: &[Comment], pos: u32) -> usize {
    comments.partition_point(|c| c.span.start < pos)
}

/// Iterate over comments in the range [start, end)
///
/// Returns an iterator over comments where start <= span.start && span.end <= end.
/// Uses binary search to find the starting point: O(log n + k) where k is result count.
#[inline]
pub fn comments_in_range(
    comments: &[Comment],
    start: u32,
    end: u32,
) -> impl Iterator<Item = &Comment> {
    let first_idx = find_first_comment_from(comments, start);
    comments[first_idx..]
        .iter()
        .take_while(move |c| c.span.end <= end)
}

/// Check if any comments exist in the range [start, end)
///
/// Uses binary search: O(log n)
#[inline]
pub fn has_comments_in_range(comments: &[Comment], start: u32, end: u32) -> bool {
    let first_idx = find_first_comment_from(comments, start);
    comments.get(first_idx).is_some_and(|c| c.span.end <= end)
}

/// Check if any line comments exist in the range [start, end)
///
/// Uses binary search: O(log n + k) where k is comments in range
#[inline]
pub fn has_line_comments_in_range(comments: &[Comment], start: u32, end: u32) -> bool {
    comments_in_range(comments, start, end).any(|c| !c.is_block)
}

/// Iterate over comments after a position (span.start >= pos)
///
/// Returns an iterator over all comments starting at or after the given position.
/// Uses binary search to find the starting point: O(log n + k) where k is result count.
#[inline]
pub fn comments_after(comments: &[Comment], pos: u32) -> impl Iterator<Item = &Comment> {
    let first_idx = find_first_comment_from(comments, pos);
    comments[first_idx..].iter()
}
