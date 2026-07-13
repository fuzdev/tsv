//! Rendering algorithm for arena-based document trees.

use crate::EmbedContext;
use crate::config::TAB_WIDTH;
use crate::printing::visual_width;
use smallvec::SmallVec;

use super::arena::{ArenaCommand, CmdStack, DocArena, DocId, DocNode, LineSuffixBuf};
use super::arena_fits::arena_fits_with_lookahead;
use super::arena_render_fill::render_fill_iterative;
use super::render_config::RenderConfig;
#[cfg(feature = "swallow_check")]
use super::swallow::SwallowTracker;
use super::types::{CachedWidth, DocContext, GroupId, LineKind, Mode, TextResolver, resolve_text};

/// The mode each id-bearing group resolved to, as a total map over the closed
/// [`GroupId`] enum. Backed by a fixed inline array indexed by `id as usize`, so
/// it never allocates (the `HashMap` it replaces allocated a table on every
/// render that resolved at least one keyed group). `None` = not yet resolved,
/// read as flat — identical to `HashMap::get` returning `None`. Writes are
/// last-write-wins, matching the `HashMap` (a `GroupId` variant shared across
/// nested groups resolves before its reader, per the variant docs).
#[derive(Default)]
struct GroupModeMap {
    slots: [Option<Mode>; GroupId::COUNT],
}

impl GroupModeMap {
    #[inline]
    fn insert(&mut self, id: GroupId, mode: Mode) {
        self.slots[id as usize] = Some(mode);
    }

    #[inline]
    fn get(&self, id: GroupId) -> Option<Mode> {
        self.slots[id as usize]
    }
}

/// Trim trailing whitespace from only the last line of output.
/// Interior lines are already handled by `trim_trailing_whitespace()` in `render_line_break()`.
fn trim_last_line(mut s: String) -> String {
    trim_last_line_in_place(&mut s);
    s
}

/// In-place form of [`trim_last_line`] for the `*_into` entry points that
/// render into a caller-provided buffer.
fn trim_last_line_in_place(s: &mut String) {
    // Find the last newline — only trim after it (the final line). A manual
    // reverse byte scan avoids `str::rfind('\n')`'s `CharSearcher`/`memrchr`
    // setup; `\n` is single-byte ASCII so its byte index is a char boundary and
    // the resulting slice is identical.
    let trim_start = s
        .as_bytes()
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let trimmed_len = trimmable_end(&s[trim_start..]);
    s.truncate(trim_start + trimmed_len);
}

//
// Shared rendering helpers
//

/// Render text content and update position.
///
/// Uses cached width when available to skip `visual_width()` for the common
/// no-newline case. Still needs `resolve_text()` to get the actual string for output.
///
/// `inline(always)`: plain `#[inline]` left this outlined (a measured ~4%
/// standalone symbol paying call overhead once per `Text` node — the most
/// common node kind), and there are only two call sites, one per render loop.
/// Forcing it measured instructions −0.8% on both corpora with cycles and
/// branch-misses down alongside — a real win, not an icache artifact.
#[allow(clippy::inline_always)]
#[inline(always)]
fn render_text<R: TextResolver + ?Sized>(
    text: &super::types::DocText,
    output: &mut String,
    pos: &mut usize,
    resolver: Option<&R>,
    pool: &str,
) {
    let s = resolve_text(text, resolver, pool);
    output.push_str(s);
    match text.cached_width() {
        CachedWidth::Width(w) => *pos += w as usize, // Common path: no visual_width call
        CachedWidth::HasNewline => update_pos_for_text_unicode(pos, s),
        CachedWidth::NotComputed => update_pos_for_text(pos, s),
    }
}

/// Update position after rendering a text string, accounting for tab expansion.
///
/// The overwhelmingly common input here is short ASCII with no newline — every
/// interned identifier (`Symbol`) and every span-identity identifier name
/// (`source_span_ident`) reaches this via `render_text`'s uncached-width arm
/// (statics carry an amortized cached width and skip it). For those the previous
/// shape scanned the bytes three times (`rfind('\n')` + `visual_width`'s own
/// `is_ascii` + tab count). The fast path below folds the newline reset, tab
/// expansion, and width accumulation into a single forward byte pass, so no
/// backward `memchr` scan runs. The first non-ASCII byte hands off to
/// `update_pos_for_text_unicode` (cold-outlined to keep this fast path lean and
/// inlinable, mirroring `skip_trivia` / `skip_trivia_scan`). Byte-identical to
/// the prior implementation by construction.
#[inline]
fn update_pos_for_text(pos: &mut usize, s: &str) {
    let mut col = *pos;
    for &b in s.as_bytes() {
        match b {
            b'\n' => col = 0,
            b'\t' => col += TAB_WIDTH,
            0..=0x7f => col += 1,
            _ => return update_pos_for_text_unicode(pos, s),
        }
    }
    *pos = col;
}

/// Position update for text that contains a newline or a non-ASCII byte: the
/// column restarts after the last newline (if any), measured grapheme-aware.
/// Re-measures the whole string from scratch (`update_pos_for_text`'s partial
/// `col` is intentionally dropped) so a combining mark attaching to an ASCII
/// base char is never split mid-grapheme. Cold-outlined to keep the ASCII fast
/// path lean and inlinable; `visual_width`'s ASCII-run scanning keeps this
/// affordable even on multibyte-dense corpora, where it is not rare.
#[cold]
#[inline(never)]
fn update_pos_for_text_unicode(pos: &mut usize, s: &str) {
    if let Some(last_newline_pos) = s.rfind('\n') {
        *pos = visual_width(&s[last_newline_pos + 1..], TAB_WIDTH);
    } else {
        *pos += visual_width(s, TAB_WIDTH);
    }
}

/// Reserved trailing-punctuation width once the printer has crossed
/// `first_line_offset`. Embedding contexts use this to keep the suffix
/// (e.g., `}` after a Svelte template expression) on the same line.
#[inline]
fn effective_suffix_width(pos: usize, embed: &EmbedContext) -> usize {
    if pos >= embed.first_line_offset {
        embed.suffix_width
    } else {
        0
    }
}

/// Width remaining on the current line for a group's fits check: the print
/// width minus the reserved embedding suffix ([`effective_suffix_width`])
/// minus the current column, saturating at zero before the `isize` cast.
#[inline]
fn remaining_width(pos: usize, render: &RenderConfig, embed: &EmbedContext) -> isize {
    render
        .print_width
        .saturating_sub(effective_suffix_width(pos, embed))
        .saturating_sub(pos) as isize
}

/// Trim trailing whitespace (spaces and tabs) from the end of the output buffer.
/// Matches Prettier's `trim()` / `trimIndentation()` — called before each
/// non-literal newline to strip trailing indentation/spaces from code lines.
#[inline]
pub(super) fn trim_trailing_whitespace(output: &mut String) {
    output.truncate(trimmable_end(output));
}

/// The length `output` keeps after trailing spaces/tabs are trimmed — **except an
/// escaped one**.
///
/// A trailing space/tab preceded by an odd-length `\` run is that escape's payload,
/// not layout whitespace: it is *content*, and trimming it strands the backslash
/// onto whatever the caller appends next. In CSS a value may legitimately end in
/// one (`width: 50px\ ;` — CSS Syntax 3 §4.3.4/§4.3.7 make `\` + whitespace a valid
/// escape), and dropping it turns the following `;` into an escaped character, so
/// the output no longer parses.
///
/// An even-length run is a completed `\\`, so the whitespace after it is ordinary
/// padding and still goes; only the escape's single payload character survives.
#[inline]
fn trimmable_end(output: &str) -> usize {
    let trimmed = output.trim_end_matches([' ', '\t']);
    if trimmed.len() == output.len() {
        return output.len();
    }
    let backslashes = trimmed.bytes().rev().take_while(|&b| b == b'\\').count();
    if backslashes % 2 == 0 {
        trimmed.len()
    } else {
        // Keep the escape's one payload character (a space or tab — both 1 byte).
        trimmed.len() + 1
    }
}

/// Render a line break.
#[inline]
fn render_line_break(
    kind: LineKind,
    mode: Mode,
    indent_level: usize,
    output: &mut String,
    pos: &mut usize,
    render: &RenderConfig,
    embed: &EmbedContext,
) -> bool {
    let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
    if mode == Mode::Break || is_hard {
        if kind == LineKind::Literal {
            // Literal line (template literals): preserve trailing whitespace
            output.push('\n');
            *pos = 0;
        } else {
            // Non-literal line: trim trailing whitespace before newline
            // (matches Prettier's trim() call before non-literal newlines)
            trim_trailing_whitespace(output);
            output.push('\n');
            write_indentation(output, indent_level, render, embed);
            *pos = line_start_column(indent_level, render, embed);
        }
        true
    } else if kind == LineKind::Normal {
        output.push(' ');
        *pos += 1;
        false
    } else {
        false
    }
}

/// Flush pending line suffix content.
#[allow(clippy::too_many_arguments)]
fn flush_line_suffix<R: TextResolver + ?Sized>(
    arena: &DocArena,
    line_suffix: &mut LineSuffixBuf,
    output: &mut String,
    pos: &mut usize,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
    should_remeasure: &mut bool,
) {
    if line_suffix.is_empty() {
        return;
    }
    for suffix_cmd in std::mem::take(line_suffix).into_iter().rev() {
        render_single_doc_inner(
            arena,
            suffix_cmd.doc,
            output,
            pos,
            suffix_cmd.indent,
            suffix_cmd.mode,
            render,
            embed,
            resolver,
            None,
            should_remeasure,
        );
    }
}

/// Process an IndentIfBreak node.
#[inline]
fn process_indent_if_break(
    contents: DocId,
    group_id: GroupId,
    negate: bool,
    group_mode_map: Option<&GroupModeMap>,
    cmd: &ArenaCommand,
) -> ArenaCommand {
    let group_mode = group_mode_map
        .and_then(|map| map.get(group_id))
        .unwrap_or(Mode::Flat);

    let should_indent = if negate {
        group_mode == Mode::Flat
    } else {
        group_mode == Mode::Break
    };

    if should_indent {
        cmd.indented(contents)
    } else {
        cmd.with_doc(contents)
    }
}

//
// Public API
//

/// Convert an arena doc tree to a formatted string (starting at column 0).
pub fn arena_print_doc(arena: &DocArena, doc: DocId, embed: &EmbedContext) -> String {
    arena_print_doc_at_column(arena, doc, embed, 0)
}

/// Render with effectively infinite print width — every group flattens.
///
/// Used by callers that need to measure a doc's flat-layout width
/// (e.g., template literal type sizing). The renderer still uses
/// [`crate::TAB_WIDTH`] / [`crate::INDENT`].
pub fn arena_print_doc_flat_resolved<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    resolver: &R,
) -> String {
    let render = RenderConfig {
        print_width: usize::MAX / 2,
        ..RenderConfig::default()
    };
    let mut output = String::with_capacity(arena.estimated_output_capacity());
    let mut pos: usize = 0;

    render_doc_iterative(
        arena,
        doc,
        &mut output,
        &mut pos,
        0,
        &render,
        embed,
        Some(resolver),
    );

    trim_last_line(output)
}

/// Convert an arena doc tree to a formatted string, starting at a specific column.
pub fn arena_print_doc_at_column(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
) -> String {
    arena_print_doc_with_indent(arena, doc, embed, start_column, 0)
}

/// Convert an arena doc tree to a formatted string with column and indent level.
pub fn arena_print_doc_with_indent(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
) -> String {
    arena_print_doc_with_indent_and_render(
        arena,
        doc,
        embed,
        start_column,
        start_indent_level,
        &RenderConfig::default(),
    )
}

/// Convert an arena doc tree to a formatted string with column, indent, and symbol resolution.
pub fn arena_print_doc_with_indent_resolved<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let mut output = String::new();
    arena_print_doc_with_indent_resolved_into(
        arena,
        doc,
        embed,
        start_column,
        start_indent_level,
        resolver,
        &mut output,
    );
    output
}

/// Like [`arena_print_doc_with_indent_resolved`], rendering into a
/// caller-provided (empty) buffer — the seam behind the printers' pooled
/// render scratch ([`DocArena::take_render_scratch`]), so the per-statement
/// output `String` reuses one warm allocation instead of alloc/free per call.
/// Reserves [`DocArena::estimated_output_capacity`] itself (a no-op once the
/// pooled buffer is warm).
pub fn arena_print_doc_with_indent_resolved_into<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
    output: &mut String,
) {
    let render = RenderConfig::default();
    let mut pos: usize = start_column;

    output.reserve(arena.estimated_output_capacity());
    render_doc_iterative(
        arena,
        doc,
        output,
        &mut pos,
        start_indent_level,
        &render,
        embed,
        Some(resolver),
    );

    trim_last_line_in_place(output);
}

/// Convert an arena doc tree, preserving trailing whitespace on the last line
/// (for HTML `<pre>`, `<textarea>`, etc.). Interior non-literal lines are still
/// trimmed inline by `render_line_break`; only the final-line trim is skipped.
pub fn arena_print_doc_with_indent_resolved_preserve_whitespace<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let mut output = String::new();
    arena_print_doc_with_indent_resolved_preserve_whitespace_into(
        arena,
        doc,
        embed,
        start_column,
        start_indent_level,
        resolver,
        &mut output,
    );
    output
}

/// Like [`arena_print_doc_with_indent_resolved_preserve_whitespace`],
/// rendering into a caller-provided (empty) buffer — the pooled-scratch seam
/// (see [`arena_print_doc_with_indent_resolved_into`]). Reserves
/// [`DocArena::estimated_output_capacity`] itself.
pub fn arena_print_doc_with_indent_resolved_preserve_whitespace_into<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
    output: &mut String,
) {
    let render = RenderConfig::default();
    let mut pos: usize = start_column;

    output.reserve(arena.estimated_output_capacity());
    render_doc_iterative(
        arena,
        doc,
        output,
        &mut pos,
        start_indent_level,
        &render,
        embed,
        Some(resolver),
    );
}

/// Test-only entry point: render with explicit width/indent overrides.
///
/// Production callers should use [`arena_print_doc`] (which uses
/// [`crate::PRINT_WIDTH`] / [`crate::TAB_WIDTH`] / [`crate::INDENT`]).
pub(crate) fn arena_print_doc_with_indent_and_render(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    render: &RenderConfig,
) -> String {
    let mut output = String::with_capacity(arena.estimated_output_capacity());
    let mut pos: usize = start_column;

    render_doc_iterative::<dyn TextResolver>(
        arena,
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        render,
        embed,
        None,
    );

    trim_last_line(output)
}

//
// Core rendering
//

/// Renderer-specific behavior threaded through [`render_doc_core`].
///
/// The top-level renderer and the single-doc sub-renderer share one loop; the
/// divergences between them are enumerable and small, so each lives behind a
/// policy method (or const) that folds away after monomorphization — two
/// instantiations, the same codegen shape as the hand-duplicated loops this
/// replaces.
trait RenderPolicy {
    /// Whether a conditional group's own `should_break` short-circuits straight
    /// to its most-expanded state in break mode (Prettier's `if (doc.break)`).
    /// The single-doc sub-renderer predates that upgrade and runs the fits
    /// ladder regardless — preserved drift, kept exactly as it was (a
    /// conditional group with `should_break` inside a fill segment or line
    /// suffix has not been observed on fixtures/corpora; unify fixtures-first
    /// if one ever appears).
    const CONDITIONAL_GROUP_HONORS_SHOULD_BREAK: bool;

    /// Whether `line_suffix` content is deferred to the buffer and flushed at
    /// line breaks. When `false` (the suffix-flush sub-render), suffix content
    /// renders inline where it appears, groups pass through in the current
    /// mode without fits checks, and `WithContext` descends without its fill
    /// special case (the suffix was already measured where it was queued).
    fn tracking_suffix(&self) -> bool;

    /// The keyed-group mode map, when this renderer resolves keyed groups
    /// (top-level only). `None` makes an id-keyed `IfBreak`/`IndentIfBreak`
    /// read its group as unresolved → flat.
    fn group_mode_map(&self) -> Option<&GroupModeMap>;

    /// Record a keyed group's chosen mode (no-op without a map).
    fn record_group_mode(&mut self, id: Option<GroupId>, mode: Mode);

    /// The pending-command lookahead a `WithContext`-wrapped fill sees: the
    /// real command stack at top level, nothing in the single-doc sub-render.
    fn with_context_fill_rest<'a>(&self, commands: &'a [ArenaCommand]) -> &'a [ArenaCommand];

    // Opt-in swallow diagnostic hooks (`swallow_check` feature): live only on
    // the top-level policy — the sub-renders never carried the check. See
    // `crate::doc::swallow`.
    #[cfg(feature = "swallow_check")]
    fn swallow_enabled(&self) -> bool;
    #[cfg(feature = "swallow_check")]
    fn swallow_on_text(&mut self, is_line_comment: bool, text: &str, output: &str);
    #[cfg(feature = "swallow_check")]
    fn swallow_on_newline(&mut self, emitted: bool);
}

/// Policy for [`render_doc_iterative`]: resolves keyed groups into a
/// [`GroupModeMap`], always defers line suffixes, honors conditional-group
/// `should_break`, hands fills the real pending-command lookahead, and (under
/// the `swallow_check` feature) hosts the line-comment swallow diagnostic.
struct TopLevelPolicy {
    group_mode_map: GroupModeMap,
    #[cfg(feature = "swallow_check")]
    swallow: SwallowTracker,
}

impl RenderPolicy for TopLevelPolicy {
    const CONDITIONAL_GROUP_HONORS_SHOULD_BREAK: bool = true;

    #[inline]
    fn tracking_suffix(&self) -> bool {
        true
    }

    #[inline]
    fn group_mode_map(&self) -> Option<&GroupModeMap> {
        Some(&self.group_mode_map)
    }

    #[inline]
    fn record_group_mode(&mut self, id: Option<GroupId>, mode: Mode) {
        if let Some(group_id) = id {
            self.group_mode_map.insert(group_id, mode);
        }
    }

    #[inline]
    fn with_context_fill_rest<'a>(&self, commands: &'a [ArenaCommand]) -> &'a [ArenaCommand] {
        commands
    }

    #[cfg(feature = "swallow_check")]
    #[inline]
    fn swallow_enabled(&self) -> bool {
        self.swallow.enabled()
    }

    #[cfg(feature = "swallow_check")]
    #[inline]
    fn swallow_on_text(&mut self, is_line_comment: bool, text: &str, output: &str) {
        self.swallow.on_text(is_line_comment, text, output);
    }

    #[cfg(feature = "swallow_check")]
    #[inline]
    fn swallow_on_newline(&mut self, emitted: bool) {
        self.swallow.on_newline(emitted);
    }
}

/// Policy for [`render_single_doc_inner`] (fill segments and line-suffix
/// flush): no keyed-group map (keyed groups read as unresolved → flat), suffix
/// tracking only when the caller supplied a buffer, no conditional-group
/// `should_break` shortcut (preserved drift — see
/// [`RenderPolicy::CONDITIONAL_GROUP_HONORS_SHOULD_BREAK`]), and fills see no
/// pending-command lookahead through `WithContext`.
struct SingleDocPolicy {
    tracking_suffix: bool,
}

impl RenderPolicy for SingleDocPolicy {
    const CONDITIONAL_GROUP_HONORS_SHOULD_BREAK: bool = false;

    #[inline]
    fn tracking_suffix(&self) -> bool {
        self.tracking_suffix
    }

    #[inline]
    fn group_mode_map(&self) -> Option<&GroupModeMap> {
        None
    }

    #[inline]
    fn record_group_mode(&mut self, _id: Option<GroupId>, _mode: Mode) {}

    #[inline]
    fn with_context_fill_rest<'a>(&self, _commands: &'a [ArenaCommand]) -> &'a [ArenaCommand] {
        &[]
    }

    #[cfg(feature = "swallow_check")]
    #[inline]
    fn swallow_enabled(&self) -> bool {
        false
    }

    #[cfg(feature = "swallow_check")]
    #[inline]
    fn swallow_on_text(&mut self, _is_line_comment: bool, _text: &str, _output: &str) {}

    #[cfg(feature = "swallow_check")]
    #[inline]
    fn swallow_on_newline(&mut self, _emitted: bool) {}
}

/// Command-stack-based rendering with look-ahead — the top-level renderer
/// behind every `arena_print_doc*` entry point. Resolves keyed groups, defers
/// `line_suffix` content (flushed at line breaks and once at the end), and
/// (under the `swallow_check` feature) hosts the line-comment swallow
/// diagnostic. The loop itself is [`render_doc_core`].
#[allow(clippy::too_many_arguments)]
fn render_doc_iterative<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    start_indent_level: usize,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
) {
    // The swallow tracker (opt-in diagnostic) snapshots the process-global
    // enabled flag once per render and is inert when disabled. Compiled out
    // entirely without the feature. See `crate::doc::swallow`.
    let mut policy = TopLevelPolicy {
        group_mode_map: GroupModeMap::default(),
        #[cfg(feature = "swallow_check")]
        swallow: SwallowTracker::new(),
    };
    // Borrow the arena-pooled work buffers for the duration of this top-level
    // render: their spill capacity warms once per arena instead of
    // re-allocating per rendered piece. Sub-renders (fill segments,
    // line-suffix flushes) use their own inline locals, never these.
    let mut commands = arena.borrow_render_commands_scratch();
    let mut line_suffix = arena.borrow_line_suffix_scratch();
    let mut should_remeasure = false;

    render_doc_core(
        arena,
        doc,
        output,
        pos,
        start_indent_level,
        Mode::Break,
        render,
        embed,
        resolver,
        &mut policy,
        &mut commands,
        &mut line_suffix,
        &mut should_remeasure,
    );

    flush_line_suffix(
        arena,
        &mut line_suffix,
        output,
        pos,
        render,
        embed,
        resolver,
        &mut should_remeasure,
    );
}

/// The shared command-stack render loop with look-ahead — the single
/// implementation behind [`render_doc_iterative`] and
/// [`render_single_doc_inner`], parameterized by [`RenderPolicy`]. Pending
/// `line_suffix` content the loop didn't flush stays in the caller's buffer
/// (the top-level wrapper flushes it; the single-doc wrapper hands it back).
///
/// Tail-continuation dispatch: `cmd` is the command being processed; arms
/// that forward to exactly one child (Indent, Group, Concat's first child,
/// …) assign `cmd` and `continue` instead of pushing it — the pushed-last
/// command would be popped right back on the next iteration (LIFO), so this
/// skips that stack round trip (SmallVec spill checks both ways plus the
/// reload feeding the dispatch load chain). Traversal order is identical,
/// and `commands` holds the same pending set at every fits/fill lookahead
/// (those run before the continuation would have been pushed). Only
/// terminal arms (Text, Line, Fill, …) fall through to the pop at the
/// bottom of the loop.
#[allow(clippy::too_many_arguments)]
fn render_doc_core<R: TextResolver + ?Sized, P: RenderPolicy>(
    arena: &DocArena,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
    policy: &mut P,
    commands: &mut CmdStack,
    line_suffix: &mut LineSuffixBuf,
    should_remeasure: &mut bool,
) {
    // The loop's termination condition is `commands` draining back to empty,
    // so the caller-provided (pooled or local) stack must start empty.
    debug_assert!(commands.is_empty());
    let mut cmd = ArenaCommand {
        indent: indent_level,
        mode,
        doc,
    };

    // Hoist arena borrows out of the loop: the arena is read-only during
    // rendering, so a single immutable borrow held for the whole render
    // avoids the per-iteration dynamic borrow-check cost.
    let nodes_outer = arena.borrow_nodes();
    let children_outer = arena.borrow_children();
    let pool_outer = arena.borrow_text_pool();
    let nodes: &[DocNode] = &nodes_outer;
    let children_vec: &[DocId] = &children_outer;
    let pool: &str = &pool_outer;

    loop {
        match &nodes[cmd.doc.index()] {
            DocNode::Text(t) => {
                #[cfg(feature = "swallow_check")]
                if policy.swallow_enabled() {
                    let s = resolve_text(t, resolver, pool);
                    policy.swallow_on_text(arena.is_line_comment(cmd.doc), s, output);
                }
                render_text(t, output, pos, resolver, pool);
            }

            DocNode::MultilineText { span, .. } => {
                // Render `[text(line0), hardline, text(line1), hardline, …]` from
                // one pool-stored body: the first line at the current column, each
                // subsequent line preceded by the hardline path (flush pending
                // line suffix, trim, newline, context indent). Byte- and
                // position-identical to the per-line concat it replaces.
                let mut lines = span.slice(pool).split('\n');
                if let Some(first) = lines.next() {
                    #[cfg(feature = "swallow_check")]
                    if policy.swallow_enabled() {
                        // Block-comment text is never a `//` line comment.
                        policy.swallow_on_text(false, first, output);
                    }
                    output.push_str(first);
                    update_pos_for_text(pos, first);
                }
                for line in lines {
                    // Hardline (breaks in either mode): flush suffix, then break.
                    // Forced out in flat mode, it invalidates the enclosing fits
                    // approval — arm the remeasure flag (see the `Line` arm).
                    if cmd.mode == Mode::Flat {
                        *should_remeasure = true;
                    }
                    if policy.tracking_suffix() {
                        flush_line_suffix(
                            arena,
                            line_suffix,
                            output,
                            pos,
                            render,
                            embed,
                            resolver,
                            should_remeasure,
                        );
                    }
                    render_line_break(
                        LineKind::Hard,
                        cmd.mode,
                        cmd.indent,
                        output,
                        pos,
                        render,
                        embed,
                    );
                    #[cfg(feature = "swallow_check")]
                    {
                        policy.swallow_on_newline(true);
                        if policy.swallow_enabled() {
                            policy.swallow_on_text(false, line, output);
                        }
                    }
                    output.push_str(line);
                    update_pos_for_text(pos, line);
                }
            }

            DocNode::Line(kind) => {
                let kind = *kind;
                let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
                // A hard line forced out in flat mode: the enclosing fits
                // approval measured only up to here (a hard line ends a fits
                // walk early), so positions beyond it are unmeasured — the next
                // group must remeasure no matter what (Prettier's
                // `shouldRemeasure`, printer.js `DOC_TYPE_LINE` flat arm).
                if is_hard && cmd.mode == Mode::Flat {
                    *should_remeasure = true;
                }
                if policy.tracking_suffix() && (cmd.mode == Mode::Break || is_hard) {
                    flush_line_suffix(
                        arena,
                        line_suffix,
                        output,
                        pos,
                        render,
                        embed,
                        resolver,
                        should_remeasure,
                    );
                }
                // A real newline ends the comment's line → clears the pending swallow.
                let emitted_newline =
                    render_line_break(kind, cmd.mode, cmd.indent, output, pos, render, embed);
                #[cfg(feature = "swallow_check")]
                policy.swallow_on_newline(emitted_newline);
                #[cfg(not(feature = "swallow_check"))]
                let _ = emitted_newline;
            }

            DocNode::Indent(inner) => {
                let inner = *inner;
                cmd = cmd.indented(inner);
                continue;
            }

            DocNode::Dedent(inner) => {
                let inner = *inner;
                cmd = cmd.dedented(inner);
                continue;
            }

            DocNode::Align { n, contents } => {
                let n = *n;
                let contents = *contents;
                cmd = cmd.with_indent(n, contents);
                continue;
            }

            DocNode::Group {
                contents,
                expanded_states,
                id,
                should_break,
            } => {
                let contents = *contents;
                let expanded_states = *expanded_states;
                let id = *id;
                let should_break = *should_break;

                if !policy.tracking_suffix() {
                    // Suffix-flush render: pass through in the current mode,
                    // no fits checks.
                    cmd = cmd.with_doc(contents);
                    continue;
                }

                let (chosen_mode, chosen_doc) = if !expanded_states.is_empty() {
                    // conditionalGroup: try each state until one fits.
                    // Prettier: only use most expanded when group's OWN should_break is true.
                    // Parent mode being Break does NOT skip the fits check — conditional
                    // groups always try flat first, even inside a MODE_BREAK parent.
                    // (Deliberately outside the flat-mode fits-skip below: Prettier's
                    // pass-through would render `contents` — the least-expanded state —
                    // where tsv's measured ladder can pick a later state; conditional
                    // groups are rare enough that skipping their re-measure isn't worth
                    // that divergence risk.)
                    if P::CONDITIONAL_GROUP_HONORS_SHOULD_BREAK && should_break {
                        // Prettier: if (doc.break) → use most expanded in break mode
                        let states = expanded_states.resolve(children_vec);
                        (Mode::Break, states.last().copied().unwrap_or(contents))
                    } else {
                        // Fits check regardless of parent mode — matches Prettier
                        let remaining = remaining_width(*pos, render, embed);

                        let contents_fit = arena_fits_with_lookahead(
                            arena,
                            contents,
                            Mode::Flat,
                            commands,
                            remaining,
                            embed,
                            resolver,
                        );

                        if contents_fit {
                            *should_remeasure = false;
                            (Mode::Flat, contents)
                        } else {
                            // Try each earlier state flat, in order; the final
                            // state is the Break fallback (`states` is non-empty
                            // — the `!expanded_states.is_empty()` guard above).
                            let states = expanded_states.resolve(children_vec);
                            let last = states.len() - 1;
                            let mut chosen = (Mode::Break, states[last]);
                            for &state in &states[..last] {
                                let state_fits = arena_fits_with_lookahead(
                                    arena,
                                    state,
                                    Mode::Flat,
                                    commands,
                                    remaining,
                                    embed,
                                    resolver,
                                );
                                if state_fits {
                                    *should_remeasure = false;
                                    chosen = (Mode::Flat, state);
                                    break;
                                }
                            }
                            chosen
                        }
                    }
                } else if should_break || arena.will_break(contents) {
                    (Mode::Break, contents)
                } else if cmd.mode == Mode::Flat && !*should_remeasure {
                    // Prettier's printGroup flat pass-through (printer.js
                    // `mode === MODE_FLAT && !shouldRemeasure`): a group reached in
                    // flat mode sits inside a subtree some enclosing fits approval
                    // already measured flat — with look-ahead through the same
                    // pending commands — so re-measuring here returns true by
                    // construction and the fits walk is skipped. The approval's
                    // accounting holds until a hard line is forced out in flat mode
                    // (a fits walk ends at a hard line, leaving everything beyond
                    // it unmeasured): that arms `should_remeasure` (the `Line` /
                    // `MultilineText` arms, plus the fill renderer's unmeasured
                    // flat entries), and the next measured fits-true clears it.
                    (Mode::Flat, contents)
                } else {
                    let fits = arena_fits_with_lookahead(
                        arena,
                        contents,
                        Mode::Flat,
                        commands,
                        remaining_width(*pos, render, embed),
                        embed,
                        resolver,
                    );
                    if fits {
                        *should_remeasure = false;
                    }
                    (if fits { Mode::Flat } else { Mode::Break }, contents)
                };

                policy.record_group_mode(id, chosen_mode);
                cmd = cmd.with_mode(chosen_mode, chosen_doc);
                continue;
            }

            DocNode::IsolatedGroup { contents } => {
                let contents = *contents;

                if !policy.tracking_suffix() {
                    // Suffix-flush render: pass through in the current mode.
                    cmd = cmd.with_doc(contents);
                    continue;
                }

                let fits = arena_fits_with_lookahead(
                    arena,
                    contents,
                    Mode::Flat,
                    commands,
                    remaining_width(*pos, render, embed),
                    embed,
                    resolver,
                );
                let chosen_mode = if fits { Mode::Flat } else { Mode::Break };
                cmd = cmd.with_mode(chosen_mode, contents);
                continue;
            }

            DocNode::IfBreak {
                break_doc,
                flat_doc,
                group_id,
            } => {
                // Without a group map (the single-doc sub-renders), a keyed
                // if_break treats its group as unresolved → flat, matching how
                // IndentIfBreak defaults below.
                let broke = match group_id {
                    Some(gid) => {
                        policy
                            .group_mode_map()
                            .and_then(|map| map.get(*gid))
                            .unwrap_or(Mode::Flat)
                            == Mode::Break
                    }
                    None => cmd.mode == Mode::Break,
                };
                let chosen = if broke { *break_doc } else { *flat_doc };
                cmd = cmd.with_doc(chosen);
                continue;
            }

            DocNode::IndentIfBreak {
                contents,
                group_id,
                negate,
            } => {
                let contents = *contents;
                let group_id = *group_id;
                let negate = *negate;
                cmd = process_indent_if_break(
                    contents,
                    group_id,
                    negate,
                    policy.group_mode_map(),
                    &cmd,
                );
                continue;
            }

            DocNode::Concat(range) => {
                let kids = range.resolve(children_vec);
                if let Some((&first, rest)) = kids.split_first() {
                    for &child in rest.iter().rev() {
                        commands.push(cmd.with_doc(child));
                    }
                    cmd = cmd.with_doc(first);
                    continue;
                }
            }

            DocNode::Fill(range) => {
                let parts = range.resolve(children_vec);
                render_fill_iterative(
                    arena,
                    parts,
                    output,
                    pos,
                    cmd.indent,
                    render,
                    embed,
                    &DocContext::default(),
                    commands,
                    resolver,
                    should_remeasure,
                );
            }

            DocNode::WithContext { doc, context } => {
                let inner_doc = *doc;

                if policy.tracking_suffix() {
                    if let DocNode::Fill(fill_range) = &nodes[inner_doc.index()] {
                        let context = context.clone();
                        let parts = fill_range.resolve(children_vec);
                        render_fill_iterative(
                            arena,
                            parts,
                            output,
                            pos,
                            cmd.indent,
                            render,
                            embed,
                            &context,
                            policy.with_context_fill_rest(commands),
                            resolver,
                            should_remeasure,
                        );
                    } else {
                        cmd = cmd.with_doc(inner_doc);
                        continue;
                    }
                } else {
                    // Suffix-flush render: descend without the fill special case.
                    cmd = cmd.with_doc(inner_doc);
                    continue;
                }
            }

            DocNode::LineSuffix(inner) => {
                let inner = *inner;
                if policy.tracking_suffix() {
                    line_suffix.push(cmd.with_doc(inner));
                } else {
                    // Suffix-flush render: render suffix content inline.
                    cmd = cmd.with_doc(inner);
                    continue;
                }
            }

            DocNode::LineSuffixBoundary => {
                if policy.tracking_suffix() {
                    flush_line_suffix(
                        arena,
                        line_suffix,
                        output,
                        pos,
                        render,
                        embed,
                        resolver,
                        should_remeasure,
                    );
                }
            }

            DocNode::BreakParent => {
                // No-op during rendering
            }
        }

        // Terminal arm: take the next pending command off the stack.
        match commands.pop() {
            Some(next) => cmd = next,
            None => break,
        }
    }
}

/// Render a single doc with specified mode (helper for Fill).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_single_doc<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
    should_remeasure: &mut bool,
) {
    let mut line_suffix: LineSuffixBuf = SmallVec::new();
    render_single_doc_inner(
        arena,
        doc,
        output,
        pos,
        indent_level,
        mode,
        render,
        embed,
        resolver,
        Some(&mut line_suffix),
        should_remeasure,
    );
    flush_line_suffix(
        arena,
        &mut line_suffix,
        output,
        pos,
        render,
        embed,
        resolver,
        should_remeasure,
    );
}

/// Unified single-doc renderer with optional suffix handling — the
/// sub-renderer behind fill segments ([`render_single_doc`]) and line-suffix
/// flushing (`suffix_buffer: None`, which renders suffix content inline). The
/// loop itself is [`render_doc_core`]; see [`SingleDocPolicy`] for what this
/// render does and doesn't do.
///
/// This wrapper looks dissolvable (its two callers could construct their own
/// policy and call [`render_doc_core`] directly), but that shape measured as
/// an instruction regression on every corpus — giving `render_doc_core`'s
/// single-doc instantiation two call sites flips its inlining and puts a call
/// on the hot per-line-break suffix-flush path. Keep the wrapper; re-attempt
/// only with an instruction-count gate.
#[allow(clippy::too_many_arguments)]
fn render_single_doc_inner<R: TextResolver + ?Sized>(
    arena: &DocArena,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    render: &RenderConfig,
    embed: &EmbedContext,
    resolver: Option<&R>,
    suffix_buffer: Option<&mut LineSuffixBuf>,
    should_remeasure: &mut bool,
) {
    let mut policy = SingleDocPolicy {
        tracking_suffix: suffix_buffer.is_some(),
    };
    let mut dummy_suffix: LineSuffixBuf = SmallVec::new();
    let line_suffix = suffix_buffer.unwrap_or(&mut dummy_suffix);

    // Sub-renders keep a local inline stack (measured allocation-free — the
    // common single-doc render never spills) rather than borrowing the pooled
    // one, which the enclosing top-level render already holds.
    let mut commands: CmdStack = SmallVec::new();
    render_doc_core(
        arena,
        doc,
        output,
        pos,
        indent_level,
        mode,
        render,
        embed,
        resolver,
        &mut policy,
        &mut commands,
        line_suffix,
        should_remeasure,
    );
}

//
// Utilities
//

pub(super) fn write_indentation(
    output: &mut String,
    level: usize,
    render: &RenderConfig,
    embed: &EmbedContext,
) {
    let extra = if embed.first_line_offset > 0 {
        embed.base_indent_offset
    } else {
        0
    };
    for _ in 0..(level + extra) {
        output.push_str(render.indent);
    }
}

fn indent_width(level: usize, render: &RenderConfig) -> usize {
    level * indent_str_width(render.indent)
}

pub(super) fn line_start_column(
    indent_level: usize,
    render: &RenderConfig,
    embed: &EmbedContext,
) -> usize {
    indent_width(indent_level, render) + embed.base_indent_offset * TAB_WIDTH
}

fn indent_str_width(indent: &str) -> usize {
    indent
        .chars()
        .map(|ch| if ch == '\t' { TAB_WIDTH } else { 1 })
        .sum()
}
