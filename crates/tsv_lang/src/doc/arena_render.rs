//! Rendering algorithm for arena-based document trees.

use crate::EmbedContext;
use crate::config::TAB_WIDTH;
use crate::printing::visual_width;
use smallvec::SmallVec;

use super::arena::{ArenaCommand, CmdStack, DocArena, DocId, DocNode, LineSuffixBuf, RenderIndent};
use super::arena_fits::arena_fits_with_lookahead;
use super::arena_render_fill::render_fill_iterative;
use super::arena_render_suffix::flush_line_suffix;
use super::render_config::RenderConfig;
#[cfg(feature = "comment_check")]
use super::render_config::RenderPurpose;
use super::specialize_short_len;
#[cfg(feature = "swallow_check")]
use super::swallow::{self, SwallowTracker};
use super::types::{CachedWidth, DocContext, GroupId, LineKind, Mode, resolve_text};
#[cfg(feature = "comment_check")]
use crate::comment_ledger;

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

//
// Shared rendering helpers
//

/// The invariant context of one render pass: everything the render path needs that does not
/// change as the doc tree is walked. Bundled so the mutually-recursive render functions pass one
/// `&RenderCtx` instead of threading four parameters through every call — this is what retires
/// the `clippy::too_many_arguments` allows across this module.
///
/// Deliberately holds **only shared references**. The mutable render state (`output`, `pos`,
/// `should_remeasure`) stays as separate `&mut` parameters: bundling those behind a struct
/// pointer would take their address and sink them out of registers in the hot loop. A `&RenderCtx`
/// has no aliasing writes through it, so its field loads hoist freely — and `render_doc_core`
/// already hoists the arena borrows into locals for the loop body regardless.
pub(super) struct RenderCtx<'a> {
    pub(super) arena: &'a DocArena,
    pub(super) render: &'a RenderConfig,
    pub(super) embed: &'a EmbedContext,
    pub(super) source: Option<&'a str>,
}

/// The doc a conditional-group state contributes to the fits ladder, or `None`
/// when the state is inadmissible and the ladder must skip it.
///
/// Every state but a [`DocNode::GatedState`] is measured as itself. A gated state
/// carries a never-fits admission probe: it stands for a layout that is correct
/// only because that probe CANNOT be laid flat on the fallback's continuation line
/// — one indent level deeper than the group, completed by the same rest commands.
/// So the probe's line is measured first, and a probe that *fits* there withdraws
/// the state: the fallback layout it would have stolen is the settled form.
///
/// The probe measurement passes `has_line_suffix: false`, unlike the caller's fits
/// on the returned state. That one measures from the CURRENT column, where a
/// pending deferred run is still pending; the probe measures a line the fallback
/// reaches only past a hardline, which flushes the run first. Passing the live flag
/// here would charge the probe for a suffix that cannot still be pending on the
/// line it stands for.
#[inline]
fn admissible_group_state(
    ctx: &RenderCtx<'_>,
    nodes: &[DocNode],
    state: DocId,
    indent: RenderIndent,
    rest_commands: &[ArenaCommand],
) -> Option<DocId> {
    let DocNode::GatedState { probe, contents } = &nodes[state.index()] else {
        return Some(state);
    };
    let fresh_pos = line_start_column(indent.indented(), ctx.render, ctx.embed);
    let probe_fits = arena_fits_with_lookahead(
        ctx.arena,
        *probe,
        Mode::Flat,
        rest_commands,
        remaining_width(fresh_pos, ctx.render, ctx.embed),
        false,
        ctx.source,
    );
    (!probe_fits).then_some(*contents)
}

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
#[expect(clippy::inline_always)]
#[inline(always)]
fn render_text(
    text: &super::types::DocText,
    output: &mut String,
    pos: &mut usize,
    source: Option<&str>,
    pool: &str,
) {
    let s = resolve_text(text, source, pool);
    // The render loop's output write: 857 K calls per pass over fuz_app/src move
    // a **mean of 4.69 bytes**, 5.1% of them move zero, and 87.5% move at most
    // eight — so the `memcpy` call was consistently larger than its payload.
    // Nine is where the curve flattens, measured, not guessed: `[0..=16]` bought
    // a further −0.013% for +1,712 B of `.text`.
    specialize_short_len!(s.len(), [0, 1, 2, 3, 4, 5, 6, 7, 8], output.push_str(s));
    match text.cached_width() {
        CachedWidth::Width(w) => *pos += w as usize, // Common path: no visual_width call
        CachedWidth::HasNewline => update_pos_for_text_unicode(pos, s),
        CachedWidth::NotComputed => update_pos_for_text(pos, s),
    }
}

/// Update position after rendering a text string, accounting for tab expansion.
///
/// The overwhelmingly common input here is short ASCII with no newline — every
/// span-identity identifier name (`source_span_ident`) reaches this via
/// `render_text`'s uncached-width arm
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
/// non-literal newline to strip trailing indentation/spaces from code lines, and
/// once more when a render finishes, as the **final-line** trim.
///
/// ⚠️ **Those two are one question, and the final-line trim's "find the last line
/// first" step was inert.** It used to `rposition` back to the last `\n` and trim
/// only the slice after it. But this walk steps over `' '` and `'\t'` and nothing
/// else, and `'\n'` is neither — so it halts at the last line's start on its own,
/// whatever precedes it, and the search could only ever re-derive where it was
/// going to stop anyway. Brute-forced over every string of ≤ 4 symbols drawn from
/// an alphabet spanning both trimmed bytes, `\n`, `\r`, `U+2028` and multi-byte
/// UTF-8: the two spellings agree everywhere. (Interior lines are trimmed at
/// their own break, so reaching one again would be a no-op regardless; the point
/// is that it *cannot* reach one.)
///
/// One of the printer's hottest calls: every non-literal line break of every
/// document reaches it (140,851 a fuz_app pass). So it is spelled as a guarded
/// reverse **byte** scan rather than `str::trim_end_matches([' ', '\t'])`, which
/// pays twice for what is usually a no-op — the array pattern searches backwards
/// over `char_indices`, decoding UTF-8 that the two ASCII bytes it seeks can
/// never be part of, and the `truncate` runs whether or not the length moved. A
/// formatted line ends in content, so the common answer is "nothing to trim", and
/// the guard turns that answer into one load and two compares.
#[inline]
pub(super) fn trim_trailing_whitespace(output: &mut String) {
    let bytes = output.as_bytes();
    let Some(&last) = bytes.last() else { return };
    if last != b' ' && last != b'\t' {
        return;
    }
    // Only ASCII bytes are ever stepped over, so `end` lands exactly where a
    // character ends — the same index `trim_end_matches` would have computed.
    let mut end = bytes.len() - 1;
    while end > 0 && matches!(bytes[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    output.truncate(end);
}

/// Render a line break.
#[inline]
pub(super) fn render_line_break(
    kind: LineKind,
    mode: Mode,
    indent: RenderIndent,
    output: &mut String,
    pos: &mut usize,
    render: &RenderConfig,
    embed: &EmbedContext,
) -> bool {
    let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
    if mode == Mode::Break || is_hard {
        // The one newline seam in the renderer, so the swallow diagnostic's "the line
        // ended" signal belongs here rather than on a per-render handle — see
        // [`swallow::note_line_end`].
        #[cfg(feature = "swallow_check")]
        swallow::note_line_end();
        if kind == LineKind::Literal {
            // Literal line (template literals): preserve trailing whitespace
            output.push('\n');
            *pos = 0;
        } else {
            // Non-literal line: trim trailing whitespace before newline
            // (matches Prettier's trim() call before non-literal newlines)
            trim_trailing_whitespace(output);
            output.push('\n');
            write_indentation(output, indent, render, embed);
            *pos = line_start_column(indent, render, embed);
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

/// Render a `Line` command whole: the remeasure obligation a hard line forced out
/// in flat mode carries, the pending-suffix flush, and the break itself.
///
/// Three arms reach it. `DocNode::Line` is the obvious one, and `DocNode::MultilineText`
/// takes it with [`LineKind::Hard`] for each interior newline of its body.
/// `DocNode::LineSuffixBoundary` is the third, and going through *this* function is
/// what makes it a boundary rather than a bare flush: Prettier renders that node by pushing a
/// `hardlineWithoutBreakParent` command and letting its own `Line` arm handle it
/// (`printer.js` `DOC_TYPE_LINE_SUFFIX_BOUNDARY`), which is exactly this call with
/// [`LineKind::Hard`]. tsv can't push the node — the arena is immutably borrowed for
/// the whole render loop — so it calls the arm instead.
// Remaining args are the mutable render state, deliberately unbundled — see
// `render_doc_core`.
#[expect(clippy::too_many_arguments)]
#[inline]
fn render_line_node(
    ctx: &RenderCtx<'_>,
    kind: LineKind,
    mode: Mode,
    indent: RenderIndent,
    output: &mut String,
    pos: &mut usize,
    tracking_suffix: bool,
    line_suffix: &mut LineSuffixBuf,
    should_remeasure: &mut bool,
) {
    let is_hard = matches!(kind, LineKind::Hard | LineKind::Literal);
    // A hard line forced out in flat mode: the enclosing fits approval measured
    // only up to here (a hard line ends a fits walk early), so positions beyond
    // it are unmeasured — the next group must remeasure no matter what
    // (Prettier's `shouldRemeasure`, printer.js `DOC_TYPE_LINE` flat arm).
    if is_hard && mode == Mode::Flat {
        *should_remeasure = true;
    }
    if tracking_suffix && (mode == Mode::Break || is_hard) {
        flush_line_suffix(ctx, line_suffix, output, pos, should_remeasure, indent);
    }
    // A real newline ends the comment's line → clears the pending swallow, inside
    // `render_line_break` itself.
    render_line_break(kind, mode, indent, output, pos, ctx.render, ctx.embed);
}

/// Process an IndentIfBreak node.
#[inline]
fn process_indent_if_break(
    contents: DocId,
    group_id: GroupId,
    group_mode_map: Option<&GroupModeMap>,
    cmd: &ArenaCommand,
) -> ArenaCommand {
    let group_mode = group_mode_map
        .and_then(|map| map.get(group_id))
        .unwrap_or(Mode::Flat);

    if group_mode == Mode::Break {
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
    arena_print_doc_with_indent_and_render(arena, doc, embed, 0, 0, &RenderConfig::default())
}

/// **Measure** a doc's flat-layout width: render at effectively infinite print width, so
/// every group flattens. The renderer still uses [`crate::TAB_WIDTH`] / [`crate::INDENT`].
///
/// ⚠️ The result is for measuring, **never** for output — it renders with
/// `RenderPurpose::Measure`, so a comment reached in `doc` is deliberately *not* recorded
/// as emitted. Writing this string into the document would make every comment it covers read
/// as DROPPED to the ledger (and, if the real render also runs, DOUBLE-PRINTED). Use a
/// `arena_print_doc_*` entry to produce output.
pub fn arena_measure_doc_flat_resolved(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    source: &str,
) -> String {
    let render = RenderConfig {
        print_width: usize::MAX / 2,
        // Measured and discarded, never written to the document — so reaching a comment's
        // node here is not that comment being emitted. See `RenderPurpose`.
        #[cfg(feature = "comment_check")]
        purpose: RenderPurpose::Measure,
        ..RenderConfig::default()
    };
    let mut output = String::with_capacity(arena.estimated_output_capacity());
    let mut pos: usize = 0;

    render_doc_iterative(
        &RenderCtx {
            arena,
            render: &render,
            embed,
            source: Some(source),
        },
        doc,
        &mut output,
        &mut pos,
        0,
    );

    trim_trailing_whitespace(&mut output);
    output
}

/// Render an arena doc tree (with column, indent, and source-span resolution)
/// into a caller-provided (empty) buffer — the seam behind the printers' pooled
/// render scratch ([`DocArena::take_render_scratch`]), so the per-statement
/// output `String` reuses one warm allocation instead of alloc/free per call.
/// Reserves [`DocArena::estimated_output_capacity`] itself (a no-op once the
/// pooled buffer is warm).
///
/// ⚠️ **Empty is a requirement, not a convention.** [`trim_trailing_whitespace`]
/// — run before every non-literal line break, and once more below as the
/// final-line trim — walks backwards from the end with no floor, so it will strip
/// text sitting in front of the doc's first byte. Every other `output.len()` the
/// render loop takes is a marker captured mid-render and is already relative, so an empty
/// base is the whole of the invariant — which is what lets a printer whose
/// buffer *is* empty pass that buffer directly and skip the copy back
/// (`OutputBuffer::as_empty_render_target`).
pub fn arena_print_doc_with_indent_resolved_into(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    source: &str,
    output: &mut String,
) {
    let render = RenderConfig::default();
    let mut pos: usize = start_column;

    output.reserve(arena.estimated_output_capacity());
    render_doc_iterative(
        &RenderCtx {
            arena,
            render: &render,
            embed,
            source: Some(source),
        },
        doc,
        output,
        &mut pos,
        start_indent_level,
    );

    trim_trailing_whitespace(output);
}

/// Render an arena doc tree into a caller-provided (empty) buffer, preserving
/// trailing whitespace on the last line (for HTML `<pre>`, `<textarea>`, etc.).
/// Interior non-literal lines are still trimmed inline by `render_line_break`;
/// only the final-line trim is skipped. The pooled-scratch seam (see
/// [`arena_print_doc_with_indent_resolved_into`]); reserves
/// [`DocArena::estimated_output_capacity`] itself.
pub fn arena_print_doc_with_indent_resolved_preserve_whitespace_into(
    arena: &DocArena,
    doc: DocId,
    embed: &EmbedContext,
    start_column: usize,
    start_indent_level: usize,
    source: &str,
    output: &mut String,
) {
    let render = RenderConfig::default();
    let mut pos: usize = start_column;

    output.reserve(arena.estimated_output_capacity());
    render_doc_iterative(
        &RenderCtx {
            arena,
            render: &render,
            embed,
            source: Some(source),
        },
        doc,
        output,
        &mut pos,
        start_indent_level,
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

    render_doc_iterative(
        &RenderCtx {
            arena,
            render,
            embed,
            source: None,
        },
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
    );

    trim_trailing_whitespace(&mut output);
    output
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

    // Opt-in swallow diagnostic hooks (`swallow_check` feature). Both policies carry
    // them: a swallow is a property of the physical output line, and the sub-renders
    // append to the same line as the main loop, so every renderer drives the one
    // shared state machine. See `crate::doc::swallow`.
    #[cfg(feature = "swallow_check")]
    fn swallow_enabled(&self) -> bool;
    #[cfg(feature = "swallow_check")]
    fn swallow_on_text(&mut self, is_line_comment: bool, text: &str, output: &str);
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
}

/// Policy for [`render_single_doc_inner`] (fill segments and line-suffix
/// flush): no keyed-group map (keyed groups read as unresolved → flat), suffix
/// tracking only when the caller supplied a buffer, no conditional-group
/// `should_break` shortcut (preserved drift — see
/// [`RenderPolicy::CONDITIONAL_GROUP_HONORS_SHOULD_BREAK`]), and fills see no
/// pending-command lookahead through `WithContext`.
struct SingleDocPolicy {
    tracking_suffix: bool,
    /// Joins the enclosing render's swallow state machine — see
    /// [`SwallowTracker::join_render`].
    #[cfg(feature = "swallow_check")]
    swallow: SwallowTracker,
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

    // A sub-render appends to the same physical output line as the main loop, so it
    // drives the same state machine rather than opting out. Without this the
    // line-suffix flush was a blind spot: two trailing `//` comments flushed at one
    // line break land back-to-back (`x; // c1 // c2`) and the first swallows the
    // second.
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
}

/// Command-stack-based rendering with look-ahead — the top-level renderer
/// behind every `arena_print_doc*` entry point. Resolves keyed groups, defers
/// `line_suffix` content (flushed at line breaks and once at the end), and
/// (under the `swallow_check` feature) hosts the line-comment swallow
/// diagnostic. The loop itself is [`render_doc_core`].
fn render_doc_iterative(
    ctx: &RenderCtx<'_>,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    start_indent_level: usize,
) {
    let arena = ctx.arena;
    // The swallow tracker (opt-in diagnostic) snapshots the process-global
    // enabled flag once per render and is inert when disabled. Compiled out
    // entirely without the feature. See `crate::doc::swallow`.
    let mut policy = TopLevelPolicy {
        group_mode_map: GroupModeMap::default(),
        #[cfg(feature = "swallow_check")]
        swallow: SwallowTracker::begin_render(),
    };
    // Borrow the arena-pooled work buffers for the duration of this top-level
    // render: their spill capacity warms once per arena instead of
    // re-allocating per rendered piece. Sub-renders (fill segments,
    // line-suffix flushes) use their own inline locals, never these.
    let mut commands = arena.borrow_render_commands_scratch();
    let mut line_suffix = arena.borrow_line_suffix_scratch();
    let mut should_remeasure = false;

    render_doc_core(
        ctx,
        doc,
        output,
        pos,
        RenderIndent::level(start_indent_level),
        Mode::Break,
        &mut policy,
        &mut commands,
        &mut line_suffix,
        &mut should_remeasure,
    );

    flush_line_suffix(
        ctx,
        &mut line_suffix,
        output,
        pos,
        &mut should_remeasure,
        RenderIndent::level(start_indent_level),
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
// Remaining args are the MUTABLE render state (`output`/`pos`/`should_remeasure`, plus the
// work buffers). Deliberately not bundled: a struct would take their address and sink them out
// of registers in the hot loop — see `RenderCtx`, which carries only the shared context.
#[expect(clippy::too_many_arguments)]
fn render_doc_core<P: RenderPolicy>(
    ctx: &RenderCtx<'_>,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    indent: RenderIndent,
    mode: Mode,
    policy: &mut P,
    commands: &mut CmdStack,
    line_suffix: &mut LineSuffixBuf,
    should_remeasure: &mut bool,
) {
    let &RenderCtx {
        arena,
        render,
        embed,
        source,
    } = ctx;
    // The loop's termination condition is `commands` draining back to empty,
    // so the caller-provided (pooled or local) stack must start empty.
    debug_assert!(commands.is_empty());
    let mut cmd = ArenaCommand { indent, mode, doc };

    // Pre-intern the flow-probe sentinel BEFORE taking the loop-long node borrow below —
    // interning allocates into the node store, which must not happen mid-render (the
    // whole-render immutable borrow would panic). One generation-gated cell hit after the
    // first call, so an unprobed document pays a single node once.
    let flow_probe_end = arena.flow_probe_end_node();

    // Hoist arena borrows out of the loop: the arena is read-only during
    // rendering, so a single immutable borrow held for the whole render
    // avoids the per-iteration dynamic borrow-check cost.
    let nodes_outer = arena.borrow_nodes();
    let children_outer = arena.borrow_children();
    let pool_outer = arena.borrow_text_pool();
    let nodes: &[DocNode] = &nodes_outer;
    let children_vec: &[DocId] = &children_outer;
    let pool: &str = &pool_outer;

    // The print-once comment ledger's render-side hook (`comment_check` feature). Every
    // command popped here is a node the renderer *emits* — a conditional-group candidate
    // that loses, or a `fits()` lookahead, never reaches this loop — so recording the tag
    // here is the emit itself. Gated on the arena actually carrying tags, so a
    // comment-free document pays nothing. See `crate::comment_ledger`.
    #[cfg(feature = "comment_check")]
    let ledger_on = comment_ledger::comment_check_enabled()
        && arena.has_comment_docs()
        && render.purpose.records_comment_emits();

    loop {
        #[cfg(feature = "comment_check")]
        if ledger_on && let Some((span, key)) = arena.comment_doc_tag(cmd.doc) {
            comment_ledger::record_emitted_keyed(key, span);
        }

        match &nodes[cmd.doc.index()] {
            DocNode::Text(t) => {
                #[cfg(feature = "swallow_check")]
                if policy.swallow_enabled() {
                    let s = resolve_text(t, source, pool);
                    policy.swallow_on_text(arena.is_line_comment(cmd.doc), s, output);
                }
                render_text(t, output, pos, source, pool);
            }

            DocNode::MultilineText { span, .. } => {
                // Render `[text(line0), hardline, text(line1), hardline, …]` from
                // one pool-stored body: the first line at the current column, each
                // subsequent line preceded by the hardline arm (`render_line_node`
                // with `Hard`: remeasure arming, suffix flush, break). Byte- and
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
                    render_line_node(
                        ctx,
                        LineKind::Hard,
                        cmd.mode,
                        cmd.indent,
                        output,
                        pos,
                        policy.tracking_suffix(),
                        line_suffix,
                        should_remeasure,
                    );
                    #[cfg(feature = "swallow_check")]
                    if policy.swallow_enabled() {
                        policy.swallow_on_text(false, line, output);
                    }
                    output.push_str(line);
                    update_pos_for_text(pos, line);
                }
            }

            DocNode::Line(kind) => {
                let kind = *kind;
                render_line_node(
                    ctx,
                    kind,
                    cmd.mode,
                    cmd.indent,
                    output,
                    pos,
                    policy.tracking_suffix(),
                    line_suffix,
                    should_remeasure,
                );
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

            DocNode::AlignRoot { n, contents } => {
                let n = *n;
                let contents = *contents;
                cmd = cmd.reset_to_level(n, contents);
                continue;
            }

            DocNode::Align { n, contents } => {
                let n = *n;
                let contents = *contents;
                cmd = cmd.aligned(n, contents);
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
                            !line_suffix.is_empty(),
                            source,
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
                                // A gated state whose probe fits is inadmissible —
                                // see `admissible_group_state`.
                                let Some(state) =
                                    admissible_group_state(ctx, nodes, state, cmd.indent, commands)
                                else {
                                    continue;
                                };
                                let state_fits = arena_fits_with_lookahead(
                                    arena,
                                    state,
                                    Mode::Flat,
                                    commands,
                                    remaining,
                                    !line_suffix.is_empty(),
                                    source,
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
                        !line_suffix.is_empty(),
                        source,
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

            DocNode::IndentIfBreak { contents, group_id } => {
                let contents = *contents;
                let group_id = *group_id;
                cmd = process_indent_if_break(contents, group_id, policy.group_mode_map(), &cmd);
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
                    ctx,
                    parts,
                    output,
                    pos,
                    cmd.indent,
                    &DocContext::default(),
                    commands,
                    !line_suffix.is_empty(),
                    should_remeasure,
                );
            }

            DocNode::WithContext { doc, context } => {
                let inner_doc = *doc;

                if context.flow_break_probe() && policy.tracking_suffix() {
                    // Open a flow probe around this subtree: the sentinel pushed BELOW the
                    // inner doc pops after the whole subtree has rendered and records
                    // whether it emitted a newline — the answer the immediately following
                    // `hold_line_after_broken_flow` fill reads. Suffix-flush renders skip
                    // probes entirely (no flagged doc renders there, and state written
                    // out of order would go stale).
                    commands.push(cmd.with_doc(flow_probe_end));
                    arena.flow_probe_begin(output.len());
                }

                // A hold-flagged LINE — the leading boundary of the held inline-sibling wrap
                // (`DocArena::inline_sibling_line_group_held`) — consumes the immediately
                // preceding flow probe and renders as a forced break when the probed
                // predecessor broke; otherwise it descends as the ordinary collapsible line.
                // The group-shaped twin of the fill hook in `render_fill_iterative`: same
                // probe, same positional pairing (the wrap's command follows the sentinel
                // directly), and measurement never sees the flag — `arena_fits` descends
                // through `WithContext` — so the wrap's own fit decision is untouched.
                if context.hold_line_after_broken_flow()
                    && policy.tracking_suffix()
                    && let DocNode::Line(kind @ (LineKind::Normal | LineKind::Soft)) =
                        &nodes[inner_doc.index()]
                {
                    if arena.flow_probe_consume() {
                        render_line_node(
                            ctx,
                            *kind,
                            Mode::Break,
                            cmd.indent,
                            output,
                            pos,
                            policy.tracking_suffix(),
                            line_suffix,
                            should_remeasure,
                        );
                    } else {
                        cmd = cmd.with_doc(inner_doc);
                        continue;
                    }
                } else if policy.tracking_suffix() {
                    if let DocNode::Fill(fill_range) = &nodes[inner_doc.index()] {
                        let context = context.clone();
                        let parts = fill_range.resolve(children_vec);
                        render_fill_iterative(
                            ctx,
                            parts,
                            output,
                            pos,
                            cmd.indent,
                            &context,
                            policy.with_context_fill_rest(commands),
                            !line_suffix.is_empty(),
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
                // A boundary with nothing pending is a no-op; with a suffix pending it
                // is a HARD LINE — the flush must end the line, or the deferred `//`
                // runs to end of line and swallows the code the boundary exists to
                // protect (`const x: T[K] = // c` + `y;`, the initializer inside the
                // comment). See `render_line_node` for why it is that node exactly.
                if policy.tracking_suffix() && !line_suffix.is_empty() {
                    render_line_node(
                        ctx,
                        LineKind::Hard,
                        cmd.mode,
                        cmd.indent,
                        output,
                        pos,
                        policy.tracking_suffix(),
                        line_suffix,
                        should_remeasure,
                    );
                }
            }

            DocNode::BreakParent | DocNode::FlushBreak => {
                // No-op during rendering (both act only on fits decisions)
            }

            DocNode::FlowProbeEnd => {
                // Close the innermost flow probe: record whether the probed subtree —
                // whose commands all popped before this sentinel — emitted a newline.
                arena.flow_probe_finish(output);
            }

            DocNode::GatedState { contents, .. } => {
                // Transparent outside conditional-group state selection (the
                // states loop unwraps it before pushing, so this arm is a
                // defensive pass-through); the probe is measure-only.
                let contents = *contents;
                cmd = cmd.with_doc(contents);
                continue;
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
pub(super) fn render_single_doc(
    ctx: &RenderCtx<'_>,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    indent: RenderIndent,
    mode: Mode,
    should_remeasure: &mut bool,
) {
    let mut line_suffix: LineSuffixBuf = SmallVec::new();
    render_single_doc_inner(
        ctx,
        doc,
        output,
        pos,
        indent,
        mode,
        Some(&mut line_suffix),
        should_remeasure,
    );
    flush_line_suffix(ctx, &mut line_suffix, output, pos, should_remeasure, indent);
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
// Remaining args are the MUTABLE render state (`output`/`pos`/`should_remeasure`, plus the
// work buffers). Deliberately not bundled: a struct would take their address and sink them out
// of registers in the hot loop — see `RenderCtx`, which carries only the shared context.
#[expect(clippy::too_many_arguments)]
pub(super) fn render_single_doc_inner(
    ctx: &RenderCtx<'_>,
    doc: DocId,
    output: &mut String,
    pos: &mut usize,
    indent: RenderIndent,
    mode: Mode,
    suffix_buffer: Option<&mut LineSuffixBuf>,
    should_remeasure: &mut bool,
) {
    let mut policy = SingleDocPolicy {
        tracking_suffix: suffix_buffer.is_some(),
        #[cfg(feature = "swallow_check")]
        swallow: SwallowTracker::join_render(),
    };
    let mut dummy_suffix: LineSuffixBuf = SmallVec::new();
    let line_suffix = suffix_buffer.unwrap_or(&mut dummy_suffix);

    // Sub-renders keep a local inline stack (measured allocation-free — the
    // common single-doc render never spills) rather than borrowing the pooled
    // one, which the enclosing top-level render already holds.
    let mut commands: CmdStack = SmallVec::new();
    render_doc_core(
        ctx,
        doc,
        output,
        pos,
        indent,
        mode,
        &mut policy,
        &mut commands,
        line_suffix,
        should_remeasure,
    );
}

//
// Utilities
//

/// A run of the production indent, long enough that every indent a real document
/// reaches is one slice of it (the deepest over four app corpora is 14 levels).
/// Deeper runs chunk through it rather than falling back to a per-level push.
const INDENT_RUN: &str = "\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t";

pub(super) fn write_indentation(
    output: &mut String,
    indent: RenderIndent,
    render: &RenderConfig,
    embed: &EmbedContext,
) {
    let extra = if embed.first_line_offset > 0 {
        embed.base_indent_offset
    } else {
        0
    };
    let levels = indent.tabs() + extra;
    // One append per line, not one per level. `String::push_str` lowers a
    // runtime-length copy to an indirect `memcpy@plt`, so pushing the one-byte
    // indent once per level paid a call per byte — 246 K calls to move 246 KB
    // over a fuz_app pass, where 95% of lines indent four levels or fewer. A run
    // of the production indent is one slice of [`INDENT_RUN`], and the short arms
    // hand LLVM a constant length so those lines store the tabs inline with no
    // call at all (the `specialize_short_len!` mechanism, at a site whose source
    // is a constant too). The general arm stays for the `RenderConfig` seam's
    // other indent strings, which only the doc-builder unit tests spell.
    if render.indent.as_bytes() == b"\t" {
        match levels {
            0 => {}
            1 => output.push('\t'),
            2 => output.push_str("\t\t"),
            3 => output.push_str("\t\t\t"),
            4 => output.push_str("\t\t\t\t"),
            5 => output.push_str("\t\t\t\t\t"),
            6 => output.push_str("\t\t\t\t\t\t"),
            7 => output.push_str("\t\t\t\t\t\t\t"),
            8 => output.push_str("\t\t\t\t\t\t\t\t"),
            mut left => {
                while left > INDENT_RUN.len() {
                    output.push_str(INDENT_RUN);
                    left -= INDENT_RUN.len();
                }
                output.push_str(&INDENT_RUN[..left]);
            }
        }
    } else {
        for _ in 0..levels {
            output.push_str(render.indent);
        }
    }
    // Sub-tab alignment is literal spaces, so a closing delimiter stays under
    // its opener at any tab width (Prettier's `align` under `useTabs`).
    for _ in 0..indent.trailing_align_spaces() {
        output.push(' ');
    }
}

fn indent_width(level: usize, render: &RenderConfig) -> usize {
    level * indent_str_width(render.indent)
}

pub(super) fn line_start_column(
    indent: RenderIndent,
    render: &RenderConfig,
    embed: &EmbedContext,
) -> usize {
    indent_width(indent.tabs(), render)
        + indent.trailing_align_spaces()
        + embed.base_indent_offset * TAB_WIDTH
}

fn indent_str_width(indent: &str) -> usize {
    indent
        .chars()
        .map(|ch| if ch == '\t' { TAB_WIDTH } else { 1 })
        .sum()
}

#[cfg(test)]
mod column_arithmetic_tests {
    //! Equivalence/contract tests for this module's corpus-blind numeric facts —
    //! the column-advance and indent-width helpers that feed every `fits` verdict.
    //!
    //! **No corpus can grade these.** A column error only changes the *output*
    //! once a fits verdict lands exactly on the print width, so an arithmetic slip
    //! on a rare byte (a tab, a control char) or a rare position leaves every
    //! formatted file byte-identical — it sails through the fixtures and any size
    //! of format/wire diff (verified for the sibling `pooled_text_width`: a
    //! one-column tab error was invisible to an 11,696-file diff and caught only by
    //! its equivalence test). These are the only gates with power over their facts.
    //! Mutation testing (`cargo mutants -p tsv_lang --file '**/arena_render.rs'`)
    //! flagged each arm below as an unasserted survivor; corruption-verify any
    //! change here by breaking the arm and watching exactly one assertion fail.
    use super::RenderConfig;
    #[cfg(feature = "comment_check")]
    use super::RenderPurpose;
    use super::{
        INDENT_RUN, RenderIndent, effective_suffix_width, indent_str_width, indent_width,
        line_start_column, update_pos_for_text, write_indentation,
    };
    use crate::EmbedContext;
    use crate::config::TAB_WIDTH;
    use crate::printing::visual_width;

    // --- update_pos_for_text / update_pos_for_text_unicode column advance ---

    /// Run `update_pos_for_text` (the ASCII fast path, which delegates the whole
    /// slice to `update_pos_for_text_unicode` on the first non-ASCII byte) and
    /// return the resulting column.
    fn advanced(pos: usize, s: &str) -> usize {
        let mut p = pos;
        update_pos_for_text(&mut p, s);
        p
    }

    /// The column after rendering `s` starting at column `pos`, spelled out
    /// independently of the fast path: a newline restarts the column at the width
    /// of the text after the last one; otherwise the width simply adds. This is
    /// the shape `update_pos_for_text_unicode` implements, kept as a *separate*
    /// copy here so a mutation to the source arithmetic desyncs the two and fires.
    fn reference(pos: usize, s: &str) -> usize {
        match s.rfind('\n') {
            Some(nl) => visual_width(&s[nl + 1..], TAB_WIDTH),
            None => pos + visual_width(s, TAB_WIDTH),
        }
    }

    fn assert_advance_agrees(pos: usize, s: &str) {
        assert_eq!(
            advanced(pos, s),
            reference(pos, s),
            "update_pos_for_text disagrees with the reference at pos {pos} on {s:?}"
        );
    }

    #[test]
    fn advance_agrees_on_exhaustive_short_strings() {
        // Every string of length 0-2 over an alphabet spanning each arm of the
        // byte walk (newline reset, tab expansion, plain ASCII, control/DEL — all
        // 0x00..=0x7f) and the non-ASCII hand-off (2-/3-/4-byte, combining mark,
        // ZWJ), at several starting columns so the `pos + w` accumulation and the
        // newline-reset-to-0 are both graded.
        let alphabet = [
            "a", "Z", "0", " ", "\t", "\n", "\x01", "\x7f", "é", "中", "🎉", "\u{0301}", "\u{200d}",
        ];
        for pos in [0usize, 1, 7, 42] {
            assert_advance_agrees(pos, "");
            for a in alphabet {
                assert_advance_agrees(pos, a);
                for b in alphabet {
                    assert_advance_agrees(pos, &format!("{a}{b}"));
                }
            }
        }
    }

    #[test]
    fn advance_agrees_on_realistic_and_boundary_inputs() {
        // Cases that pin specific arms: tab expansion, the newline reset (both
        // pure-ASCII, handled by the fast path, and non-ASCII, routed whole to
        // `_unicode` so its `rfind('\n') + 1` slice + tail measure are graded),
        // and a combining cluster crossing an ASCII boundary.
        for pos in [0usize, 5] {
            for s in [
                "identifier",
                "a\tb\tc",
                "line one\ntail",
                "\tindented\ttail",
                // Non-ASCII → the whole slice goes through `_unicode`; a newline
                // after the multibyte char grades its restart slice.
                "é\ntail",
                "中\tafter",
                "prefix\n中tail",
                "e\u{0301}x",
                "1\u{fe0f}\u{20e3}",
            ] {
                assert_advance_agrees(pos, s);
            }
        }
    }

    // --- indent column math: indent_str_width / indent_width / line_start_column ---

    fn cfg(indent: &'static str) -> RenderConfig {
        RenderConfig {
            print_width: 100,
            indent,
            #[cfg(feature = "comment_check")]
            purpose: RenderPurpose::Output,
        }
    }

    #[test]
    fn indent_str_width_counts_tabs_as_tab_width() {
        // Each '\t' is TAB_WIDTH columns, every other char is 1.
        assert_eq!(indent_str_width(""), 0);
        assert_eq!(indent_str_width("\t"), TAB_WIDTH);
        assert_eq!(indent_str_width("\t\t"), 2 * TAB_WIDTH);
        assert_eq!(indent_str_width("  "), 2);
        assert_eq!(indent_str_width("    "), 4);
        // Mixed: the tab/non-tab split must not collapse to a constant.
        assert_eq!(indent_str_width(" \t "), 1 + TAB_WIDTH + 1);
    }

    #[test]
    fn indent_width_is_level_times_indent_str_width() {
        let tab = cfg("\t");
        let spaces = cfg("  ");
        assert_eq!(indent_width(0, &tab), 0);
        assert_eq!(indent_width(3, &tab), 3 * TAB_WIDTH);
        assert_eq!(indent_width(4, &spaces), 4 * 2);
    }

    #[test]
    fn line_start_column_adds_indent_and_embed_offset() {
        let tab = cfg("\t");
        // base_indent_offset 0: purely the indent width.
        let embed0 = EmbedContext::default();
        assert_eq!(line_start_column(RenderIndent::level(0), &tab, &embed0), 0);
        assert_eq!(
            line_start_column(RenderIndent::level(2), &tab, &embed0),
            2 * TAB_WIDTH
        );
        // base_indent_offset > 0 contributes base * TAB_WIDTH, ADDED (not
        // multiplied) to the indent width. Level 0 isolates the additive term
        // (a `+`→`*` flip reads 0 here instead of the offset); level 2 grades the
        // sum of two non-zero terms.
        let embed = EmbedContext {
            base_indent_offset: 3,
            ..EmbedContext::default()
        };
        assert_eq!(
            line_start_column(RenderIndent::level(0), &tab, &embed),
            3 * TAB_WIDTH
        );
        assert_eq!(
            line_start_column(RenderIndent::level(2), &tab, &embed),
            2 * TAB_WIDTH + 3 * TAB_WIDTH
        );
        // A sub-tab align adds literal spaces on top of the whole-tab column,
        // independent of TAB_WIDTH (the tab-width-agnostic alignment property).
        assert_eq!(
            line_start_column(RenderIndent::level(2).aligned(2), &tab, &embed0),
            2 * TAB_WIDTH + 2
        );
    }

    // --- effective_suffix_width boundary ---

    #[test]
    fn effective_suffix_width_gates_on_first_line_offset() {
        let embed = EmbedContext {
            first_line_offset: 5,
            suffix_width: 3,
            ..EmbedContext::default()
        };
        // pos >= first_line_offset → the reserved suffix; the boundary is
        // inclusive (pos == offset already reserves).
        assert_eq!(effective_suffix_width(5, &embed), 3);
        assert_eq!(effective_suffix_width(6, &embed), 3);
        // pos < first_line_offset → nothing reserved yet.
        assert_eq!(effective_suffix_width(4, &embed), 0);
        assert_eq!(effective_suffix_width(0, &embed), 0);
    }

    // --- write_indentation: the emitted whitespace itself ---

    /// The indentation a line starts with, spelled out independently of
    /// [`write_indentation`]'s specialized arms: the indent string once per
    /// level, then the sub-tab alignment as literal spaces.
    fn indent_reference(
        indent: RenderIndent,
        render: &RenderConfig,
        embed: &EmbedContext,
    ) -> String {
        let extra = if embed.first_line_offset > 0 {
            embed.base_indent_offset
        } else {
            0
        };
        let mut s = render.indent.repeat(indent.tabs() + extra);
        for _ in 0..indent.trailing_align_spaces() {
            s.push(' ');
        }
        s
    }

    fn assert_indent_agrees(indent: RenderIndent, render: &RenderConfig, embed: &EmbedContext) {
        let mut out = String::new();
        write_indentation(&mut out, indent, render, embed);
        assert_eq!(
            out,
            indent_reference(indent, render, embed),
            "write_indentation disagrees with the reference at {} levels (indent {:?}, align {})",
            indent.tabs(),
            render.indent,
            indent.trailing_align_spaces()
        );
    }

    #[test]
    fn write_indentation_agrees_with_the_reference_at_every_depth() {
        // The emitter takes a run of the production indent from one static
        // string, with the first few depths specialized to constant-length
        // stores — so the arms, the slice past them, and the chunking loop that
        // covers a depth deeper than the run are three separate code paths that
        // no corpus reaches past ~14 levels (the deepest real indent measured),
        // while the parser accepts nests thousands deep. Every depth up to two
        // full runs is graded here, which is the only place either boundary is.
        for render in [cfg("\t"), cfg("  "), cfg("")] {
            for level in 0..=(2 * INDENT_RUN.len() + 3) {
                assert_indent_agrees(
                    RenderIndent::level(level),
                    &render,
                    &EmbedContext::default(),
                );
            }
        }
    }

    #[test]
    fn write_indentation_covers_alignment_and_the_embed_offset() {
        let tab = cfg("\t");
        let embed0 = EmbedContext::default();
        // Sub-tab alignment follows the whole tabs, at every specialization arm
        // and past them.
        for level in [0usize, 1, 4, 8, 9, INDENT_RUN.len(), INDENT_RUN.len() + 1] {
            for align in [0u32, 1, 3] {
                assert_indent_agrees(RenderIndent::level(level).aligned(align), &tab, &embed0);
            }
        }
        // `base_indent_offset` adds levels, but only when `first_line_offset` is
        // set — the gate that decides whether an embedded context indents at all.
        let gated = EmbedContext {
            base_indent_offset: 5,
            first_line_offset: 0,
            ..EmbedContext::default()
        };
        let live = EmbedContext {
            base_indent_offset: 5,
            first_line_offset: 7,
            ..EmbedContext::default()
        };
        for level in [0usize, 2, 30] {
            assert_indent_agrees(RenderIndent::level(level), &tab, &gated);
            assert_indent_agrees(RenderIndent::level(level), &tab, &live);
        }
        let mut out = String::new();
        write_indentation(&mut out, RenderIndent::level(2), &tab, &live);
        assert_eq!(
            out,
            "\t".repeat(7),
            "the embed offset must add levels, not replace them"
        );
    }
}

#[cfg(test)]
mod render_base_contract_tests {
    //! The `*_into` entry points' "(empty) buffer" is a **requirement**, and this
    //! is the executable form of it — the reason
    //! [`crate::OutputBuffer::as_empty_render_target`] hands back `None` rather
    //! than trusting its caller.
    //!
    //! **No corpus can grade this**, in either direction. Today's only in-place
    //! caller (`tsv_ts`'s whole-program render) always arrives on a fresh printer,
    //! so removing the refusal changes no output at all and every fixture, format
    //! diff and wire diff stays green; the bug it prevents arrives with the *next*
    //! caller, and arrives as text silently eaten from in front of the doc.
    use super::{
        arena_print_doc_with_indent_resolved_into,
        arena_print_doc_with_indent_resolved_preserve_whitespace_into,
    };
    use crate::EmbedContext;
    use crate::doc::arena::DocArena;

    /// A doc whose first command is a hard line break — the shape that reaches the
    /// trim before it has written anything of its own.
    fn break_first_doc(d: &DocArena) -> crate::doc::arena::DocId {
        let parts = [d.hardline(), d.text("b")];
        d.concat(&parts)
    }

    #[test]
    fn a_non_empty_base_lets_the_line_break_trim_reach_backwards() {
        let d = DocArena::new();
        let doc = break_first_doc(&d);
        let embed = EmbedContext::default();

        // The contract-honouring shape: render into an empty buffer, then append.
        let mut scratch = String::new();
        arena_print_doc_with_indent_resolved_into(&d, doc, &embed, 0, 0, "", &mut scratch);
        let mut appended = String::from("x\t");
        appended.push_str(&scratch);

        // The same doc rendered into a buffer that already holds `"x\t"`.
        let mut in_place = String::from("x\t");
        arena_print_doc_with_indent_resolved_into(&d, doc, &embed, 0, 0, "", &mut in_place);

        assert_eq!(appended, "x\t\nb");
        assert_eq!(
            in_place, "x\nb",
            "`trim_trailing_whitespace` runs before the doc's first newline and \
             has no floor, so it eats the tab the doc never wrote"
        );
        assert_ne!(
            appended, in_place,
            "if these ever agree, the trims have grown a floor and \
             `as_empty_render_target`'s refusal can be widened"
        );
    }

    #[test]
    fn a_non_empty_base_also_loses_bytes_through_the_final_line_trim() {
        // The preserve-whitespace entry point skips only the FINAL-line trim, so
        // the interior trim above still bites there; the default entry point adds
        // the final-line trim, which reaches back over an empty render outright.
        let d = DocArena::new();
        let empty_doc = d.text("");
        let embed = EmbedContext::default();

        let mut in_place = String::from("x\t");
        arena_print_doc_with_indent_resolved_into(&d, empty_doc, &embed, 0, 0, "", &mut in_place);
        assert_eq!(
            in_place, "x",
            "a doc that writes nothing still truncates a non-empty base"
        );

        let mut preserved = String::from("x\t");
        arena_print_doc_with_indent_resolved_preserve_whitespace_into(
            &d,
            empty_doc,
            &embed,
            0,
            0,
            "",
            &mut preserved,
        );
        assert_eq!(
            preserved, "x\t",
            "the preserve-whitespace entry point skips the final-line trim, so this \
             one base survives — the interior trim is what the other test pins"
        );
    }

    #[test]
    fn an_empty_base_is_what_makes_rendering_in_place_byte_identical() {
        // The positive half: with nothing in front of the doc's first byte, both
        // shapes agree, which is the whole licence `write_arena_doc` takes.
        let d = DocArena::new();
        let embed = EmbedContext::default();
        for doc in [break_first_doc(&d), d.text(""), d.text("b")] {
            let mut scratch = String::new();
            arena_print_doc_with_indent_resolved_into(&d, doc, &embed, 0, 0, "", &mut scratch);
            let mut in_place = String::new();
            arena_print_doc_with_indent_resolved_into(&d, doc, &embed, 0, 0, "", &mut in_place);
            assert_eq!(scratch, in_place);
        }
    }
}
