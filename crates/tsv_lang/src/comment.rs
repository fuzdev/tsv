// Shared comment type and utilities used across languages
use crate::Span;
use crate::printing;
use smallvec::SmallVec;

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)] // independent flags + serializer hints, not a state machine
pub struct Comment {
    /// Byte span of the comment's content (delimiters excluded), into the
    /// source. The text is recovered on demand via [`Comment::content`] rather
    /// than stored owned — comments are a pure sub-slice of source (no decoding
    /// for JS/TS/CSS comments), so a span avoids a `String` allocation per
    /// comment in the lexer and the parser's collect-clone.
    pub content_span: Span,
    pub is_block: bool, // true for /* */ or <!-- -->, false for //
    /// Whether the content contains a `\n`. Precomputed at construction so the
    /// multi-line-block-comment expansion checks (here and in the printers) stay
    /// O(1) and source-free. Line comments never contain a newline, so this is
    /// only ever `true` for block comments.
    pub multiline: bool,
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
    /// Public-AST serializer hint (Svelte): bump this comment's JSON `loc`
    /// columns by one. Set for a comment collected inside a Svelte block
    /// pattern (`read_pattern`'s synthetic `(pattern = 1)` parse) on the
    /// pattern's start line when that line is `> 1` — the inserted `(` shifts
    /// the line's columns right by one, the comment sibling of the
    /// block-pattern node-`loc` quirk. The `end` column bumps only when the
    /// comment is single-line (a multiline block comment ends on an unshifted
    /// later line).
    pub bump_pattern_columns: bool,
    /// Whether this comment is **bound to the token that follows it**, and so is printed by
    /// the AST node that token begins rather than by the enclosing gap. Set by `tsv_ts`'s
    /// parser; always a **block** comment, and only ever when glued to its token (a comment
    /// the author left on its own line leads the *line*, not the token — the one exception
    /// is a JSDoc cast, whose comment may sit a newline above its `(` and still be the cast).
    ///
    /// **Every glued block comment is owned**, not a special class: the position binds the
    /// comment to the operand it leads, so a paren the printer synthesizes around an enclosing
    /// expression would otherwise land between them and re-bind it. There is no content sniff —
    /// a plain `/* c */`, a **bundler annotation** (`/* @__PURE__ */ f()`, which marks the call
    /// after it side-effect-free), and a **JSDoc type cast** (`/** @type {T} */ (x)`, whose
    /// comment plus `(` *are* the cast) all bind their token the same way. Two print shapes:
    ///
    /// - the general glued comment (plain or annotation) has no node of its own, so the
    ///   innermost node its token begins prints it (via `build_expression_doc`);
    /// - the JSDoc cast is printed by its `JsdocCast` node, which carries its own copy.
    ///   `is_jsdoc_type_cast_comment` (the only surviving content sniff) decides cast
    ///   *paren-retention*, i.e. whether that node is built — never ownership itself.
    ///
    /// **Ownership is a fact about who PRINTS a comment, never about whether it EXISTS.**
    /// The lookups below make a caller name which of the three questions it is asking, and
    /// only the **to emit** one skips an owned comment — so no gap emitter can print it
    /// twice and no synthesized paren can land between it and its token, while every layout
    /// gate and source cursor still sees it. Getting that backwards is the recurring bug in
    /// this model; see the module docs below.
    ///
    /// Set this flag exclusively where the owning node also prints — **an owned comment
    /// nothing prints is a dropped comment.** What makes that safe to rely on is the
    /// print-once ledger (`crate::comment_ledger`, `comment_check` feature): it asserts that
    /// every parsed comment is emitted exactly once, so a broken ownership claim fails the
    /// `comments:audit` gate instead of silently deleting the comment.
    pub owned_by_node: bool,
}

impl Comment {
    /// The comment's content (delimiters excluded), sliced from `source`.
    ///
    /// `source` must be the same text the comment's spans were recorded
    /// against (the host document for embedded `<script>`/`{expr}` comments).
    #[inline]
    pub fn content<'s>(&self, source: &'s str) -> &'s str {
        self.content_span.extract(source)
    }
}

//
// Format-Ignore Directive Recognition
//
//
// A comment can suppress formatting of the construct that follows it. tsv
// recognizes its own tool-neutral `format-ignore` family as canonical and
// prettier's `prettier-ignore` family as a drop-in-compatible alias — both
// spellings are honored everywhere. These predicates are the single source of
// truth for the directive set, called by each language printer (the comment
// types differ across crates, so the shared atom operates on the trimmed text).

/// Whether `content` is a `format-ignore` / `prettier-ignore` directive — emit
/// the following construct as raw source instead of formatting it.
#[inline]
pub fn is_format_ignore_directive(content: &str) -> bool {
    matches!(content.trim(), "format-ignore" | "prettier-ignore")
}

/// Whether `content` opens an ignore range (`format-ignore-start` /
/// `prettier-ignore-start`). Everything through the matching range-end marker is
/// emitted as raw source.
#[inline]
pub fn is_format_ignore_range_start(content: &str) -> bool {
    matches!(
        content.trim(),
        "format-ignore-start" | "prettier-ignore-start"
    )
}

/// Whether `content` closes an ignore range (`format-ignore-end` /
/// `prettier-ignore-end`). See `is_format_ignore_range_start`.
#[inline]
pub fn is_format_ignore_range_end(content: &str) -> bool {
    matches!(content.trim(), "format-ignore-end" | "prettier-ignore-end")
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

        for comment in comments_to_emit_in_range(comments, start, end) {
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

    /// All leading (own-line) comments in source order, merging the `leading_block`
    /// and `leading_line` buckets.
    ///
    /// `from_range` splits leading comments by kind because chain printers emit the
    /// two runs separately (all blocks, then all lines). Callers that emit a gap's
    /// leading comments in authored order — ternary operand→operator gaps,
    /// call-argument gaps — use this instead, so an interleaved block/line sequence
    /// keeps the order the author wrote it. Each bucket is already source-sorted, so
    /// this is a linear two-way merge on `span.start`.
    pub fn leading_in_source_order(&self) -> SmallVec<[&'a Comment; 2]> {
        let (block, line) = (&self.leading_block, &self.leading_line);
        let mut out: SmallVec<[&'a Comment; 2]> = SmallVec::with_capacity(block.len() + line.len());
        let (mut bi, mut li) = (0, 0);
        while bi < block.len() && li < line.len() {
            if block[bi].span.start <= line[li].span.start {
                out.push(block[bi]);
                bi += 1;
            } else {
                out.push(line[li]);
                li += 1;
            }
        }
        out.extend_from_slice(&block[bi..]);
        out.extend_from_slice(&line[li..]);
        out
    }
}

//
// Comment Lookup: three questions, three names
//
// Comments are collected in order during lexing, so they are sorted by `span.start`.
// Every lookup below binary-searches to the range start: O(log n + k).
//
// [`Comment::owned_by_node`] takes a comment out of the *positional* model — the node
// its token begins prints it, from its own reference. **Ownership is a fact about who
// PRINTS a comment, never about whether it EXISTS.** A caller that conflates the two
// asks the wrong question and gets a wrong answer, so the API asks the caller to name
// the question. There are exactly three:
//
// | axis         | question                                          | owned comments |
// | ------------ | ------------------------------------------------- | -------------- |
// | **to emit**  | "which comments must *I* print here?"             | **skipped**    |
// | **on page**  | "does any comment OCCUPY THE PAGE here?"          | **counted**    |
// | **in source**| "what comment BYTES are physically here?"         | **counted**    |
//
// - **to emit** — a gap emitter. Skipping is what keeps an owned comment from being
//   printed twice, and keeps a synthesized paren from landing between it and its token.
// - **on page** — a layout gate (break / expand / hug / paren / fast-path / force-
//   multiline). An owned comment is *still in the output* and *still occupies width*, so
//   it means to the layout exactly what any comment means. Skipping it here makes the
//   comment vanish from a decision it is visibly part of.
// - **in source** — a cursor stepping over comment bytes: a blank-line scan, an offset,
//   a `prev_end`. The bytes are in the file regardless of who prints them; a scan that
//   skips them reads the comment's own newlines as an author's blank line.
//
// Naming rule: every name states its axis, so a miswire reads as a category error at the
// call site rather than as plausible code.

/// Find the index of the first comment with `span.start >= pos`.
///
/// Physical (a raw index into the sorted slice) — the shared entry point of all three
/// axes, which then apply their own owned-comment policy.
#[inline]
pub fn find_first_comment_from(comments: &[Comment], pos: u32) -> usize {
    comments.partition_point(|c| c.span.start < pos)
}

/// **to emit**: the comments in `[start, end)` that *this* caller must print.
///
/// [`Comment::owned_by_node`] comments are **skipped** — the node their token begins
/// prints them. Use this only to decide what to *emit*; for a layout decision use
/// [`has_comments_on_page_in_range`], and for a source cursor use
/// [`comments_in_source_range`].
#[inline]
pub fn comments_to_emit_in_range(
    comments: &[Comment],
    start: u32,
    end: u32,
) -> impl Iterator<Item = &Comment> {
    comments_in_source_range(comments, start, end).filter(|c| !c.owned_by_node)
}

/// **to emit**: whether this caller has any comment to print in `[start, end)`.
#[inline]
pub fn has_comments_to_emit_in_range(comments: &[Comment], start: u32, end: u32) -> bool {
    comments_to_emit_in_range(comments, start, end)
        .next()
        .is_some()
}

/// **to emit**: the comments at or after `pos` that *this* caller must print.
#[inline]
pub fn comments_to_emit_after(comments: &[Comment], pos: u32) -> impl Iterator<Item = &Comment> {
    comments_in_source_after(comments, pos).filter(|c| !c.owned_by_node)
}

/// **on page**: whether any comment occupies the page in `[start, end)` —
/// [`Comment::owned_by_node`] comments **counted**.
///
/// The existence check for a *layout* gate. An owned comment is printed by its own node
/// rather than by this gap, but it is still in the output and still occupies width, so a
/// decision about break / expand / hug / paren / fast-path must see it. Use
/// [`has_comments_to_emit_in_range`] for anything that decides who *prints*.
#[inline]
pub fn has_comments_on_page_in_range(comments: &[Comment], start: u32, end: u32) -> bool {
    let first_idx = find_first_comment_from(comments, start);
    comments.get(first_idx).is_some_and(|c| c.span.end <= end)
}

/// **on page**: every comment occupying the page in `[start, end)` —
/// [`Comment::owned_by_node`] comments **counted**.
///
/// The iterator form of [`has_comments_on_page_in_range`], for a layout gate whose rule is
/// per-comment (`.any(|c| …)`) rather than a bare existence check.
///
/// Note the shape of the model: there are **three questions but only two membership sets** —
/// *on page* and *in source* both count an owned comment (it is in the output and its bytes
/// are in the file), and only *to emit* skips it. So this is [`comments_in_source_range`] by
/// construction; the two names exist because the *question* differs, and a call site that
/// names the wrong one is the bug this API is shaped to prevent.
#[inline]
pub fn comments_on_page_in_range(
    comments: &[Comment],
    start: u32,
    end: u32,
) -> impl Iterator<Item = &Comment> {
    comments_in_source_range(comments, start, end)
}

/// **on page**: whether a multi-line block comment occupies the page in `[start, end)` —
/// [`Comment::owned_by_node`] comments **counted**.
///
/// A multi-line block comment forces the containing construct (array, object, conditional
/// type) to expand — a pure layout question, and an owned one forces it just the same.
#[inline]
pub fn has_multiline_block_comments_on_page_in_range(
    comments: &[Comment],
    start: u32,
    end: u32,
) -> bool {
    comments_in_source_range(comments, start, end).any(|c| c.is_block && c.multiline)
}

/// Whether any **line** comment lies in `[start, end)`.
///
/// Carries no axis in its name because it provably has none: a comment is owned only via
/// `bind_leading_comment` / the JSDoc cast, both of which take a **block** comment
/// (`glued_block_comment_index`), so `owned ⇒ is_block` and no line comment is ever
/// owned. Skipping and counting therefore agree here, on every axis, by construction.
/// (If a line comment ever becomes ownable, this function must grow an axis.)
#[inline]
pub fn has_line_comments_in_range(comments: &[Comment], start: u32, end: u32) -> bool {
    comments_in_source_range(comments, start, end).any(|c| !c.is_block)
}

/// **in source**: every comment physically inside `[start, end)` —
/// [`Comment::owned_by_node`] comments **counted**.
///
/// The lookup for a cursor stepping over comment *bytes*: a blank-line scan, an offset
/// computation, a `prev_end`. The bytes sit in the file regardless of who prints them, so
/// a scan that skipped an owned comment would read its own newlines as an author's blank
/// line.
#[inline]
pub fn comments_in_source_range(
    comments: &[Comment],
    start: u32,
    end: u32,
) -> impl Iterator<Item = &Comment> {
    let first_idx = find_first_comment_from(comments, start);
    comments[first_idx..]
        .iter()
        .take_while(move |c| c.span.end <= end)
}

/// **in source**: every comment physically at or after `pos` —
/// [`Comment::owned_by_node`] comments **counted**.
#[inline]
pub fn comments_in_source_after(comments: &[Comment], pos: u32) -> impl Iterator<Item = &Comment> {
    let first_idx = find_first_comment_from(comments, pos);
    comments[first_idx..].iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::printing::build_line_breaks;

    fn comment(start: u32, end: u32, is_block: bool, content: &str) -> Comment {
        Comment {
            // The lookup/classification tests exercise span-based logic only;
            // content_span mirrors the full span (no source to slice here).
            content_span: Span::new(start, end),
            is_block,
            multiline: content.contains('\n'),
            span: Span::new(start, end),
            emit_character_field: false,
            bump_pattern_columns: false,
            owned_by_node: false,
        }
    }

    #[test]
    fn format_ignore_directives_recognize_both_spellings() {
        // The tsv-native `format-ignore` family and prettier's `prettier-ignore`
        // family are both honored, with surrounding whitespace trimmed (block
        // comments arrive as ` format-ignore `).
        assert!(is_format_ignore_directive("format-ignore"));
        assert!(is_format_ignore_directive("prettier-ignore"));
        assert!(is_format_ignore_directive("  format-ignore  "));
        assert!(!is_format_ignore_directive("format-ignore-start"));
        assert!(!is_format_ignore_directive("eslint-disable"));

        assert!(is_format_ignore_range_start("format-ignore-start"));
        assert!(is_format_ignore_range_start("prettier-ignore-start"));
        assert!(!is_format_ignore_range_start("format-ignore"));
        assert!(!is_format_ignore_range_start("format-ignore-end"));

        assert!(is_format_ignore_range_end("format-ignore-end"));
        assert!(is_format_ignore_range_end("prettier-ignore-end"));
        assert!(!is_format_ignore_range_end("format-ignore"));
        assert!(!is_format_ignore_range_end("format-ignore-start"));
    }

    #[test]
    fn comments_to_emit_in_range_respects_start_and_end_boundaries() {
        let comments = vec![
            comment(0, 2, true, "a"),
            comment(5, 7, true, "b"),
            comment(10, 12, true, "c"),
        ];

        // [5, 12] includes the comments starting at 5 and 10 (both end <= 12).
        let starts: Vec<u32> = comments_to_emit_in_range(&comments, 5, 12)
            .map(|c| c.span.start)
            .collect();
        assert_eq!(starts, vec![5, 10]);

        // Tightening `end` to 11 drops the [10,12) comment (its end 12 > 11) —
        // the `take_while(end <= end)` bound, not a filter.
        let starts: Vec<u32> = comments_to_emit_in_range(&comments, 5, 11)
            .map(|c| c.span.start)
            .collect();
        assert_eq!(starts, vec![5]);

        // Raising `start` past a comment excludes it via the binary-search entry.
        let starts: Vec<u32> = comments_to_emit_in_range(&comments, 6, 12)
            .map(|c| c.span.start)
            .collect();
        assert_eq!(starts, vec![10]);
    }

    #[test]
    fn has_comments_to_emit_in_range_agrees_with_iterator() {
        let comments = vec![comment(0, 2, false, "a"), comment(5, 7, false, "b")];
        for (start, end) in [(0, 2), (0, 7), (3, 7), (3, 6), (6, 7), (0, 1)] {
            assert_eq!(
                has_comments_to_emit_in_range(&comments, start, end),
                comments_to_emit_in_range(&comments, start, end)
                    .next()
                    .is_some(),
                "range {start}..{end}"
            );
        }
    }

    #[test]
    fn has_comments_to_emit_in_range_shortcut_only_inspects_first_comment() {
        // A multi-line block comment whose end overruns the query window: the
        // O(log n) shortcut returns false because the first comment at/after
        // `start` ends past `end`, and the iterator agrees (take_while stops there).
        let comments = vec![comment(5, 40, true, "*\n big\n ")];
        assert!(!has_comments_to_emit_in_range(&comments, 5, 10));
        assert!(comments_to_emit_in_range(&comments, 5, 10).next().is_none());
    }

    #[test]
    fn line_and_multiline_block_predicates() {
        let block_ml = comment(0, 10, true, "a\nb");
        let block_sl = comment(0, 6, true, "a");
        let line = comment(0, 4, false, " x");

        assert!(has_multiline_block_comments_on_page_in_range(
            std::slice::from_ref(&block_ml),
            0,
            10
        ));
        assert!(!has_multiline_block_comments_on_page_in_range(
            std::slice::from_ref(&block_sl),
            0,
            6
        ));
        assert!(!has_multiline_block_comments_on_page_in_range(
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
    fn leading_in_source_order_merges_interleaved_block_and_line() {
        // Each leading bucket is source-sorted; the merge must restore authored order
        // across an interleaved line / block / line sequence.
        let line1 = comment(2, 8, false, " l1");
        let block = comment(15, 22, true, " b ");
        let line2 = comment(30, 36, false, " l2");
        let classified = ClassifiedComments {
            trailing_block: SmallVec::new(),
            trailing_line: SmallVec::new(),
            leading_block: SmallVec::from_slice(&[&block]),
            leading_line: SmallVec::from_slice(&[&line1, &line2]),
        };
        let order: Vec<u32> = classified
            .leading_in_source_order()
            .iter()
            .map(|c| c.span.start)
            .collect();
        assert_eq!(order, vec![2, 15, 30]);

        // Single-bucket inputs pass through unchanged.
        let only_line = ClassifiedComments {
            leading_line: SmallVec::from_slice(&[&line1, &line2]),
            ..Default::default()
        };
        let starts: Vec<u32> = only_line
            .leading_in_source_order()
            .iter()
            .map(|c| c.span.start)
            .collect();
        assert_eq!(starts, vec![2, 30]);
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
