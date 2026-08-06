// Comment handling for call expression arguments
//
// Handles detection and partitioning of comments in argument lists:
// - Inter-argument comments (between arguments)
// - Trailing comments on arguments
// - Leading comments before arguments

use smallvec::SmallVec;

use super::super::{CommentFilter, CommentSpacing, Printer};
use crate::ast::internal;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

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

/// Emit an empty argument list into `parts`: the comments in the gap before the
/// `(` (`fn<string> /* c */()`), then the parens themselves — closed (`()`) or
/// enclosing their dangling comments (`fn(/* c */)`, and the broken form a line
/// comment forces).
///
/// `search_from` is where to look for the `(` (the position after the type
/// arguments, or after the callee when there are none) and `paren_close` the
/// position past the `)`. `prefix` is the open delimiter — `"("`, or `"?.("` for
/// an optional call.
///
/// Shared by the plain call/`new` path and the member-chain path so the empty-args
/// shape lives in one place: these two drifted apart once already, and the
/// inline-comment emission that drift preserved let a `//` comment swallow the `)`.
pub(super) fn push_empty_args(
    printer: &Printer<'_>,
    parts: &mut DocBuf,
    search_from: u32,
    paren_close: u32,
    prefix: &'static str,
) {
    let d = printer.d();
    let Some(paren_pos) = printer.find_char_outside_comments(search_from, paren_close, b'(') else {
        // No `(` found at all (unreachable for valid code): emit the closed form.
        parts.push(d.text(prefix));
        parts.push(d.text(")"));
        return;
    };
    let parens =
        printer.build_empty_parens_inline_with_comments_doc(paren_pos, paren_close, prefix);
    // A **line** comment in this gap runs to end of line, so the argument list cannot
    // stay on it — left inline the `//` swallows the `()` and everything after it
    // (`call // c⏎()` → `call // c();`, losing the call itself). The comment keeps the
    // position the author gave it and the list drops to a continuation line indented one
    // level: the uniform forced-continuation indent every line-comment-split construct
    // shares (`build_continuation_indent`), so the list reads as part of its call rather
    // than as a sibling statement. A block comment forces nothing and stays inline below.
    if printer.has_line_comments_between(search_from, paren_pos) {
        parts.push(printer.build_continuation_indent(search_from, paren_pos, parens));
        return;
    }
    if let Some(pre) = printer.build_comments_between_filtered_opt(
        search_from,
        paren_pos,
        CommentSpacing::Leading,
        CommentFilter::All,
    ) {
        parts.push(pre);
    }
    parts.push(parens);
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
/// Tolerates an inverted range (`from > to`): parser-produced argument spans
/// are always ascending, but a synthetic tree mixing borrowed (host-span) and
/// minted (appendix-span) arguments can invert a gap — there is no paren to
/// skip in an empty or inverted gap, so `to` comes back unchanged.
#[inline]
pub(crate) fn skip_stripped_open_paren(source: &str, from: u32, to: u32) -> u32 {
    if from >= to {
        return to;
    }
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

/// Build inline block comments AFTER the comma as leading on the next arg.
///
/// For `fn(a, /** @type {T} */ b)`, the comment is after the comma and should
/// be emitted as `/** @type {T} */ ` before `b`, not as trailing on `a`. Shared
/// by the call, `new`, and chain expand-first paths so a block comment leading the
/// second arg is preserved inline rather than dropped.
pub(super) fn build_after_comma_leading_comments(
    printer: &Printer<'_>,
    prev_arg_end: u32,
    arg_start: u32,
) -> Option<DocId> {
    let d = printer.d();
    let comma_pos = find_comma_pos(printer.source, prev_arg_end, arg_start)?;
    let mut parts = DocBuf::new();
    for comment in comments_to_emit_in_range(printer.comments, prev_arg_end, arg_start) {
        if is_comment_after_comma(comment, comma_pos)
            && comment.is_block
            && is_comment_inline_with_next(printer, comment.span.end, arg_start)
        {
            parts.push(printer.build_comment_doc(comment));
            parts.push(d.text(" "));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(d.concat(&parts))
    }
}

/// Build inline block comments BEFORE the comma as trailing on the current arg.
///
/// For `fn(a /* comment */, b)`, the comment is before the comma and should
/// be emitted as ` /* comment */` after `a`.
pub(super) fn build_before_comma_trailing_comments(
    printer: &Printer<'_>,
    arg_end: u32,
    next_arg_start: u32,
) -> Option<DocId> {
    let d = printer.d();
    let comma_pos = find_comma_pos(printer.source, arg_end, next_arg_start)?;
    let mut parts = DocBuf::new();
    for comment in comments_to_emit_in_range(printer.comments, arg_end, next_arg_start) {
        if is_comment_before_comma(comment, comma_pos)
            && comment.is_block
            && printer.is_same_line(arg_end, comment.span.start)
        {
            parts.push(d.text(" "));
            parts.push(printer.build_comment_doc(comment));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(d.concat(&parts))
    }
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
        .any(|pair| printer.has_comments_to_emit_between(pair[0].span().end, pair[1].span().start))
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
    for comment in comments_to_emit_in_range(printer.comments, start, next_code_pos) {
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
    if printer.has_comments_to_emit_between(paren_open, first_arg_start)
        && should_force_expansion_for_comments(printer, paren_open, first_arg_start)
    {
        return true;
    }

    // Check inter-argument and trailing comments
    for (i, arg) in call.arguments.iter().enumerate() {
        // A comment a spread's stripped grouping parens left behind (`...(x⏎/* c */)`,
        // `...(x // c⏎)`) sits BEFORE the argument's own end, so the gap scan below cannot
        // see it — yet it is exactly what forces the list open, either because it needs a
        // line of its own or because its deferred `//` would otherwise flush past the
        // call's `)`. Asked per argument: a non-last spread's interior needs the expansion
        // just as much as the last one's.
        if printer.spread_paren_comment_forces_expansion(arg) {
            return true;
        }

        let arg_end = arg.span().end;
        let next_boundary = if i < call.arguments.len() - 1 {
            call.arguments[i + 1].span().start
        } else {
            call.span.end
        };

        if !printer.has_comments_to_emit_between(arg_end, next_boundary) {
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

    // Leading: comments before last arg. Counts owned comments — this is a pure layout
    // gate (it only disables the expand-last hug), and a bundler annotation leading the
    // last argument is on the page just like any other leading comment, so prettier's
    // `shouldExpandLastArg` refuses the hug for it too.
    if arguments.len() >= 2 {
        // Multi-arg: check after comma
        let prev_end = arguments[arguments.len() - 2].span().end;
        if let Some(cp) = find_comma_pos(printer.source, prev_end, last_start)
            && printer.has_comments_on_page_between((cp + 1) as u32, last_start)
        {
            return true;
        }
    } else {
        // Single-arg: check after opening paren
        if printer.has_comments_on_page_between(paren_open + 1, last_start) {
            return true;
        }
    }

    // Trailing: comments after last arg, before closing paren
    printer.has_comments_to_emit_between(last.span().end, call_end)
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
///
/// **on page**, like its twin [`last_arg_has_comments`]: this only disables the
/// expand-first hug — a pure layout gate — and a bundler annotation leading the first
/// argument is on the page just like any other leading comment, so prettier's
/// `shouldExpandFirstArg` refuses the hug for it too.
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
    if printer.has_comments_on_page_between(paren_open, first.span().start) {
        return true;
    }

    // Trailing: comments between first arg end and comma
    if arguments.len() >= 2 {
        let first_end = first.span().end;
        let next_start = arguments[1].span().start;
        if let Some(cp) = find_comma_pos(printer.source, first_end, next_start) {
            return printer.has_comments_on_page_between(first_end, cp as u32);
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

/// Emit the leading comments between `(` and the first argument, split across the
/// two lines they can land on: `paren_line` rides the `(` line, `parts` leads the
/// first argument.
///
/// Own-line comments always lead the first argument. A same-line run trailing `(`
/// goes one of two ways, and the split is the run's *kind*, not each comment's:
///
/// - **Block-only** (`fn(/* c */ a)`) — emitted inline ahead of the first argument, so
///   a call that fits stays on one line (matching prettier).
/// - **Any line comment in the run** — the whole run stays on the `(` line, in source
///   order. A `//` runs to EOL, so it cannot ride the argument's line; keeping it where
///   the author put it is tsv's sanctioned divergence (prettier relocates it to its own
///   line) — see `docs/conformance_prettier_ts_comments.md` §Comment relocation (Call
///   open paren `(`). Taking the run whole is what keeps a preceding block from jumping
///   past it: emitting the block inline and the line comment on the `(` line would
///   REVERSE the authored pair.
///
/// A non-empty `paren_line` obliges the caller to hard-break the call —
/// [`wrap_call_with_hard_breaks_paren_line`](super::arg_wrapping::wrap_call_with_hard_breaks_paren_line)
/// is that wrap. Otherwise the `(` line's `//` swallows the arguments that follow it.
///
/// Several per-argument printer loops only emit leading comments for args `1..n` (via
/// the previous arg's gap), so the first arg's leading comment must be emitted
/// explicitly or it's dropped.
///
/// **Why the split is `has_trailing_line` here and
/// `Printer::delimiter_line_comment_prefix`'s wider conjunction elsewhere** — the two are
/// the same rule read at different points, not drift (`docs/comments.md` §The
/// delimiter-line question). Both say *pull onto the delimiter's line only when the
/// container is breaking anyway; otherwise let the block hug the first element*. The list
/// family has to derive "is it breaking?" from the gap
/// (`should_force_expansion_for_comments`), and so do the call family's force-expanded
/// builders, which spell that same conjunction over
/// [`PartitionedComments::has_trailing_comments`]. This emitter runs on the paths where
/// the call may still collapse, so the only thing that can force the break is a `//` in
/// the run itself — and a block-only run must NOT be pulled, or a call that would have fit
/// is broken by the pull.
pub(crate) fn emit_first_arg_leading_comments(
    printer: &Printer<'_>,
    paren_line: &mut DocBuf,
    parts: &mut DocBuf,
    paren_open: u32,
    first_arg_start: u32,
) {
    if !printer.has_comments_to_emit_between(paren_open, first_arg_start) {
        return;
    }
    let d = printer.d();
    let pc = PartitionedComments::new(
        printer.comments,
        printer.comment_line_breaks,
        paren_open,
        first_arg_start,
    );
    if pc.has_trailing_line() {
        pc.emit_trailing_comments(paren_line, printer);
    } else {
        for comment in &pc.trailing_block {
            parts.push(printer.build_comment_doc(comment));
            parts.push(d.text(" "));
        }
    }
    pc.emit_leading_comments_inline_aware(parts, printer);
}

/// Emit the comments between the LAST argument and `)` — the closing counterpart to
/// [`emit_first_arg_leading_comments`], and the other half of what a per-argument loop
/// owes beyond its interior gaps (see `docs/comments.md` §The element-comma seam).
///
/// A loop whose gap lookup is guarded by `i < len - 1` has no emitter for this region at
/// all, so everything an author parked after the last argument is DROPPED.
///
/// Two regions, in source order, and they must **partition**: the parent's share of the
/// last argument's own stripped-paren interior
/// ([`Printer::push_spread_own_line_block_comments`] — the own-line blocks a spread's
/// doc deliberately leaves for its parent), then the ordinary gap between the argument's
/// end and `)`. The second keeps the plain `arg.span().end` anchor: widening it to reach
/// the interior is what claims the spread's own share a second time.
pub(crate) fn emit_last_arg_trailing_comments(
    printer: &Printer<'_>,
    parts: &mut DocBuf,
    last_arg: &internal::Expression<'_>,
    paren_close: u32,
) {
    printer.push_spread_own_line_block_comments(parts, last_arg);
    let arg_end = last_arg.span().end;
    // Every argument list reaching these builders pays this call, comments or not, so
    // skip the partition on the common empty gap (same guard as
    // `emit_first_arg_leading_comments`). Both emits `PartitionedComments` would run
    // walk only its own buckets, so an empty range is already a no-op.
    if !printer.has_comments_to_emit_between(arg_end, paren_close) {
        return;
    }
    let mut pc = PartitionedComments::new(
        printer.comments,
        printer.comment_line_breaks,
        arg_end,
        paren_close,
    );
    pc.demote_trailing_line_after_deferred(printer, last_arg);
    pc.emit_last_arg_comments(parts, printer);
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
        printer.has_comments_to_emit_between(start, end)
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
    /// [`Self::emit_trailing_comments_around_comma`], the line break, then `leading` (own-line
    /// comments + hugged after-comma) via [`Self::emit_leading_comments_inline_aware`] — so the
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

    /// Reclassify this gap's same-line LINE comments as own-line when `prev` — the node
    /// the gap opens after — already ends in a DEFERRED line comment
    /// ([`Printer::defers_trailing_line_comment`]).
    ///
    /// Its output line already terminates in a `//`, so nothing more may join it:
    /// deferring a second line comment onto the same line welds the two into ONE comment,
    /// the second `//` becoming text inside the first. Moving it to `leading` gives it the
    /// line it needs — which is also where a reparse keeps it, so the form is a fixed
    /// point. Prepended, because a same-line comment precedes every own-line one in
    /// source.
    ///
    /// Asked at the gap rather than inside the emitters, so the two last-argument
    /// consumers (the shared [`emit_last_arg_trailing_comments`] and
    /// `call_formatting`'s own loop, which needs its `force_expansion` feedback) get the
    /// rule from one place.
    pub fn demote_trailing_line_after_deferred(
        &mut self,
        printer: &Printer<'_>,
        prev: &internal::Expression<'_>,
    ) {
        if self.trailing_line.is_empty() || !printer.defers_trailing_line_comment(prev) {
            return;
        }
        for comment in self.trailing_line.drain(..).rev() {
            self.leading.insert(0, comment);
        }
    }

    pub fn has_trailing_block(&self) -> bool {
        !self.trailing_block.is_empty()
    }

    /// Whether anything at all sits on the opening delimiter's line — exactly what
    /// [`Self::emit_trailing_comments`] would emit, asked ahead of emitting it.
    ///
    /// The delimiter-line question every argument builder opens with: a non-empty run
    /// has to be injected after the `(` rather than led onto the first argument, and
    /// the run is taken WHOLE (both buckets) or an authored `/* b */ // c` pair comes
    /// back reversed. Paired with the emitter so the two can't answer differently.
    ///
    /// Conjoined with `should_force_expansion_for_comments` at every force-expanded call
    /// site, this **is** `Printer::delimiter_line_comment_prefix`'s `pull` — the list
    /// family's spelling of the same rule (`docs/comments.md` §The delimiter-line
    /// question). Keep the two in step.
    pub fn has_trailing_comments(&self) -> bool {
        self.has_trailing_block() || self.has_trailing_line()
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
    /// Unlike [`Self::emit_trailing_comments`] (which the caller invokes *after* pushing
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

    /// Emit own-line ("leading") comments, with no comma. The bare dangling-comment
    /// emission shared by every last-argument path (no trailing comma precedes them —
    /// trailingComma: 'none') and by comma-less shapes (dynamic `import()`). Without it,
    /// own-line comments before the closing paren are dropped (content loss).
    /// The dangling comments follow the gap `start` (the preceding element's end);
    /// that is the base for preserving an author blank line before the first own-line
    /// comment.
    ///
    /// The separator goes BEFORE each comment (`docs/comments.md` §Trailing and dangling
    /// runs) and is asked of the **source**, like
    /// [`Printer::push_trailing_comments_in_range`]: a comment the author glued to the
    /// previous one's line (`/* c */ // t`, `/* c1 */ /* c2 */`) stays on that line.
    /// Giving each its own line unconditionally reads as the safer rule and is not — the
    /// run re-collapses on the next pass, because a comment printed onto a fresh line is
    /// no longer glued when it is reparsed, so the output never reaches a fixed point
    /// (F1). The first comment's separator is unconditional: either it is own-line by
    /// construction (that is what put it in `leading`), or
    /// [`Self::demote_trailing_line_after_deferred`] moved it here precisely because it
    /// needs a line the previous node's deferred `//` denies it.
    pub fn emit_dangling_comments(&self, parts: &mut DocBuf, printer: &Printer<'_>) {
        let d = printer.d();
        let mut prev_end = self.start;
        for (i, comment) in self.leading.iter().enumerate() {
            if i > 0 && !printer.has_newline_between(prev_end, comment.span.start) {
                parts.push(d.text(" "));
            } else {
                // Preserve an author blank line before an own-line trailing comment
                // (`arg⏎⏎/* c */` before the closing `)`), matching prettier.
                printer.push_blank_preserving_hardline(parts, prev_end, comment.span.start);
            }
            parts.push(printer.build_comment_doc(comment));
            prev_end = comment.span.end;
        }
    }

    /// Emit a last argument's complete trailing-comment region: same-line comments (via
    /// [`Self::emit_trailing_comments`]), then own-line dangling comments (via
    /// [`Self::emit_dangling_comments`]). No trailing comma is emitted (trailingComma: 'none'),
    /// so a same-line block trails the arg in source order whether it sat before or after
    /// the source comma — the last arg needs no split around the never-emitted comma.
    ///
    /// The last-argument counterpart to [`Printer::open_inter_arg_gap`] (the non-last
    /// gap): shared by the `new` and member-chain last-arg paths so the ordering lives in
    /// one place. (`call_formatting` keeps its own same-line loop, feeding
    /// `force_expansion`, and calls only [`Self::emit_dangling_comments`] directly.)
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
                printer.push_leading_run_separator(
                    parts,
                    comment,
                    self.leading.get(i + 1).map_or(next_pos, |c| c.span.start),
                );
            }
        }
    }
}
