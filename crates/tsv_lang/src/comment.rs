// Shared comment type and utilities used across languages
use crate::Span;
use crate::printing;
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub struct Comment {
    pub content: String,
    pub is_block: bool, // true for /* */ or <!-- -->, false for //
    pub span: Span,
    /// Public-AST serializer hint: when true, the JSON `loc` for this comment
    /// includes a `character` (byte-offset) field alongside `line`/`column`.
    /// Set by parsers that emit comments matching Svelte's template open-tag
    /// shape; cleared for comments inside `<script>`/expressions/CSS that
    /// follow the standard Svelte/acorn shape.
    //
    // TODO: this serializer flag is a stopgap for the detached-comment model.
    // Once an LSP/linter consumer arrives, promote to a structural attachment
    // (a parallel comment collection on the language root, or per-element
    // attachment if a richer model is needed).
    pub emit_character_field: bool,
}

//
// Comment Classification
//
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

/// Classify a comment's position using precomputed line breaks (O(log n)).
///
/// This is the optimized version of [`classify_comment`] that uses binary search
/// on a precomputed line breaks table instead of scanning the source string.
///
/// # Arguments
///
/// * `comment` - The comment to classify
/// * `prev_end` - End position of the previous element
/// * `curr_start` - Start position of the next element
/// * `line_breaks` - Sorted slice of newline byte offsets
///
/// # Returns
///
/// The comment's position classification.
#[inline]
pub fn classify_comment_fast(
    comment: &Comment,
    prev_end: u32,
    curr_start: u32,
    line_breaks: &[u32],
) -> CommentPosition {
    // Check if trailing (same line as prev_end)
    if printing::is_same_line_fast(line_breaks, prev_end, comment.span.start) {
        return CommentPosition::Trailing;
    }

    // Check if inline leading (same line as curr_start)
    if printing::is_same_line_fast(line_breaks, comment.span.end, curr_start) {
        return CommentPosition::LeadingInline;
    }

    // Otherwise, it's on its own line
    CommentPosition::LeadingOwnLine
}

/// Comments classified by position and type in a single pass.
///
/// Used by chain printers to avoid multiple binary searches per chain segment.
/// Instead of calling 4 separate filter functions (block/line × trailing/leading),
/// this struct collects all comments in O(log n + k) time.
#[derive(Debug, Default)]
pub struct ClassifiedComments<'a> {
    /// Block comments on same line as prev_end (trailing position)
    pub trailing_block: SmallVec<[&'a Comment; 2]>,
    /// Line comments on same line as prev_end (trailing position)
    pub trailing_line: SmallVec<[&'a Comment; 2]>,
    /// Block comments on their own line (leading position)
    pub leading_block: SmallVec<[&'a Comment; 2]>,
    /// Line comments on their own line (leading position)
    pub leading_line: SmallVec<[&'a Comment; 2]>,
}

impl<'a> ClassifiedComments<'a> {
    /// Classify all comments in a range using a single binary search.
    ///
    /// This is more efficient than calling separate filter functions when you need
    /// multiple comment categories from the same range.
    ///
    /// # Arguments
    ///
    /// * `comments` - All comments sorted by span.start
    /// * `start` - Start position (e.g., end of previous chain element)
    /// * `end` - End position (e.g., start of next chain element)
    /// * `line_breaks` - Precomputed line break positions for O(log n) same-line checks
    ///
    /// # Complexity
    ///
    /// O(log n + k) where n is total comments and k is comments in range.
    /// Compared to 4 separate filter calls which would be O(4 log n + 4k).
    pub fn from_range(comments: &'a [Comment], start: u32, end: u32, line_breaks: &[u32]) -> Self {
        let mut result = Self::default();

        for comment in comments_in_range(comments, start, end) {
            let same_line = printing::is_same_line_fast(line_breaks, start, comment.span.start);
            match (comment.is_block, same_line) {
                (true, true) => result.trailing_block.push(comment),
                (false, true) => result.trailing_line.push(comment),
                (true, false) => result.leading_block.push(comment),
                (false, false) => result.leading_line.push(comment),
            }
        }

        result
    }

    /// Check if all buckets are empty (no comments in range).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.trailing_block.is_empty()
            && self.trailing_line.is_empty()
            && self.leading_block.is_empty()
            && self.leading_line.is_empty()
    }
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

//
// Efficient Comment Lookup Utilities
//
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

/// Check if any multi-line block comments exist in the range [start, end)
///
/// Multi-line block comments contain newlines in their content and force
/// expansion of containing constructs (arrays, objects, etc.).
/// Uses binary search: O(log n + k) where k is comments in range
#[inline]
pub fn has_multiline_block_comments_in_range(comments: &[Comment], start: u32, end: u32) -> bool {
    comments_in_range(comments, start, end).any(|c| c.is_block && c.content.contains('\n'))
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
