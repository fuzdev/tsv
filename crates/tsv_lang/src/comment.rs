// Shared comment type and utilities used across languages
use crate::Span;

#[derive(Debug, Clone)]
pub struct Comment {
    pub content: String,
    pub is_block: bool, // true for /* */ or <!-- -->, false for //
    pub span: Span,
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
