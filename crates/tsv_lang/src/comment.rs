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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::printing::build_line_breaks;

    fn comment(start: u32, end: u32, is_block: bool, content: &str) -> Comment {
        Comment {
            content: content.to_string(),
            is_block,
            span: Span::new(start, end),
            emit_character_field: false,
        }
    }

    #[test]
    fn comments_in_range_respects_start_and_end_boundaries() {
        let comments = vec![
            comment(0, 2, true, "a"),
            comment(5, 7, true, "b"),
            comment(10, 12, true, "c"),
        ];

        // [5, 12] includes the comments starting at 5 and 10 (both end <= 12).
        let starts: Vec<u32> = comments_in_range(&comments, 5, 12)
            .map(|c| c.span.start)
            .collect();
        assert_eq!(starts, vec![5, 10]);

        // Tightening `end` to 11 drops the [10,12) comment (its end 12 > 11) —
        // the `take_while(end <= end)` bound, not a filter.
        let starts: Vec<u32> = comments_in_range(&comments, 5, 11)
            .map(|c| c.span.start)
            .collect();
        assert_eq!(starts, vec![5]);

        // Raising `start` past a comment excludes it via the binary-search entry.
        let starts: Vec<u32> = comments_in_range(&comments, 6, 12)
            .map(|c| c.span.start)
            .collect();
        assert_eq!(starts, vec![10]);
    }

    #[test]
    fn has_comments_in_range_agrees_with_iterator() {
        let comments = vec![comment(0, 2, false, "a"), comment(5, 7, false, "b")];
        for (start, end) in [(0, 2), (0, 7), (3, 7), (3, 6), (6, 7), (0, 1)] {
            assert_eq!(
                has_comments_in_range(&comments, start, end),
                comments_in_range(&comments, start, end).next().is_some(),
                "range {start}..{end}"
            );
        }
    }

    #[test]
    fn has_comments_in_range_shortcut_only_inspects_first_comment() {
        // A multi-line block comment whose end overruns the query window: the
        // O(log n) shortcut returns false because the first comment at/after
        // `start` ends past `end`, and the iterator agrees (take_while stops there).
        let comments = vec![comment(5, 40, true, "*\n big\n ")];
        assert!(!has_comments_in_range(&comments, 5, 10));
        assert!(comments_in_range(&comments, 5, 10).next().is_none());
    }

    #[test]
    fn line_and_multiline_block_predicates() {
        let block_ml = comment(0, 10, true, "a\nb");
        let block_sl = comment(0, 6, true, "a");
        let line = comment(0, 4, false, " x");

        assert!(has_multiline_block_comments_in_range(
            std::slice::from_ref(&block_ml),
            0,
            10
        ));
        assert!(!has_multiline_block_comments_in_range(
            std::slice::from_ref(&block_sl),
            0,
            6
        ));
        assert!(!has_multiline_block_comments_in_range(
            std::slice::from_ref(&line),
            0,
            4
        ));

        assert!(has_line_comments_in_range(
            std::slice::from_ref(&line),
            0,
            4
        ));
        assert!(!has_line_comments_in_range(
            std::slice::from_ref(&block_sl),
            0,
            6
        ));
    }

    #[test]
    fn classify_comment_slow_and_fast_agree() {
        // Offsets: 'x'=0, "// trail"=[2,10), '\n'=10, "/* own */"=[11,20),
        // '\n'=20, "/* inline */"=[21,33), ' '=33, 'y'=34.
        let source = "x // trail\n/* own */\n/* inline */ y";
        let breaks = build_line_breaks(source);
        let line = comment(2, 10, false, " trail");
        let own = comment(11, 20, true, " own ");
        let inline = comment(21, 33, true, " inline ");

        // prev_end = 1 (after 'x'), curr_start = 34 (the 'y').
        assert_eq!(
            classify_comment(&line, 1, 34, source),
            CommentPosition::Trailing
        );
        assert_eq!(
            classify_comment(&own, 1, 34, source),
            CommentPosition::LeadingOwnLine
        );
        assert_eq!(
            classify_comment(&inline, 1, 34, source),
            CommentPosition::LeadingInline
        );

        // The precomputed-line-breaks variant must never disagree with the
        // source-scanning one.
        for c in [&line, &own, &inline] {
            assert_eq!(
                classify_comment(c, 1, 34, source),
                classify_comment_fast(c, 1, 34, &breaks),
                "slow/fast disagree for span {:?}",
                c.span
            );
        }
    }
}
