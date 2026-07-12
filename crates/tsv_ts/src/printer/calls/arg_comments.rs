// Comment handling for call expression arguments
//
// Handles detection and partitioning of comments in argument lists:
// - Inter-argument comments (between arguments)
// - Trailing comments on arguments
// - Leading comments before arguments

use smallvec::SmallVec;

use super::super::Printer;
use crate::ast::internal;
use tsv_lang::comments_in_range;
use tsv_lang::doc::DocBuf;

impl<'a> Printer<'a> {
    /// Open a non-last argument gap that may carry comments and emit its head into
    /// `parts`: partition the comments, reclassify an after-comma block that **hugs**
    /// the next arg as leading (`C`) while a **stranded** one stays on the comma line
    /// (`A`), then emit the before-comma blocks, the comma, the stranded after-comma
    /// blocks, and the same-line line comment.
    ///
    /// Returns the routed [`PartitionedComments`] so the caller supplies the rest of the
    /// gap — its own separator policy (soft vs. hard line, blank-line preservation, any
    /// force-expansion feedback), then the next arg's leading comments via
    /// [`PartitionedComments::emit_leading_comments_inline_aware`]. Every per-argument
    /// loop (`call`, `new`, member-chain, and the wrapping helpers) shares this head so
    /// the route-then-emit ordering — and the respect-the-newline rule it encodes —
    /// lives in one place; only the separator, which genuinely differs per layout, stays
    /// at the call site.
    pub(super) fn open_inter_arg_gap(
        &self,
        parts: &mut DocBuf,
        arg_end: u32,
        next_arg_start: u32,
    ) -> PartitionedComments<'a> {
        let mut pc = PartitionedComments::new(
            self.comments,
            self.comment_line_breaks,
            arg_end,
            next_arg_start,
        );
        pc.route_after_comma_hugging_to_leading(self);
        pc.emit_trailing_comments_around_comma(parts, self);
        pc
    }
}

//
// Comma-relative comment helpers
//

/// Find the comma position between two argument spans
///
/// Returns the absolute position of the separating comma in the source, or None
/// if not found. Commas inside comments are skipped: the gap between two argument
/// expressions only ever holds whitespace, comments, stripped parens, and the
/// separating comma — never strings or code — so skipping `/* … */` and `// …`
/// spans is enough to avoid mistaking a comment-internal comma (`a /* p, q */, b`)
/// for the separator.
#[inline]
pub(crate) fn find_comma_pos(source: &str, start: u32, end: u32) -> Option<usize> {
    // Byte scan is safe: `,`, `/`, `*`, `\n` are ASCII and never appear as a
    // UTF-8 continuation byte, so multibyte content in a comment can't false-match.
    let bytes = source.as_bytes();
    let (s, e) = (start as usize, end as usize);
    let mut i = s;
    while i < e {
        match bytes[i] {
            b',' => return Some(i),
            b'/' if i + 1 < e && bytes[i + 1] == b'*' => {
                // Skip a block comment, including its internal commas.
                i += 2;
                while i + 1 < e && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'/' if i + 1 < e && bytes[i + 1] == b'/' => {
                // Skip a line comment to end of line.
                i += 2;
                while i < e && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    None
}

/// Find the effective start position for blank-line checking before an arg.
///
/// When grouping parens are stripped (e.g., `(expr)` → `expr`), the expression's
/// span starts after the `(`, but the source between a comma and the expression
/// may contain `(\n\t\texpr` — two newlines that look like a blank line.
/// This scans from `from` toward `to` and skips past any opening `(` that's the
/// first non-whitespace character, returning the position after it.
///
/// Callers must pass `from <= to`.
#[inline]
pub(crate) fn skip_stripped_open_paren(source: &str, from: u32, to: u32) -> u32 {
    let slice = &source[from as usize..to as usize];
    for (i, byte) in slice.bytes().enumerate() {
        if byte == b'(' {
            return from + i as u32 + 1;
        }
        if !byte.is_ascii_whitespace() {
            break;
        }
    }
    to
}

/// Check for a blank line between two consecutive call arguments, accounting
/// for stripped grouping parens on both sides.
///
/// Uses `find_comma_pos` to skip past the closing `)` gap after the previous arg,
/// and `skip_stripped_open_paren` to skip past the opening `(` gap before the next arg.
#[inline]
pub(crate) fn has_blank_line_between_args(
    source: &str,
    line_breaks: &[u32],
    prev_end: u32,
    curr_start: u32,
) -> bool {
    let check_start =
        find_comma_pos(source, prev_end, curr_start).map_or(prev_end, |c| c as u32 + 1);
    let check_end = skip_stripped_open_paren(source, check_start, curr_start);
    tsv_lang::printing::has_blank_line_between_fast(line_breaks, check_start, check_end)
}

/// Check if a comment is before the comma position
#[inline]
pub(crate) fn is_comment_before_comma(comment: &internal::Comment, comma_pos: usize) -> bool {
    (comment.span.start as usize) < comma_pos
}

/// Check if a comment is after the comma position
#[inline]
pub(crate) fn is_comment_after_comma(comment: &internal::Comment, comma_pos: usize) -> bool {
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
    call: &internal::CallExpression<'_>,
    printer: &Printer<'_>,
) -> bool {
    has_inter_argument_comments_slice(call.arguments, printer)
}

/// Check if there are comments between arguments in a slice
pub(crate) fn has_inter_argument_comments_slice(
    arguments: &[internal::Expression<'_>],
    printer: &Printer<'_>,
) -> bool {
    if arguments.len() < 2 {
        return false;
    }

    arguments
        .windows(2)
        .any(|pair| printer.has_comments_between(pair[0].span().end, pair[1].span().start))
}

/// Check if the gap between two source positions contains only whitespace and parens,
/// with the first paren on the same line as `start`.
///
/// Detects stripped grouping parens: `/** @type {T} */ (\n\texpr)` → after stripping,
/// the gap between `*/` and `expr` is ` (\n\t` (whitespace + parens). The opening
/// paren is on the same line as the comment, so these should be treated as inline.
///
/// Returns false when the paren is on a different line from the comment:
/// `/* block */\n(expr)` → gap `\n(` has a newline before the paren → NOT inline.
fn has_stripped_paren_gap(source: &str, start: u32, end: u32) -> bool {
    let s = start as usize;
    let e = end as usize;
    if s >= e || e > source.len() {
        return false;
    }
    let gap = &source[s..e];
    // All bytes must be whitespace or parens
    if !gap
        .bytes()
        .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'(' | b')'))
    {
        return false;
    }
    // Must have a paren, and no newline before it (comment and paren on same line)
    match gap.bytes().position(|b| b == b'(' || b == b')') {
        Some(pos) => !gap.as_bytes()[..pos]
            .iter()
            .any(|&b| b == b'\n' || b == b'\r'),
        None => false,
    }
}

/// Check if a block comment ending at `comment_end` is effectively inline with `next_pos`.
///
/// True if they share a source line, or if the gap between them contains only stripped
/// grouping parens on the same line as the comment (e.g., `/** @type {T} */ (\n\texpr)`).
pub(super) fn is_comment_inline_with_next(
    printer: &Printer<'_>,
    comment_end: u32,
    next_pos: u32,
) -> bool {
    printer.is_same_line(comment_end, next_pos)
        || has_stripped_paren_gap(printer.source, comment_end, next_pos)
}

/// Check if comments between `start` and `next_code_pos` should force expansion.
/// Excludes inline block comments that share a source line with `next_code_pos`.
///
/// A block comment on a different line from `start` but the same line as `next_code_pos`
/// is an inline leading comment (e.g., `arg1,\n/** @type {T} */ arg2`). These should NOT
/// force expansion — they're part of the next arg's line and the group/fits mechanism
/// should decide the layout.
///
/// Only truly standalone block comments (different line from both `start` AND `next_code_pos`)
/// force expansion.
pub(crate) fn should_force_expansion_for_comments(
    printer: &Printer<'_>,
    start: u32,
    next_code_pos: u32,
) -> bool {
    // Line comments always force expansion
    if printer.has_line_comments_between(start, next_code_pos) {
        return true;
    }
    // Check if any block comment is truly standalone (not inline with the next code)
    for comment in comments_in_range(printer.comments, start, next_code_pos) {
        if comment.is_block
            && !printer.is_same_line(start, comment.span.start)
            && !is_comment_inline_with_next(printer, comment.span.end, next_code_pos)
        {
            return true;
        }
    }
    false
}

/// Check if any comments in a call's arguments force expansion.
///
/// Returns true for line comments or standalone block comments (on their own line,
/// not inline with either neighbor). Inline block comments do not force expansion.
pub(super) fn any_comment_forces_expansion(
    call: &internal::CallExpression<'_>,
    printer: &Printer<'_>,
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

        // Line comments or standalone block comments force expansion.
        // Inline block comments (same line as previous arg or inline with next arg)
        // do not force expansion — the group/fits mechanism decides layout.
        if should_force_expansion_for_comments(printer, arg_end, next_boundary) {
            return true;
        }
    }

    false
}

/// Check if the last arg has leading or trailing comments.
///
/// Matches prettier's shouldExpandLastArg checks:
///   `!hasComment(lastArg, CommentCheckFlags.Leading) &&
///    !hasComment(lastArg, CommentCheckFlags.Trailing)`
///
/// Leading = comments after the comma (or opening paren for single-arg),
/// before the last arg's span.
/// Trailing = comments after the last arg's span, before the closing paren.
///
/// Used to prevent expand-last-arg layout when the last arg has comments,
/// since prettier's shouldExpandLastArg returns false in that case.
pub(super) fn last_arg_has_comments(
    arguments: &[internal::Expression<'_>],
    printer: &Printer<'_>,
    call_end: u32,
    paren_open: u32,
) -> bool {
    let Some(last) = arguments.last() else {
        return false;
    };
    let last_start = last.span().start;

    // Leading: comments before last arg
    if arguments.len() >= 2 {
        // Multi-arg: check after comma
        let prev_end = arguments[arguments.len() - 2].span().end;
        if let Some(cp) = find_comma_pos(printer.source, prev_end, last_start)
            && printer.has_comments_between((cp + 1) as u32, last_start)
        {
            return true;
        }
    } else {
        // Single-arg: check after opening paren
        if printer.has_comments_between(paren_open + 1, last_start) {
            return true;
        }
    }

    // Trailing: comments after last arg, before closing paren
    printer.has_comments_between(last.span().end, call_end)
}

/// Check if the first arg has any comments (leading or trailing).
///
/// Matches prettier's shouldExpandFirstArg check: `!hasComment(firstArg)`
///
/// Leading = comments between opening paren and the first arg's span.
/// Trailing = comments between the first arg's span end and the comma.
///
/// Used to prevent expand-first-arg layout when the first arg has comments,
/// since prettier's shouldExpandFirstArg returns false in that case.
pub(super) fn first_arg_has_any_comments(
    arguments: &[internal::Expression<'_>],
    printer: &Printer<'_>,
    paren_open: u32,
) -> bool {
    if arguments.is_empty() {
        return false;
    }
    let first = &arguments[0];

    // Leading: comments between paren and first arg
    if printer.has_comments_between(paren_open, first.span().start) {
        return true;
    }

    // Trailing: comments between first arg end and comma
    if arguments.len() >= 2 {
        let first_end = first.span().end;
        let next_start = arguments[1].span().start;
        if let Some(cp) = find_comma_pos(printer.source, first_end, next_start) {
            return printer.has_comments_between(first_end, cp as u32);
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
    call: &internal::CallExpression<'_>,
    printer: &Printer<'_>,
) -> bool {
    has_trailing_line_comments_slice(call.arguments, call.span.end, printer)
}

/// Check if there are trailing line comments on any arguments (generic version)
///
/// Used by both CallExpression and NewExpression.
pub(crate) fn has_trailing_line_comments_slice(
    arguments: &[internal::Expression<'_>],
    call_span_end: u32,
    printer: &Printer<'_>,
) -> bool {
    has_trailing_comments_slice_impl(arguments, call_span_end, |start, end| {
        printer.has_line_comments_between(start, end)
    })
}

/// Emit the leading comments between `(` and the first argument into `parts`.
///
/// Same-line trailing block comments are emitted inline (`/* c */ arg`); own-line
/// comments stay on their own line. Several per-argument printer loops only emit
/// leading comments for args `1..n` (via the previous arg's gap), so the first
/// arg's leading comment must be emitted explicitly or it's dropped.
pub(crate) fn emit_first_arg_leading_comments(
    printer: &Printer<'_>,
    parts: &mut DocBuf,
    paren_open: u32,
    first_arg_start: u32,
) {
    if !printer.has_comments_between(paren_open, first_arg_start) {
        return;
    }
    let d = printer.d();
    let pc = PartitionedComments::new(
        printer.comments,
        printer.comment_line_breaks,
        paren_open,
        first_arg_start,
    );
    for comment in &pc.trailing_block {
        parts.push(printer.build_comment_doc(comment));
        parts.push(d.text(" "));
    }
    pc.emit_leading_comments_inline_aware(parts, printer);
}

/// Check if there are trailing comments (line OR block) on any arguments
///
/// Used when we need to detect ALL trailing comments, not just line comments.
/// This is important for new expressions where block comments after arguments
/// can also be lost if not handled properly.
pub(crate) fn has_trailing_comments_slice(
    arguments: &[internal::Expression<'_>],
    call_span_end: u32,
    printer: &Printer<'_>,
) -> bool {
    has_trailing_comments_slice_impl(arguments, call_span_end, |start, end| {
        printer.has_comments_between(start, end)
    })
}

/// Shared implementation for checking trailing comments on arguments.
#[inline]
fn has_trailing_comments_slice_impl<F>(
    arguments: &[internal::Expression<'_>],
    call_span_end: u32,
    has_comments: F,
) -> bool
where
    F: Fn(u32, u32) -> bool,
{
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

        if has_comments(arg_end, next_boundary) {
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
///
/// `new` shares the same-line/later-line classification with the ternary
/// (`conditional.rs`) and member-chain (`chain/builder/helpers.rs`) gap printers via
/// `tsv_lang::ClassifiedComments`. This type adds the call-argument-specific emission
/// (`emit_*`) and comma-relative helpers on top; only the emission differs per shape
/// (operator / comma / dot), which is intentional.
pub(crate) struct PartitionedComments<'a> {
    pub trailing_line: SmallVec<[&'a internal::Comment; 2]>,
    pub trailing_block: SmallVec<[&'a internal::Comment; 2]>,
    pub leading: SmallVec<[&'a internal::Comment; 2]>,
    /// The gap the comments were partitioned over: `start` is the preceding element's
    /// end, `end` the following element's start. The emit/query methods operate on
    /// this gap (comma scan, blank-line check, dangling-comment base), so they read
    /// these rather than re-receiving the bounds the caller already passed to `new`.
    start: u32,
    end: u32,
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
        // Share the same-line/later-line classification with the chain and ternary
        // gap printers (`tsv_lang::ClassifiedComments`). `leading` keeps the two
        // own-line buckets merged in source order — the inline-aware emitter and its
        // JSDoc-cast detection rely on the authored order.
        let classified =
            tsv_lang::ClassifiedComments::from_range(comments, start, end, line_breaks);
        let leading = classified.leading_in_source_order();
        Self {
            trailing_line: classified.trailing_line,
            trailing_block: classified.trailing_block,
            leading,
            start,
            end,
        }
    }

    /// Respect-the-newline split for a non-last argument gap: move after-comma block
    /// comments that **hug** the next argument out of `trailing_block` and into
    /// `leading`, so they render as a leading comment on the next argument (`C`).
    /// A **stranded** after-comma block (a newline separates it from the next argument)
    /// stays in `trailing_block` and renders after the comma on the same line (`A`).
    ///
    /// The author's placement is preserved in both cases: a comment hugging the next
    /// arg leads it; a comment left alone on the comma line stays there. Callers then
    /// emit `trailing_block` (before-comma blocks + stranded after-comma) via
    /// [`emit_trailing_comments_around_comma`], the line break, then `leading` (own-line
    /// comments + hugged after-comma) via [`emit_leading_comments_inline_aware`] — so the
    /// rule lives here once and every argument path inherits it.
    pub fn route_after_comma_hugging_to_leading(&mut self, printer: &Printer<'_>) {
        let Some(comma_pos) = find_comma_pos(printer.source, self.start, self.end) else {
            return;
        };
        let mut kept: SmallVec<[&'a internal::Comment; 2]> = SmallVec::new();
        for comment in self.trailing_block.drain(..) {
            if is_comment_after_comma(comment, comma_pos)
                && is_comment_inline_with_next(printer, comment.span.end, self.end)
            {
                // Hugs the next arg → leads it. Source order holds: the hug sits on the
                // next arg's line, after any own-line leading comments, so appending keeps
                // `leading` sorted.
                self.leading.push(comment);
            } else {
                kept.push(comment);
            }
        }
        self.trailing_block = kept;
    }

    pub fn has_trailing_line(&self) -> bool {
        !self.trailing_line.is_empty()
    }

    pub fn has_trailing_block(&self) -> bool {
        !self.trailing_block.is_empty()
    }

    /// Check for a blank line in the gap between trailing and leading comments.
    ///
    /// When comments exist between arguments, we can't check the full arg-to-arg
    /// range for blank lines because intermediate comment newlines would create
    /// false positives. Instead, check the sub-range from:
    /// - Start: after the last trailing line comment, or after the comma, or the gap start
    /// - End: before the first leading comment, or the gap end
    pub fn has_blank_line_in_gap(&self, source: &str, line_breaks: &[u32]) -> bool {
        let comma_after = find_comma_pos(source, self.start, self.end).map(|c| c as u32 + 1);
        let check_start = if let Some(last) = self.trailing_line.last() {
            // A line comment trailing the arg before its comma (`a // c⏎,⏎b`) ends on
            // an earlier line than the comma, which sits on its own line. Scanning from
            // the comment's end would straddle the comma's line and miscount it as a
            // blank line, so start after whichever of (comment end, comma) is later.
            comma_after.map_or(last.span.end, |c| last.span.end.max(c))
        } else {
            comma_after.unwrap_or(self.start)
        };
        let check_end = if !self.leading.is_empty() {
            self.leading[0].span.start
        } else {
            self.end
        };
        tsv_lang::printing::has_blank_line_between_fast(line_breaks, check_start, check_end)
    }

    /// Emit trailing comments (block then line) with leading spaces to a parts vector.
    ///
    /// Used for comments that follow an argument, formatted as ` /* block */ // line`.
    /// Line comments go through `line_suffix` (zero width) so they never count against
    /// the argument's own group — flushing at the caller's following hardline (every
    /// caller is a forced-multiline context). Prettier's `lineSuffix`.
    pub fn emit_trailing_comments(&self, parts: &mut DocBuf, printer: &Printer<'_>) {
        let d = printer.d();
        for comment in &self.trailing_block {
            parts.push(d.text(" "));
            parts.push(printer.build_comment_doc(comment));
        }
        for comment in &self.trailing_line {
            parts.push(printer.build_trailing_line_comment_doc(comment));
        }
    }

    /// Emit a non-last arg's trailing comments split around its comma, then push the
    /// comma itself: before-comma block comments trail the arg (`arg /* c */,`),
    /// after-comma blocks and the same-line line comment follow the comma
    /// (`arg, /* c */ // c2`). The caller adds the line break after.
    ///
    /// Unlike [`emit_trailing_comments`] (which the caller invokes *after* pushing
    /// the comma, so every block lands after it), this keeps a before-comma block in
    /// its authored position. Shared by the `new`-argument non-last paths
    /// (`build_new_doc_with_wrapping` and `build_args_with_blank_lines`) so they
    /// can't drift — both used to relocate the block past the comma.
    pub fn emit_trailing_comments_around_comma(&self, parts: &mut DocBuf, printer: &Printer<'_>) {
        let d = printer.d();
        let comma_pos = find_comma_pos(printer.source, self.start, self.end);
        if let Some(cpos) = comma_pos {
            for comment in &self.trailing_block {
                if is_comment_before_comma(comment, cpos) {
                    parts.push(d.text(" "));
                    parts.push(printer.build_comment_doc(comment));
                }
            }
        }
        parts.push(d.text(","));
        if let Some(cpos) = comma_pos {
            for comment in &self.trailing_block {
                if is_comment_after_comma(comment, cpos) {
                    parts.push(d.text(" "));
                    parts.push(printer.build_comment_doc(comment));
                }
            }
        }
        for comment in &self.trailing_line {
            parts.push(printer.build_trailing_line_comment_doc(comment));
        }
    }

    /// Emit own-line ("leading") comments each on its own line (hardline before),
    /// with no comma. The bare dangling-comment emission shared by every last-argument
    /// path (no trailing comma precedes them — trailingComma: 'none') and by comma-less
    /// shapes (dynamic `import()`). Without it, own-line comments before the closing paren
    /// are dropped (content loss).
    /// The dangling comments follow the gap `start` (the preceding element's end);
    /// that is the base for preserving an author blank line before the first own-line
    /// comment.
    pub fn emit_dangling_comments(&self, parts: &mut DocBuf, printer: &Printer<'_>) {
        let mut prev_end = self.start;
        for comment in &self.leading {
            // Preserve an author blank line before an own-line trailing comment
            // (`arg⏎⏎/* c */` before the closing `)`), matching prettier.
            printer.push_blank_preserving_hardline(parts, prev_end, comment.span.start);
            parts.push(printer.build_comment_doc(comment));
            prev_end = comment.span.end;
        }
    }

    /// Emit a last argument's complete trailing-comment region: same-line comments (via
    /// [`emit_trailing_comments`]), then own-line dangling comments (via
    /// [`emit_dangling_comments`]). No trailing comma is emitted (trailingComma: 'none'),
    /// so a same-line block trails the arg in source order whether it sat before or after
    /// the source comma — the last arg needs no split around the never-emitted comma.
    ///
    /// The last-argument counterpart to [`Printer::open_inter_arg_gap`] (the non-last
    /// gap): shared by the `new` and member-chain last-arg paths so the ordering lives in
    /// one place. (`call_formatting` keeps its own same-line loop, feeding
    /// `force_expansion`, and calls only [`emit_dangling_comments`] directly.)
    pub fn emit_last_arg_comments(&self, parts: &mut DocBuf, printer: &Printer<'_>) {
        // `emit_trailing_comments` already no-ops when there are no trailing comments,
        // so no presence guard is needed.
        self.emit_trailing_comments(parts, printer);
        self.emit_dangling_comments(parts, printer);
    }

    /// Emit leading comments, keeping inline block comments on the same line as the
    /// gap `end` (the following element's start).
    ///
    /// For comments on the same line as `end`, emits them inline (comment + space).
    /// For comments on their own line, emits them with hardline after.
    ///
    /// For nested JSDoc casts like `/** @type {A} */ (\n\t/** @type {B} */ (expr))`,
    /// after paren stripping both comments become leading. The inner comment is inline
    /// with the arg, and the outer comment is followed by a stripped `(` on the same line.
    /// Both should stay inline: `/** @type {A} */ /** @type {B} */ expr`.
    pub fn emit_leading_comments_inline_aware(&self, parts: &mut DocBuf, printer: &Printer<'_>) {
        let d = printer.d();
        let next_pos = self.end;

        // Pre-compute which comments should be inline. Walk backwards: if the last
        // block comment is inline with next_pos, check preceding block comments — if
        // they're followed by a stripped open paren on the same line, they're also inline
        // (nested JSDoc cast pattern).
        let mut inline_flags: SmallVec<[bool; 4]> = SmallVec::new();
        inline_flags.resize(self.leading.len(), false);

        let mut next_inline_start = next_pos;
        for (i, comment) in self.leading.iter().enumerate().rev() {
            if comment.is_block
                && is_comment_inline_with_next(printer, comment.span.end, next_inline_start)
            {
                inline_flags[i] = true;
                // This comment is inline — check if the PREVIOUS comment connects
                // to this one via a stripped paren gap
                next_inline_start = comment.span.start;
            } else {
                break;
            }
        }

        for (i, comment) in self.leading.iter().enumerate() {
            parts.push(printer.build_comment_doc(comment));
            if inline_flags[i] {
                parts.push(d.text(" "));
            } else {
                // Preserve a blank line the author left between this comment and what
                // follows it (the next leading comment, or the argument itself).
                let next = self.leading.get(i + 1).map_or(next_pos, |c| c.span.start);
                printer.push_blank_preserving_hardline(parts, comment.span.end, next);
            }
        }
    }
}
