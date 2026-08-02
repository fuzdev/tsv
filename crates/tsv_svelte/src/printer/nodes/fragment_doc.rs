// Doc-based formatting for inline fragment content
//
// Builds Doc IR trees for fragment nodes, enabling proper fits() checks
// that account for siblings. This matches Prettier's architecture where
// the entire inline content is represented as a single doc tree.
//
// Entered through the `build_nodes_doc_*` family (and the element/block/root doc
// builders that call them) to format fragment content with correct attribute
// wrapping decisions that consider what comes after each element.

// Allow Svelte block syntax like `{:else}`, `{:then}`, `{:catch}` which
// look like Rust format args but are valid Svelte template syntax.
#![allow(clippy::literal_string_with_formatting_args)]

use super::element_doc::MultilineCause;
use super::helpers::{is_control_flow_block, is_inline_content};
use crate::ast::internal::{
    self, Fragment, FragmentNode, is_collapsible_ws, is_collapsible_ws_char, split_collapsible_ws,
};
use crate::printer::Printer;
use smallvec::SmallVec;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::is_format_ignore_directive;

/// Position of a text node relative to its siblings.
///
/// Encodes both position (first/last/middle/only) and whether adjacent
/// siblings are inline content, which affects whitespace handling.
enum SiblingPosition {
    /// Only child (first AND last) - no siblings
    Only,
    /// First child with info about next sibling
    First { next_is_inline: bool },
    /// Last child with info about previous sibling
    Last { prev_is_inline: bool },
    /// Middle child with info about both neighbors
    Middle {
        prev_is_inline: bool,
        next_is_inline: bool,
    },
}

impl SiblingPosition {
    fn new(is_first: bool, is_last: bool, prev_is_inline: bool, next_is_inline: bool) -> Self {
        match (is_first, is_last) {
            (true, true) => Self::Only,
            (true, false) => Self::First { next_is_inline },
            (false, true) => Self::Last { prev_is_inline },
            (false, false) => Self::Middle {
                prev_is_inline,
                next_is_inline,
            },
        }
    }

    fn prev_is_inline(&self) -> bool {
        match self {
            Self::Last { prev_is_inline } | Self::Middle { prev_is_inline, .. } => *prev_is_inline,
            _ => false,
        }
    }

    fn next_is_inline(&self) -> bool {
        match self {
            Self::First { next_is_inline } | Self::Middle { next_is_inline, .. } => *next_is_inline,
            _ => false,
        }
    }
}

/// The per-call layout facts `handle_text_child` cannot derive from `trimmed_nodes[i]` alone —
/// each is decided by the *caller's* context rather than by the text node's own position.
#[derive(Clone, Copy)]
struct TextChildContext {
    /// A break-capable expression tag is present in the fragment, so boundary text adjacent to a
    /// tag emits a plain space instead of a fill `line` (which would short-circuit an earlier
    /// expression group's `fits()` lookahead).
    breakable_exprs: bool,
    /// Whether the fragment is built on the convergence path (the multiline element arm, the only
    /// caller that routes blocks and control-flow blocks through their own dispatch) — and, when
    /// it is, *why* the layout went multiline. The cause is read by the sibling-newline flow rule
    /// alone; every other site asks only [`MultilineCause::is_multiline`].
    cause: MultilineCause,
    /// Whether this node's inline run holds prose — one of the two gates on the sibling-newline
    /// flow rule at its standalone-separator site (`cause` is the other). Computed once per run by
    /// the caller ([`Printer::scan_inline_run`]) and only on the `multiline` path; `false`
    /// everywhere else, which is inert because that site returns before reading it in the
    /// non-multiline arm. Note "inert per arm" is NOT "the arms agree" — the inline arm reaches
    /// the same separator through its own `next_is_tag` case, and the two emitting a different doc
    /// for one logical separator is exactly the period-2 cycle `cause` exists to close.
    run_has_prose: bool,
    /// The first and last index in `trimmed_nodes` that the whitespace rules see — the fragment's
    /// content bounds once every HOISTED node is skipped
    /// ([`FragmentNode::content_bounds`]). `handle_text_child`'s `is_first` / `is_last` are
    /// `i <= .0` / `i >= .1` rather than `i == 0` / `i + 1 == len`, so a text with only hoisted
    /// nodes between it and the edge trims its run — the compiler deletes that run, since it lifts
    /// those nodes out before it trims.
    ///
    /// Carried on the context rather than recomputed per child: the question is per-FRAGMENT, and
    /// asking it per node would rescan the sibling list at every text (O(n²) on a long fragment).
    content_bounds: (usize, usize),
    /// A byte-glued HTML-comment run immediately preceding this text
    /// ([`Printer::glued_comment_run_text`]), already built as one doc by the caller and **not**
    /// pushed as a sibling — this handler fuses it into the fill's first item instead, so the unit
    /// is unbreakable by construction. `None` for every other text child. See the `glued_lead`
    /// comment in [`Printer::handle_text_child`] for why a comment prefix is fused where every
    /// other glued predecessor is flagged.
    ///
    /// Carries the run's **head index** beside the doc because fusing moves the unit's leading
    /// boundary: the break point in front of the unit is the one in front of the *comment*, not the
    /// one in front of the text, and only the head index can name it.
    glued_prefix: Option<(DocId, usize)>,
    /// Index of the node the **previously pushed sibling doc** begins at — its unit's head, which
    /// is not `i - 1` whenever that doc is a consume-ahead unit (a glued element run, a
    /// comment-prefixed element). Only the after-element fold reads it, and only to ask whether the
    /// element it folds is byte-glued to what precedes it: the fold's leading boundary is the one in
    /// front of the *unit*, so a run's tail index would answer about an interior boundary that is
    /// glued by construction. Tracked by the caller's loop, which visits each unit exactly once.
    ///
    /// ⚠️ Precisely: the previously **visited** unit's head — a visit that pushes no doc (a
    /// whitespace-only text) claims it too. The fold reader is safe because it only fires when the
    /// previous visit built the element doc it pops; a new reader must not assume a pushed doc.
    prev_sibling_head: usize,
}

/// The treatment of an inline child doc's LEADING boundary, decided at the unit's head — the
/// argument to [`Printer::push_inline_child_doc`]. Three mutually exclusive cases, and the
/// exclusivity is structural: a previous text that trimmed a boundary space cannot also be glued
/// ([`Printer::text_glued_after`] fails on a whitespace tail), so no caller ever holds two at once.
#[derive(Clone, Copy)]
enum LeadBoundary {
    /// The previous text trimmed a space-only boundary and deferred the separator to this sibling
    /// (prettier's `handleWhitespaceOfPrevTextNode`): lead with a collapsible `line` inside a
    /// group — a space when the fill fits, a break when it wraps.
    Spaced,
    /// Byte-glued to the sibling before it: there is no boundary space to honor, and the doc is
    /// instead **marked** as the continuation of a welded run (`glued_lead` + `glued_atom`). The
    /// mark is inert at render — a `DocContext` is consumed only by a `Fill`, and this wraps an
    /// element — and exists solely so a preceding text run's `break_before_wide_flow` look-ahead
    /// can walk *through* this element to the rest of the run (`DocArena::welded_entry`). Without
    /// it the walk stops at the last glued TEXT node and the run stands on the line, tearing a
    /// later element open instead of travelling to the boundary whole.
    Glued,
    /// An ordinary boundary already carried by the surrounding docs: push bare.
    Plain,
}

/// Whether `raw` begins with a linebreak, ignoring leading horizontal whitespace — prettier's
/// `startsWithLinebreak` (`^([\t\f\r ]*\n)`) with the form feed dropped, since a form feed is
/// content rather than skippable whitespace ([`is_collapsible_ws`]). Used by the block-child
/// boundary logic to tell a leading-linebreak text (which supplies its own break) from
/// content/space text (which needs a `softline`).
///
/// The array spelling is deliberate: it feeds a `str` pattern, where an `is_collapsible_ws_char`
/// predicate fn would change the `Pattern` monomorphization (a measured `.text` growth).
fn text_starts_with_linebreak(raw: &str) -> bool {
    raw.trim_start_matches([' ', '\t', '\r']).starts_with('\n')
}

impl<'a> Printer<'a> {
    /// Build a doc for an entire fragment (sequence of nodes)
    ///
    /// This is the entry point for doc-based inline content formatting.
    /// The resulting doc includes all nodes, so fits() checks will
    /// naturally account for siblings.
    pub(super) fn build_fragment_doc(&self, fragment: &Fragment<'_>) -> DocId {
        self.build_nodes_doc(fragment.nodes)
    }

    /// Build a doc for a slice of fragment nodes
    ///
    /// Accepts a slice directly, avoiding Fragment allocation when caller
    /// already has a `&[FragmentNode]`.
    pub(crate) fn build_nodes_doc(&self, nodes: &[FragmentNode<'_>]) -> DocId {
        let mut docs: DocBuf = DocBuf::new();
        let mut format_ignore_next = false;
        // Running flag for the control-flow `has_preceding_breakable` test below. `is_inline_content`
        // is monotone over the prefix, so OR-in the prior node once per iteration instead of
        // re-scanning `nodes[..i]` at each control-flow node (O(N²) over the sibling list). Reading
        // `nodes[i - 1]` at the top keeps the flag equal to `nodes[..i]` through the `continue`s below
        // (a format-ignored inline element must still count for a later block).
        let mut has_preceding_breakable = false;
        for (i, node) in nodes.iter().enumerate() {
            if i > 0 && is_inline_content(&nodes[i - 1]) {
                has_preceding_breakable = true;
            }
            // format-ignore: skip whitespace, emit raw source for ignored node
            if format_ignore_next {
                if let Some(raw_doc) = self.format_ignore_raw_doc(node) {
                    docs.push(raw_doc);
                    format_ignore_next = false;
                }
                continue;
            }
            if Self::is_format_ignore_comment(node, self.source) {
                if let Some(doc) = self.build_fragment_node_doc(node) {
                    docs.push(doc);
                }
                format_ignore_next = true;
                continue;
            }

            // For control flow blocks, check if there's preceding breakable content
            let is_control_flow = is_control_flow_block(node);
            let doc = if is_control_flow {
                // "Breakable preceding content" is exactly the inline-content set — text never
                // breaks before a control-flow block, so reuse the one predicate (tracked as the
                // running flag above rather than re-scanned here).
                self.build_fragment_node_doc_with_preceding_context(node, has_preceding_breakable)
            } else {
                self.build_fragment_node_doc(node)
            };
            if let Some(doc) = doc {
                docs.push(doc);
            }
        }

        // `concat` short-circuits the empty case to `empty()`.
        self.d().concat(&docs)
    }

    /// Find the inclusive-exclusive index range of `nodes` after trimming boundary nodes for
    /// which `skip` returns true. Returns `None` when every node is skipped (the range is empty),
    /// so callers can short-circuit to an empty doc.
    fn trimmed_node_bounds(
        nodes: &[FragmentNode<'_>],
        skip: impl Fn(&FragmentNode<'_>) -> bool,
    ) -> Option<(usize, usize)> {
        let start = nodes.iter().position(|n| !skip(n))?;
        let end = nodes.iter().rposition(|n| !skip(n)).map_or(0, |i| i + 1);
        Some((start, end))
    }

    /// Build a doc for a node slice with boundary whitespace trimmed
    ///
    /// Matches prettier-plugin-svelte's printChildren behavior:
    /// - Skip whitespace-only text at start and end
    /// - Each text node gets its own fill (for word-level breaking)
    /// - Whitespace between text and inline elements is handled via group([line, ...])
    /// - This allows fills to operate independently while still coordinating breaks
    ///
    /// The key insight from prettier-plugin-svelte:
    /// - Text ending with whitespace before inline element: trim ws, set flag
    /// - Inline element with flag: wrap as group([line, element])
    /// - Text starting with whitespace after inline element: trim ws, wrap prev element with line after
    ///
    /// Boundary whitespace is always trimmed — whitespace-only text at the fragment edges is
    /// skipped and the first/last text node's edge run is stripped. It is render-free under
    /// Svelte 5 (`clean_nodes` trims every fragment edge at compile), so no element kind keeps
    /// it — see conformance_prettier.md §Svelte: Inline content block-style.
    ///
    /// # Parameters
    /// - `breakable_exprs`: If true, boundary text adjacent to expression/html/render tags is
    ///   emitted as plain spaces instead of `fill` `line`s. Set when the fragment has a
    ///   break-capable expression tag (the hard-width divergence): a `line` in
    ///   fits()-Break mode short-circuits a preceding expression group's width check, stranding
    ///   it flat and overshooting printWidth (`fill_multiple_expr_long`). Plain spaces keep the
    ///   expression group's full `fits()` obligation so it breaks instead. Callers with no
    ///   break-capable expression pass `false`.
    /// - `cause`: the convergence mode — [`MultilineCause::None`] is the legacy inline arm;
    ///   anything else is the element multiline arm (`compute_multiline_cause`). Multiline turns on
    ///   the ported prettier-plugin-svelte printChildren handling that the legacy inline callers
    ///   don't need (and would be churned by): block children via `handle_block_child` +
    ///   `forceBreakContent`; `printWhitespace` (a whitespace-only text at a non-HTML-element
    ///   boundary becomes a hardline/blank/bare-line); the `splitTextToDocs` leading-linebreak rule
    ///   (content text with a leading newline emits a hardline rather than folding into the prev
    ///   element); and the first/last whitespace-only boundary deferring to the parent's
    ///   leading/trailing break (emit nothing) instead of the inline single space. The legacy
    ///   callers pass `None` and stay byte-identical. (Path 1, `build_nodes_doc_multiline`, still
    ///   serves block bodies / root / special elements — its reroute onto this path + deletion is
    ///   the remaining Slice-2/3 work.) The `Structural` / `SourceBreaks` split is read only by the
    ///   sibling-newline flow rule in [`Self::handle_text_child`].
    pub(super) fn build_nodes_doc_trimmed(
        &self,
        nodes: &[FragmentNode<'_>],
        breakable_exprs: bool,
        cause: MultilineCause,
    ) -> DocId {
        let multiline = cause.is_multiline();
        let d = self.d();
        if nodes.is_empty() {
            return d.empty();
        }

        // Skip whitespace-only text nodes at the fragment boundaries (collapsible whitespace
        // only — a non-breaking space (U+00A0) or a form feed is content, not a collapsible
        // boundary, so a node made only of those is never skipped).
        let source = self.source;
        let Some((start_idx, end_idx)) =
            Self::trimmed_node_bounds(nodes, |n: &FragmentNode<'_>| n.is_whitespace_only_text())
        else {
            return d.empty();
        };

        let trimmed_nodes = &nodes[start_idx..end_idx];
        let trimmed_len = trimmed_nodes.len();

        // Build docs matching prettier-plugin-svelte's structure:
        // - Each text node → fill([word, line, word, ...])
        // - Inline elements → wrapped with group([line, element]) or group([element, line])
        //   depending on surrounding whitespace
        let mut child_docs = d.pooled_docbuf();
        let mut handle_whitespace_of_prev_text = false;

        // forceBreakContent (prettier-plugin-svelte): a fragment that mixes a block element
        // with more than one child breaks, so each block lands on its own line. tsv hardens the
        // block-child boundaries (hardline) rather than pushing a `break_parent` sibling, which
        // would poison a preceding group's `fits()` lookahead. See `handle_block_child`. Only
        // the `multiline` convergence arm routes blocks here, so the scan is gated on it.
        // A declaration tag on its own line is the same kind of child as a block element here —
        // both own a line, so both harden every block boundary in the fragment.
        let force_break = multiline
            && trimmed_len > 1
            && (0..trimmed_len).any(|j| self.owns_own_line(trimmed_nodes, j));

        // The fragment's content bounds once every HOISTED node is skipped — the positions the
        // compiler's whitespace rules actually measure the edges from (see
        // `TextChildContext::content_bounds`). An all-hoisted fragment has no text child to ask,
        // so the fallback is inert.
        let content_bounds =
            FragmentNode::content_bounds(trimmed_nodes).unwrap_or((0, trimmed_len - 1));

        let mut format_ignore_next = false;
        // Exclusive upper bound of indices already consumed by a maximal glued-element run built
        // at its head (`build_glued_element_run`): the run is built ONCE at its first element and
        // its tail elements are skipped, so the build is O(run length), not the O(run length²) a
        // rebuild-at-each-element would cost on a long glued run (generated per-token `<span>`s).
        let mut glued_run_consumed_until = 0usize;
        // Running `has_preceding_breakable` flag (see `build_nodes_doc`): OR-in the
        // prior node once per iteration rather than re-scanning `trimmed_nodes[..i]` at each of the
        // two use sites below. Reading `trimmed_nodes[i - 1]` at the top keeps the flag equal to
        // `trimmed_nodes[..i]` through the `continue`s (format-ignore, whitespace-run collapse,
        // glued-run skip).
        let mut has_preceding_breakable = false;
        // A glued HTML-comment run that prefixes the NEXT text child, built at the run's head and
        // carried forward instead of pushed — `handle_text_child` fuses it into the text's fill so
        // the unbreakable boundary is expressed structurally (see its `glued_lead` comment). Held
        // across exactly one iteration: the run's own indices are skipped via
        // `glued_run_consumed_until`, so the very next node visited is the text that takes it.
        let mut pending_glued_prefix: Option<(DocId, usize)> = None;
        // Index of the node the most recently VISITED iteration began its unit at — usually the
        // head of the previously pushed sibling doc, but a visit that pushes nothing (a
        // whitespace-only text) claims it too. Handed to the next visited node as
        // `prev_sibling_head`.
        let mut sibling_head = 0usize;
        // Inline-run prose cursor for the sibling-newline flow rule at its standalone-separator
        // site (`handle_text_child`'s whitespace-only arm). Runs partition `trimmed_nodes`, so
        // advancing the cursor here — at the TOP of the body, ahead of every `continue` — visits
        // each node once and keeps the total cost O(n). Reading it at the separator instead would
        // rescan per separator, and a mid-run `continue` (a glued run skips its tail) would leave
        // a later rescan blind to the prose *before* it in the same run.
        let (mut run_end, mut run_has_prose) = (0usize, false);
        for (i, node) in trimmed_nodes.iter().enumerate() {
            if multiline && i >= run_end {
                (run_end, run_has_prose) = self.scan_inline_run(trimmed_nodes, i);
            }
            if i > 0 && is_inline_content(&trimmed_nodes[i - 1]) {
                has_preceding_breakable = true;
            }
            // Tail of a glued element run already built at its head — skip (its doc is in place).
            if i < glued_run_consumed_until {
                continue;
            }
            // Hand the PREVIOUS visited index forward and claim this one. Every sibling doc is
            // built at its unit's HEAD (a glued element run and a comment-prefixed element are both
            // consume-ahead, their tails skipped by the `continue` above), so the previous visited
            // index names where the previously pushed doc BEGINS — which is the only thing that can
            // answer whether that doc's leading boundary is glued. Reading `i - 1` instead would
            // name a run's tail element, whose own leading boundary is glued by construction and so
            // says nothing about the unit's. See `handle_text_child`'s after-element fold.
            let prev_sibling_head = std::mem::replace(&mut sibling_head, i);
            // format-ignore: skip whitespace, emit raw source for ignored node
            if format_ignore_next {
                if let Some(raw_doc) = self.format_ignore_raw_doc(node) {
                    // The directive comment is the previous child and the whitespace between it
                    // and this node was skipped above; in `multiline` mode that boundary must keep
                    // its line break (path 1 flushed the buffer here) so the ignored node starts on
                    // its own line (`<!-- prettier-ignore -->⏎<div …>`) rather than hugging the
                    // directive. A first node (no preceding sibling) defers to the parent boundary.
                    if multiline && !child_docs.is_empty() {
                        child_docs.push(self.d().hardline());
                    }
                    child_docs.push(raw_doc);
                    handle_whitespace_of_prev_text = false;
                    format_ignore_next = false;
                }
                continue;
            }
            if Self::is_format_ignore_comment(node, source) {
                format_ignore_next = true;
            }

            // Collapse a run of consecutive whitespace-only text nodes (left adjacent by
            // extracted `<script>`/`<style>` sections at the root — the parser never merges them):
            // the first node of the run emits the structural break, the rest would double it.
            // Mirrors the blank-collapsing the retired `emit_lines` did. Only in `multiline` mode;
            // the inline callers never see adjacent whitespace-only nodes.
            if multiline
                && i > 0
                && matches!(node, FragmentNode::Text(t) if t.is_collapsible_ws_only)
                && matches!(&trimmed_nodes[i - 1], FragmentNode::Text(p) if p.is_collapsible_ws_only)
            {
                continue;
            }

            // Consume the "previous text trimmed a boundary space" signal once per iteration:
            // snapshot it and clear the field, so no dispatch arm can leak a stale flag by
            // forgetting to reset — the class of bug this whole path has repeatedly hit. Only
            // `handle_text_child` re-arms the field (for the *next* sibling); the block and inline
            // arms are the two readers and take the snapshot by value. The early `continue` paths
            // above run before this and intentionally carry the flag forward untouched.
            let prev_text_ws = std::mem::take(&mut handle_whitespace_of_prev_text);

            if matches!(node, FragmentNode::Text(_)) {
                self.handle_text_child(
                    trimmed_nodes,
                    i,
                    TextChildContext {
                        breakable_exprs,
                        cause,
                        run_has_prose,
                        content_bounds,
                        glued_prefix: pending_glued_prefix.take(),
                        prev_sibling_head,
                    },
                    &mut child_docs,
                    &mut handle_whitespace_of_prev_text,
                );
            } else if multiline && self.is_block_element_node(node) {
                // Block element (div, p, block component): own-line via softlines +
                // forceBreakContent — prettier-plugin-svelte's handleBlockChild. Gated on
                // `multiline` — the convergence path (the multiline element arm) is the only
                // caller that opts in; the legacy non-multiline callers keep routing blocks
                // through handle_inline_child until the element-arm reroute lands (it is
                // currently parked on a corpus parity gap, tracked in internal notes).
                self.handle_block_child(
                    trimmed_nodes,
                    i,
                    force_break,
                    &mut child_docs,
                    prev_text_ws,
                );
            } else if multiline && self.is_own_line_declaration(trimmed_nodes, i) {
                // Declaration (`{@const}` / `{const}` / `{let}` / `{#snippet}`) on its own
                // line — the hoist's layout licence, see `is_own_line_declaration`. Checked
                // BEFORE the control-flow arm: an own-line `{#snippet}` takes its line here
                // (the handler builds it in multiline context, so its body layout is the
                // same the control-flow arm would produce); a snippet glued to content on
                // both sides does not own a line and keeps the control-flow path below.
                self.handle_own_line_tag(trimmed_nodes, i, &mut child_docs);
            } else if multiline && is_control_flow_block(node) {
                // Control-flow block (`{#if}`/`{#each}`/`{#await}`/`{#key}`, and a
                // `{#snippet}` glued to content on both sides) in the convergence path.
                // Mirror path 1's block dispatch.
                //
                // Axis-3 sibling-`>` dangle first: a block directly following an inline-element
                // sibling (no whitespace between) sheds that element's closing `>` onto the
                // block-head line (`</span⏎>{#if…}`) — a deliberate tsv divergence (block-tag
                // wrapping). The element was already pushed as the previous child; swap in its
                // omit-`>` form and append the block that now owns the `>`.
                if let Some((element_doc, block_doc)) =
                    self.try_block_sibling_gt_dangle(trimmed_nodes, i)
                {
                    if let Some(last) = child_docs.last_mut() {
                        *last = element_doc;
                    } else {
                        child_docs.push(element_doc);
                    }
                    child_docs.push(block_doc);
                } else {
                    // No dangle. A block the root marked as part of a SINGLE-LINE inline run builds
                    // in inline context (its long body inner-breaks rather than dropping to its own
                    // line — `is_root_inline_run_block`); every other block builds with
                    // `in_multiline_context=true`, which is what lets a wrapped head
                    // (`{#if a || b || …}`) break its condition and dangle the `}` (the block-tag
                    // wrapping work). The non-multiline callers keep the inline
                    // `build_fragment_node_doc_*` path below.
                    let node_doc = if self.is_root_inline_run_block(node) {
                        self.build_fragment_node_doc_with_preceding_context(
                            node,
                            has_preceding_breakable,
                        )
                    } else {
                        self.build_fragment_node_doc_in_multiline(node)
                    };
                    if let Some(node_doc) = node_doc {
                        child_docs.push(node_doc);
                    }
                }
            } else if is_inline_content(node) {
                // The unit's leading-boundary treatment — the glue test is asked at the unit's
                // HEAD (`i`, where every inline doc below is built), so it names the boundary in
                // front of the whole unit. See `LeadBoundary`.
                let lead = if prev_text_ws {
                    LeadBoundary::Spaced
                } else if self.leading_boundary_glued(trimmed_nodes, i, content_bounds.0) {
                    LeadBoundary::Glued
                } else {
                    LeadBoundary::Plain
                };
                // Axis-3 sibling-`>` dangle onto glued following TEXT: an inline element byte-glued
                // to text on both sides (no whitespace either side, so break-before can't fire)
                // dangles its closing `>` onto the following text's line when that fits, else
                // block-styles — the text-follower analog of the element→element run and the
                // element→block dangle. Checked before the element-run (disjoint: this needs a
                // following TEXT, the run a following element).
                if let Some(dangle_doc) = self.try_build_glued_both_text_dangle(trimmed_nodes, i) {
                    self.push_inline_child_doc(&mut child_docs, dangle_doc, lead);
                }
                // Axis-3 element→element sibling-`>` dangle ("G2"), over a maximal glued RUN: when
                // this element heads a run of 2+ byte-glued inline elements (`<span>foo</span><b>b</b><a…>`),
                // build the whole run as ONE concat, so the preceding text's break-before-flow
                // measurement sees the whole run as a unit — it moves to a fresh line together rather
                // than dangling an opening tag after a space (any single element short enough to fit
                // after the text can't rescue a wide LATER element in the run) — and each adjacent
                // Soft pair sheds its `>` onto the next tag's line. Built once at the head; the tail
                // elements are skipped via `glued_run_consumed_until`.
                else if let Some((run_doc, run_end)) =
                    self.try_build_glued_element_run(trimmed_nodes, i)
                {
                    // Honor a trimmed boundary space from the previous text node exactly as
                    // the single-element path does — the run leads with `group([line, …])` so
                    // an inter-sibling space before a glued run (`</span>` ` ` `<br/><br/>`)
                    // renders (a space when it fits, a break when the fill wraps) rather than
                    // being dropped.
                    self.push_inline_child_doc(&mut child_docs, run_doc, lead);
                    glued_run_consumed_until = run_end + 1;
                } else {
                    self.handle_inline_child(node, &mut child_docs, lead);
                }
            } else if !format_ignore_next
                && let Some((unit_doc, run_end)) =
                    self.try_build_glued_comment_prefixed_element(trimmed_nodes, i)
            {
                // Glued comment prefix (`<!--c--><a…>`): the comment(s) are the element's prefix,
                // so build comments + element as ONE concat here (at the head comment) and skip the
                // tail via `glued_run_consumed_until`. This is the comment analog of the glued
                // element run above — the preceding text's break-before-flow measurement then sees
                // the whole unit flat and moves it to a fresh line together (its `next_is_flow`
                // looked through the comments via `comment_glued_next_flow`), rather than dangling
                // the opening tag after a space. Honor a trimmed boundary space from the previous
                // text exactly as the single-element path does. Guarded on `!format_ignore_next` so
                // a `<!-- prettier-ignore -->` directive still routes to the raw path below.
                // Spaced or Plain only — the fused unit carries no welded-run mark, so an earlier
                // boundary's welded walk ends in front of it.
                let lead = if prev_text_ws {
                    LeadBoundary::Spaced
                } else {
                    LeadBoundary::Plain
                };
                self.push_inline_child_doc(&mut child_docs, unit_doc, lead);
                glued_run_consumed_until = run_end + 1;
            } else if !format_ignore_next
                && !prev_text_ws
                && let Some((prefix, text_idx)) =
                    self.try_build_glued_comment_prefix_for_text(trimmed_nodes, i)
            {
                // Glued comment prefix on a TEXT run (`<!--c-->text1 text2`) — the text sibling of
                // the element arm above, and carried rather than pushed: `handle_text_child` fuses
                // it into the fill's first item. Skipping to `text_idx` (not past it) consumes only
                // the comments, so the text is the next node visited and takes the prefix.
                //
                // The two dispatch guards are the caller's state, which is why they stay here rather
                // than inside the builder: `!format_ignore_next` so a directive still routes to the
                // raw path, and `!prev_text_ws` so a trimmed boundary space from the previous text
                // is never dropped — that space wants the `group([line, …])` wrap
                // `push_inline_child_doc` applies, which a fused prefix has nowhere to carry, so the
                // ordinary per-node path handles it (`glued_lead` then guards the boundary as
                // before).
                pending_glued_prefix = Some((prefix, i));
                glued_run_consumed_until = text_idx;
            } else {
                // Other nodes (comments, `{@const}`/`{@debug}`/`{const}`/`{let}` tags).
                // `has_preceding_breakable` (tracked above) affects whether block conditions use
                // remove_lines(): with preceding breakable content, content breaks first so it
                // respects print_width; without, allow wrapping.
                if let Some(node_doc) = self
                    .build_fragment_node_doc_with_preceding_context(node, has_preceding_breakable)
                {
                    child_docs.push(node_doc);
                }
            }
        }

        // `concat` short-circuits the empty case to `empty()`.
        d.concat(&child_docs)
    }

    /// Whether a node is a **tag** — `{expr}`, `{@html …}`, or `{@render …}`. All three,
    /// not just `ExpressionTag` (the old name said `is_expression_tag` and read as if it
    /// meant only the first).
    ///
    /// These tags use the leading/trailing line fill approach instead of group wrapping,
    /// because group wrapping forces line breaks after multiline expressions.
    fn is_tag_node(node: &FragmentNode<'_>) -> bool {
        matches!(
            node,
            FragmentNode::ExpressionTag(_) | FragmentNode::HtmlTag(_) | FragmentNode::RenderTag(_)
        )
    }

    /// Whether two fragment nodes are **byte-glued** — no source between them (`a`'s end is `b`'s
    /// start). The adjacency test behind the "glued run" *layout* questions in this file (the
    /// sibling-`>` dangle, the break-before travel unit): a glued boundary is render-significant
    /// (breaking it would inject a rendered space), so a glued prefix or element run always
    /// travels as one unit. Any node — including whitespace-only text — between them makes them
    /// non-adjacent.
    ///
    /// ⚠️ The converse does not hold at the ROOT, where a byte gap between consecutive fragment
    /// nodes is a lifted `<script>` / `<style>` / `<svelte:options>` — content the compiler
    /// removes, so the survivors are still render-adjacent. The render-glue question
    /// ([`Self::glued_to_content`]) therefore deliberately does NOT use this test; the layout
    /// callers here read "not glued" across such a gap and merely decline a dangle, which is
    /// layout-conservative rather than a render change.
    fn byte_glued(a: &FragmentNode<'_>, b: &FragmentNode<'_>) -> bool {
        a.span().end == b.span().start
    }

    /// Whether a content text's **leading** edge is glued — its raw slice starts with no
    /// collapsible whitespace, so the boundary in front of it carries no break point and breaking
    /// there would inject a rendered space.
    ///
    /// ⚠️ **This is a claim about the text node's own BYTES, not about sibling adjacency**, and the
    /// distinction is the reason it sits beside [`Self::byte_glued`] rather than folding into it.
    /// The two answer different halves of "is this boundary breakable": this one says the separator
    /// is not inside the *text*, `byte_glued` says there is no *node* between the two siblings. A
    /// caller needs both only where a byte gap can exist between siblings — at the ROOT, where a
    /// lifted `<script>` / `<style>` / `<svelte:options>` leaves one ([`Self::byte_glued`]'s own
    /// warning). Inside any other fragment sibling spans tile, so this predicate alone decides, and
    /// that is why [`Self::build_container_content_doc`] can ask it bare while
    /// [`Self::handle_text_child`] conjoins `byte_glued`.
    ///
    /// The character class is `is_collapsible_ws_char` (`[ \t\n\r]`), deliberately narrower than
    /// ASCII whitespace — a non-breaking space or form feed is rendered content, so a text that
    /// begins with one is glued.
    pub(super) fn text_glued_before(raw: &str) -> bool {
        !raw.starts_with(is_collapsible_ws_char)
    }

    /// Whether a content text's **trailing** edge is glued — the mirror of
    /// [`Self::text_glued_before`], whose doc carries the shared rules.
    pub(super) fn text_glued_after(raw: &str) -> bool {
        !raw.ends_with(is_collapsible_ws_char)
    }

    /// Whether the boundary immediately in FRONT of `nodes[idx]` carries no whitespace — so no break
    /// may land there, since one would inject a rendered space.
    ///
    /// Composes the two halves the question actually has: `byte_glued` says no *node* sits between
    /// the two siblings, and — when that sibling is a text — [`Self::text_glued_after`] says no
    /// whitespace sits at its edge *inside* it. Asking only the first is the mistake
    /// [`Self::glued_comment_run_text`] documents from the other direction.
    ///
    /// Two positions are never glued however the bytes fall: the fragment's own content edge
    /// (`idx <= content_start`), whose boundary belongs to the parent and is trimmed, and a
    /// predecessor that owns its own line ([`Self::is_own_line_declaration`]), which supplies the
    /// break itself.
    fn leading_boundary_glued(
        &self,
        nodes: &[FragmentNode<'_>],
        idx: usize,
        content_start: usize,
    ) -> bool {
        if idx <= content_start {
            return false;
        }
        let Some(j) = idx.checked_sub(1) else {
            return false;
        };
        if self.is_own_line_declaration(nodes, j) || !Self::byte_glued(&nodes[j], &nodes[idx]) {
            return false;
        }
        match &nodes[j] {
            FragmentNode::Text(t) => Self::text_glued_after(t.raw(self.source)),
            _ => true,
        }
    }

    /// Whether the tag at `nodes[tag_idx]` **heads a welded run** — its follower is byte-glued
    /// and stays in the inline run in the OUTPUT: glued content text, an inline
    /// element/component, or another tag. The SPACED text→tag boundary's gate for
    /// [`Self::handle_text_child`]'s `break_before_wide_flow`: a spaced tag enters the flow rule
    /// only as the head of a welded run — one that ends the run keeps the ordinary Case-2
    /// measurement (the separated-tag divergence, `fill_break_before_expr_long`).
    ///
    /// The member set is "stays in the inline run", not "glued in the source": a BLOCK element
    /// follower detaches to its own line by its own layout (render-free at a block boundary), so
    /// a weld into it exists only in the source and measuring through it would grade a unit the
    /// output never has. A comment or control-flow follower is likewise not a member —
    /// conservative there, since the render-side welded walk (`flow_lookahead` in
    /// `arena_render_fill`, whose contract lives on
    /// [`tsv_lang::doc::DocContext::break_before_wide_flow`]) would end at it anyway.
    fn tag_heads_welded_run(&self, nodes: &[FragmentNode<'_>], tag_idx: usize) -> bool {
        nodes.get(tag_idx + 1).is_some_and(|follower| {
            Self::byte_glued(&nodes[tag_idx], follower)
                && match follower {
                    FragmentNode::Text(t) => Self::text_glued_before(t.raw(self.source)),
                    n => self.is_inline_el_or_comp(n) || Self::is_tag_node(n),
                }
        })
    }

    /// Whether the node at `i` is a **declaration that owns its own line** — `{@const}` /
    /// `{const …}` / `{let …}` / `{#snippet}`, unless it is glued to content on both sides.
    ///
    /// Such a node renders nothing and `clean_nodes` hoists it out of its fragment *before* the
    /// whitespace rules run, so the whitespace beside it is never inter-sibling whitespace: at a
    /// fragment edge the run is deleted, and in the interior the two runs it splits merge back
    /// into the single whitespace rule 1 would have produced anyway (`{#if c}a {@const} b{/if}`
    /// compiles to `a b`, exactly like the own-line form). The break is therefore render-free and
    /// the layout question is free, so tsv answers it with the declaration's own line — where
    /// authors already write declarations, and what makes a run of them read as what they are.
    /// The full oracle matrix — every glue position × node kind, graded by `render_compare` —
    /// is `../test-svelte-prettier-whitespace/hoisted-tags.md`.
    ///
    /// ⚠️ **The exception is a declaration glued to content on BOTH sides**, the one shape where
    /// the break is not render-free: `{#if c}a{@const x = 1}b{/if}` compiles to `ab` while the
    /// own-line form compiles to `a b` — a different document. That is the standing "a glued
    /// boundary is never split" rule, and it is what bounds this one.
    ///
    /// ⚠️ **`{@debug}` is excluded** ([`internal::FragmentNode::is_declaration`]): it is not a
    /// declaration but a transient debugging aid, so it keeps the edge *trim* the same hoist
    /// licenses — see [`internal::FragmentNode::is_hoisted_from_fragment`].
    ///
    /// See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
    pub(super) fn is_own_line_declaration(&self, nodes: &[FragmentNode<'_>], i: usize) -> bool {
        nodes[i].is_declaration()
            && !(self.glued_to_content(nodes, i, true) && self.glued_to_content(nodes, i, false))
    }

    /// Whether the node at `i` is glued to the nearest **content** before (`prev`) or after
    /// it — the neighbour the compiler's whitespace rules actually see, which is what decides
    /// whether breaking there would inject a rendered space.
    ///
    /// Three things make a neighbour not-content, and each is the compiler's own answer:
    /// a **hoisted** sibling vanishes from those rules, so the scan steps over it (a run of
    /// `{@const}`s is not glued to itself — stepping over hoisted neighbours can only end at a
    /// text, whose edges answer, or at the fragment edge, which is not content); a
    /// **whitespace-only** text is the separator, not the content; and a content **text** counts
    /// only when its facing edge carries no collapsible whitespace, since that whitespace is the
    /// separator instead. Anything else is content and glues.
    ///
    /// ⚠️ There is deliberately **no byte-adjacency test** here, and adding one was a render
    /// bug. Sibling spans tile every fragment except the ROOT, where `<script>` / `<style>` /
    /// `<svelte:options>` are lifted out of `Fragment::nodes` and leave a byte gap — but the
    /// compiler removes exactly those before its whitespace rules run, so the survivors around
    /// the gap are adjacent to it: `a<script>…</script>{const y = 2}b` renders `ab`, and a
    /// break injected at the gap is a rendered space. A real separator always materializes as
    /// its own whitespace text node (or a text's own edge), which the arms above answer — a
    /// byte gap between consecutive fragment nodes is never render-whitespace. Pinned by
    /// [root_script_gap](../../../../../tests/fixtures/svelte/tags/root_script_gap/).
    fn glued_to_content(&self, nodes: &[FragmentNode<'_>], i: usize, before: bool) -> bool {
        let mut cur = i;
        loop {
            let neighbor = if before {
                cur.checked_sub(1)
            } else {
                Some(cur + 1).filter(|j| *j < nodes.len())
            };
            let Some(j) = neighbor else { return false };
            match &nodes[j] {
                FragmentNode::Text(t) if t.is_collapsible_ws_only => return false,
                FragmentNode::Text(t) => {
                    // The neighbour's FACING edge, so the directions invert: scanning backward we
                    // ask about that text's trailing edge, forward about its leading one.
                    let raw = t.raw(self.source);
                    return if before {
                        Self::text_glued_after(raw)
                    } else {
                        Self::text_glued_before(raw)
                    };
                }
                n if n.is_hoisted_from_fragment() => cur = j,
                _ => return true,
            }
        }
    }

    /// Whether the node at `i` **owns its own line** in a multiline fragment — a block element
    /// (via `handle_block_child`) or a declaration tag (via [`Self::handle_own_line_tag`]).
    ///
    /// The one question both emitters ask of a neighbour: *did that node already supply the break
    /// between us?* Asking it as two separate predicates is how a node that already owns its line
    /// picks up a second break — the failure mode this whole path keeps returning to.
    pub(super) fn owns_own_line(&self, nodes: &[FragmentNode<'_>], i: usize) -> bool {
        self.is_block_element_node(&nodes[i]) || self.is_own_line_declaration(nodes, i)
    }

    /// Whether `nodes` holds a declaration that owns its line — the fragment then cannot
    /// render on one line.
    ///
    /// A *lone* declaration counts too: it touches only the fragment's boundaries, which are
    /// not content, so it is not glued and takes its line all the same — the host goes
    /// multiline (`{#if cond}{@const x = 1}{/if}` formats to the own-line form).
    pub(super) fn has_own_line_declaration(&self, nodes: &[FragmentNode<'_>]) -> bool {
        (0..nodes.len()).any(|i| self.is_own_line_declaration(nodes, i))
    }

    /// Check if a node is a format-ignore comment — the directive that pins the next node's
    /// raw source instead of formatting it. Single recognition point for the three
    /// `build_nodes_doc_*` accumulation loops.
    ///
    // Recognition lives in `tsv_lang::is_format_ignore_directive` — the single source of
    // truth for the directive set, shared across all three language printers.
    fn is_format_ignore_comment(node: &FragmentNode<'_>, source: &str) -> bool {
        matches!(node, FragmentNode::Comment(c) if is_format_ignore_directive(c.content(source)))
    }

    /// Build the verbatim doc for a format-ignored node, or `None` when the node is
    /// whitespace-only text to skip — the pin then carries to the next real node.
    /// Shared leading step of the three `build_nodes_doc_*` accumulation loops; each
    /// caller owns its sink and clears `format_ignore_next` only when this returns `Some`.
    fn format_ignore_raw_doc(&self, node: &FragmentNode<'_>) -> Option<DocId> {
        if let FragmentNode::Text(text) = node
            && text.is_collapsible_ws_only
        {
            return None;
        }
        // The ignored node's subtree can hold `{expr}` / block-head comments (all in
        // `Root.comments`); they ride out inside the raw slice — see
        // `tsv_lang::comment_ledger`.
        Some(self.verbatim_source_doc(node.span()))
    }

    /// Handle a text child node - matches prettier-plugin-svelte's handleTextChild.
    ///
    /// Takes `trimmed_nodes` + the node index `i` (the same shape as `handle_block_child`)
    /// and derives every sibling-kind fact internally, rather than receiving them as a long
    /// list of positional bools. `trimmed_nodes[i]` must be a `FragmentNode::Text`.
    fn handle_text_child(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
        ctx: TextChildContext,
        child_docs: &mut DocBuf,
        handle_whitespace_of_prev_text: &mut bool,
    ) {
        let TextChildContext {
            breakable_exprs,
            cause,
            run_has_prose,
            content_bounds,
            glued_prefix,
            prev_sibling_head,
        } = ctx;
        let multiline = cause.is_multiline();
        let FragmentNode::Text(text) = &trimmed_nodes[i] else {
            return;
        };
        let raw: &str = text.raw(self.source);

        // Sibling-kind facts, derived from the node's position in `trimmed_nodes`.
        //
        // "First"/"last" is asked of the nodes the whitespace rules actually see, so a HOISTED
        // sibling (`{@const}` / `{const}` / `{let}` / `{@debug}` / `{#snippet}` / `<title>`) does not stand
        // between this text and the fragment edge: `clean_nodes` lifts those out before it trims,
        // making this text the real last node and its trailing run a render-free edge run
        // ([`FragmentNode::is_hoisted_from_fragment`]). The bounds are computed once per fragment
        // by the caller rather than scanned here — see [`TextChildContext::content_bounds`].
        let is_first = i <= content_bounds.0;
        let is_last = i >= content_bounds.1;
        let prev_node = i.checked_sub(1).map(|j| &trimmed_nodes[j]);
        let next_node = trimmed_nodes.get(i + 1);
        // A declaration tag on either side owns its own line ([`Self::is_own_line_declaration`]),
        // and the run between it and this text is render-free — the tag hoists out of the fragment,
        // so that run is an edge run whichever side of the tag it sits on. This text therefore
        // trims it and prints no boundary of its own; the line comes from the tag (for a content
        // text) or from this node (for a whitespace-only separator, whose arm is the tag's other
        // side). An authored blank line still survives, as everywhere else.
        let prev_owns_line = i
            .checked_sub(1)
            .is_some_and(|j| self.is_own_line_declaration(trimmed_nodes, j));
        let next_owns_line =
            i + 1 < trimmed_nodes.len() && self.is_own_line_declaration(trimmed_nodes, i + 1);
        let prev_is_inline = prev_node.is_some_and(is_inline_content);
        let prev_is_tag = prev_node.is_some_and(Self::is_tag_node);
        // A byte-glued HTML-comment run (`<!--c--><a…>`) between this text and an inline element
        // makes the comment the element's glued prefix: the break-before coupling must treat the
        // effective next node as that element (skip the comments), so the whole run travels to a
        // fresh line together rather than dangling the opening tag after a space. The comment run
        // is then built + printed with the element as one concat by the main loop's
        // `try_build_glued_comment_prefixed_element` arm — see [`Self::glued_comment_run_element`].
        let comment_glued_next_flow = self
            .glued_comment_run_element(trimmed_nodes, i + 1)
            .is_some();
        let next_is_inline = next_node.is_some_and(is_inline_content) || comment_glued_next_flow;
        let next_is_tag = next_node.is_some_and(Self::is_tag_node);
        // Whether the next sibling is an HTML *inline* element vs a *block* element —
        // the two kinds prettier-plugin-svelte trims boundary whitespace *into* (the
        // trimmed text emits nothing; the element's own group([line, …]) /
        // handle_block_child supplies the break), but under different linebreak rules:
        // an inline element trims only a *space-only* boundary (`!endsWithLinebreak`), a
        // block element trims anything short of a *blank line* (`!endsWithLinebreak(_, 2)`).
        // For anything else (component, `{expr}`, control-flow block, comment) the
        // whitespace text is printed via `splitTextToDocs`, so a newline becomes a hardline.
        let next_is_inline_el = self.next_is_inline_element(trimmed_nodes, i);
        let next_is_block_el = next_node.is_some_and(|n| self.is_block_element_node(n));
        // Whether the next sibling is a flowing inline element OR component (the
        // Fill-idempotency boundary). Text before such a node ends its fill with a trailing
        // `line` so the boundary breaks per width inside the fill (keeping the run idempotent),
        // rather than a `group([line, node])` whose all-or-nothing break flip-flops across
        // passes.
        let next_is_flow =
            next_node.is_some_and(|n| self.is_inline_el_or_comp(n)) || comment_glued_next_flow;
        // Whether the *previous* sibling is a block element — prettier trims a boundary
        // whitespace adjacent to a block but does NOT then wrap the next inline element in
        // `group([line, el])` (`handleWhitespaceOfPrevTextNode = !isBlockElement(prevNode)`),
        // because the block's own `handle_block_child` already supplies the break; wrapping
        // would add a stray leading space after that break.
        let prev_is_block_el = prev_node.is_some_and(|n| self.is_block_element_node(n));
        let position = SiblingPosition::new(is_first, is_last, prev_is_inline, next_is_inline);

        let d = self.d();
        *handle_whitespace_of_prev_text = false;

        // Collapsible whitespace class `[ \t\n\r]` (`is_collapsible_ws_char` —
        // deliberately narrower than prettier-plugin-svelte's `[\t\n\f\r ]`: a form
        // feed is content). A leading/trailing non-breaking space or form feed is
        // content, so a node made only of those is not whitespace-only and is
        // preserved verbatim.
        let has_leading_ws = !Self::text_glued_before(raw);
        let has_trailing_ws = !Self::text_glued_after(raw);

        if text.is_collapsible_ws_only {
            // Whitespace-only text node (never at a fragment boundary — those are skipped
            // by `build_nodes_doc_trimmed`).
            if !multiline {
                // Before a tag the separator is a bare collapsible break — a space while
                // the fragment fits, a newline once it breaks — exactly as the multiline
                // arm below emits it. `group([line, tag])` (the inline-element form) would
                // instead decide the separator on its own width, independently of whether
                // the parent broke: a compact `<small>{a} {b}</small>` that overflows would
                // pack `{a} {b}` onto the block-style content line, while the same document
                // authored across lines splits them. That makes the layout follow the
                // content-boundary whitespace — which is render-free under Svelte 5, and
                // which tsv *injects* when it converts an authoring to block-style, so the
                // emitted form would reflow on the next pass.
                //
                // An inline ELEMENT or component keeps `group([line, el])` deliberately: it
                // carries its own tags, so the group is what lets a wide element drop to its
                // own line whole instead of breaking its tag in place, and both formatters
                // settle on a stable (if authoring-dependent) form there — the sanctioned
                // Tier-2 element-expansion class, not this bug. A tag has no such structure
                // to protect, so the bare break is strictly better.
                if next_is_tag {
                    child_docs.push(d.line());
                } else {
                    // Signal the next inline element to lead with a line.
                    *handle_whitespace_of_prev_text = true;
                }
                return;
            }
            // A separator beside a declaration tag that owns its line: the run is render-free (the
            // tag hoists out of the fragment), and the break is THIS node's to emit — the tag
            // breaks only across a directly adjacent sibling, so exactly one line lands here
            // however either side spelled it. An authored blank line survives as the second.
            if prev_owns_line || next_owns_line {
                if text.newline_count >= 2 {
                    child_docs.push(d.hardline());
                }
                child_docs.push(d.hardline());
                return;
            }
            // Multiline middle whitespace-only text — mirror prettier-plugin-svelte's
            // `handleTextChild` (`index.ts:1308`) + `splitTextToDocs` (`:1353`). The boundary is
            // *trimmed* to a collapsible break — emitted by the next sibling (an inline element's
            // `group([line, …])`, a block element's `handle_block_child` softline) — only when
            // prettier would trim it:
            // - next is an inline element AND the text does NOT end with a linebreak
            //   (`!isTextNodeEndingWithLinebreak`), i.e. a pure space separator; OR
            // - next is a block element AND the text is NOT a blank line
            //   (`!isTextNodeEndingWithLinebreak(_, 2)`).
            // Otherwise the node is printed via `splitTextToDocs`: a newline → `hardline`, a blank
            // line (2+ newlines) → preserved blank `[hardline, hardline]`, a pure space → bare
            // `line` (space when the fragment fits, newline when the parent breaks — what lets a
            // space-separated `{/if} {x}` drop once the `{#if}` forces the parent multiline). A
            // newline before an *inline element* therefore breaks (matching prettier and path 1),
            // rather than collapsing as it did before this convergence.
            //
            let newline_count = text.newline_count as usize;
            // The sibling-newline flow rule ([`Self::sibling_newline_flows`]) reaches this site —
            // a whitespace-only separator between two *non-text* siblings — but only when the
            // separator's own inline RUN holds prose (`run_has_prose`, computed once per run by
            // the caller). That gate is the rule's boundary, and it is structural rather than
            // mechanical: flowing means *reflowing into a text fill*, and a run with no content
            // text has no fill to reflow into. Its newlines are then the author's only structure —
            // a vertical list of siblings — and collapsing them packs independent items onto one
            // line (and, for a short list, lets the collapse cascade into the parent element's own
            // hug decision, an F1 break). See the standalone-separator paragraph in
            // [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
            // Both spellings of a *flowing* separator — an authored space and an authored single
            // newline — must land on the same doc, or the pair diverges (and, once the formatter
            // emits one of them, flip-flops). So the flow test is spelling-independent and the
            // newline arm below simply re-spells itself as the space.
            //
            // The enclosing element's [`MultilineCause`] is the rule's other boundary — the same
            // claim read one level up. Landing both spellings on one doc converges them only if
            // the *arm* they land in is itself spelling-independent, and it is not when the
            // element went multiline BECAUSE of these newlines (`MultilineCause::SourceBreaks`).
            // There, collapsing the separator deletes the very break that chose the multiline arm,
            // so the next pass takes the inline arm, whose `next_is_tag` case emits a bare `line`
            // (all-or-nothing with the already-broken parent group) and splits the run apart
            // again — the two spellings become each other's output, a period-2 cycle rather than a
            // fixed point. So the rule stands down exactly there: inside such an element the
            // newline is not pure spelling, it is the sanctioned Tier-2 element-expansion signal.
            // It holds wherever the multiline layout is structural — the root fragment, block
            // bodies, and any element forced multiline by block children, an expanding block, or a
            // whitespace-collapsing container. Pinned by
            // `elements/inline_content_spaced_tags_tail_long`.
            let separator_flows = run_has_prose
                && cause == MultilineCause::Structural
                && prev_node.is_some_and(|n| self.sibling_newline_flows(n))
                && next_node.is_some_and(|n| self.sibling_newline_flows(n));
            let ws_flows = newline_count == 1 && separator_flows;
            // The rule's own claim, applied literally: a flowing single newline IS the space
            // separator, differently spelled. So it takes the space arm verbatim rather than a
            // parallel one — same doc, same layout, and idempotency by construction. Emitting a
            // *different* collapsible form here (`group([line, el])` where the space emits a bare
            // `line`) is what made the first attempt flip-flop: pass 1 wrote a newline that pass 2
            // then re-read as flowable and collapsed.
            let newline_count = if ws_flows { 0 } else { newline_count };
            let trim_to_collapsible = (next_is_inline_el && newline_count == 0)
                || (next_is_block_el && newline_count < 2);
            if trim_to_collapsible {
                // prettier: `handleWhitespaceOfPrevTextNode = !isBlockElement(prevNode)`. When the
                // previous sibling is a block element its own `handle_block_child` already supplies
                // the separating break, so the next inline element is NOT wrapped in
                // `group([line, el])` (which would strand a leading space after the block's break).
                // `handle_whitespace_of_prev_text` signals the trimmed boundary to the *next*
                // sibling. For a next **block** element it must stay set so the block's
                // `handle_block_child` emits its `break_before` (tsv keeps the text node intact,
                // unlike prettier which trims it, so the flag IS the "boundary was trimmed" signal).
                // For a next **inline** element it follows prettier's
                // `handleWhitespaceOfPrevTextNode = !isBlockElement(prevNode)`: when the previous
                // sibling is a block, its own `handle_block_child` already supplies the break, so the
                // inline element is NOT wrapped in `group([line, el])` (which would strand a leading
                // space after the block's break — `block_before_inline`).
                *handle_whitespace_of_prev_text = !next_is_inline_el || !prev_is_block_el;
            } else if newline_count >= 1 {
                if newline_count >= 2 {
                    child_docs.push(d.hardline());
                }
                child_docs.push(d.hardline());
            } else if next_is_tag && separator_flows {
                // A flowing separator before a TAG. `trim_to_collapsible` above covers only a next
                // inline *element*, so without this arm the boundary would fall to the bare `line`
                // below — which resolves all-or-nothing with the parent group, and the parent is
                // already broken whenever the fragment is multiline. The whole run would then hard-
                // break while the one boundary owned by a content text's fill flowed: the mixed
                // layout this rule exists to remove, just relocated from elements to tags. Deferring
                // to the next sibling gives the tag the same per-width `group([line, tag])` an inline
                // element gets, so the run reflows as one.
                //
                // Gated on `separator_flows` — NOT on `next_is_tag` alone. A plain authored space
                // before a tag keeps the bare `line`: its neighbour may be a **comment**, whose own
                // line is authorship (`<!-- c -->` `{expr}` must not weld — `root_expressions_spaced`),
                // and the flow predicate is exactly what excludes it.
                *handle_whitespace_of_prev_text = true;
            } else {
                child_docs.push(d.line());
            }
            return;
        }

        // A first/last node's boundary run is always trimmed (render-free); interior
        // trimming decisions are made per-sibling below.
        let mut trim_left = is_first;
        let mut trim_right = is_last;

        // Track if we need to add a space to replace trimmed whitespace (fill-adjacency cases)
        let mut add_leading_space = false;
        let mut add_trailing_space = false;

        // If text starts with whitespace and prev is inline element:
        // trim the leading ws and wrap the previous element with a trailing line.
        //
        // For last child: match prettier's handleTextChild early return for idx===last
        // which does NOT wrap the previous element. Instead, the fill starts with a
        // line() element so it can continue on the expression's continuation line
        // (line() → space in flat mode) or break to a new line (line() → newline).
        //
        // For non-last child with breaking prev: skip wrapping because
        // group([breaking_element, line()]) forces the line() to break too,
        // incorrectly separating the closing tag from trailing text.
        let prev_will_break = child_docs.last().is_some_and(|&doc| d.will_break(doc));
        let mut leading_line = false;
        // The mirror of the whitespace-only rule above, on a content text's *leading* run: a
        // SINGLE leading newline after flowing inline content is a spelling difference only, so
        // it falls through to the space arms below and reflows with the fill instead of pinning a
        // hardline. A blank line (2+) still breaks, and a comment predecessor pins its authored
        // line; a block-element predecessor keeps the hardline arm it already had. See
        // [`Self::sibling_newline_flows`].
        //
        // Shared by both the leading run here and the trailing run below — see
        // [`Self::is_separator_like_text`] for why such a node is excluded, and why this one
        // reads the decoded text where the rest of the path reads `raw`.
        //
        // This is the node-local face of the flow rule's prose gate: here the text node IS the
        // run's fill, so `!separator_like_text` answers the same question `run_has_prose` answers
        // for the whitespace-only separator site, which owns no fill (see [`Self::is_run_prose`]).
        // Node-local is the stricter reading and is the one that belongs here — a separator-like
        // node must not flow even when its run holds prose elsewhere, or the fill re-reads a break
        // it emitted itself (the NBSP F1 break).
        let separator_like_text = Self::is_separator_like_text(&text.data(self.source));
        let leading_run = &raw[..raw.len() - raw.trim_start_matches(is_collapsible_ws_char).len()];
        let leading_newline_flows = leading_run.matches('\n').count() == 1
            && !separator_like_text
            && prev_node.is_some_and(|n| self.sibling_newline_flows(n));
        if multiline && prev_owns_line {
            // After a declaration tag's own line: trim the render-free run rather than printing a
            // boundary — the tag's own break_after is the line. Checked ahead of the
            // `splitTextToDocs` linebreak arm below, which would double it.
            trim_left = true;
            add_leading_space = false;
            if leading_run.matches('\n').count() >= 2 {
                child_docs.push(d.hardline());
            }
        } else if multiline
            && text_starts_with_linebreak(raw)
            && !is_first
            && !leading_newline_flows
        {
            // splitTextToDocs (prettier-plugin-svelte): a content text whose leading whitespace
            // carries a newline puts a hardline before its first word — the newline is a
            // structural break (path 1's line-buffer flushes on it), NOT a fold into the prev
            // element. prettier never trims a linebreak boundary, so this fires after *every*
            // previous-sibling kind (inline element, component, tag, control-flow block, comment,
            // block element) — e.g. text after a `{/snippet}` keeps its own line. Folding here
            // would pull a width-breaking first child into a `fill` whose at-line-start re-check
            // drops it onto its own line right after `>`, which re-parses as a leading break and
            // flip-flops the parent element's start boundary (Hug ⇄ Hard).
            trim_left = true;
            add_leading_space = false;
            // A blank line (2+ leading newlines) is preserved as `[hardline, hardline]` —
            // prettier's `splitTextToDocs` startsWithLinebreak(_, 2). A single newline → one
            // hardline.
            let content_start = raw.len() - raw.trim_start_matches(is_collapsible_ws_char).len();
            if raw[..content_start].matches('\n').count() >= 2 {
                child_docs.push(d.hardline());
            }
            child_docs.push(d.hardline());
        } else if multiline && has_leading_ws && !is_first && prev_is_block_el {
            // Content text after a block element with a same-line (space, no linebreak) boundary —
            // the linebreak case is handled above. prettier trims the leading whitespace
            // (`isBlockElement(prevNode) && !startsWithLinebreak → trimTextNodeLeft`); the block's
            // `handle_block_child` break_after already supplies the separating line, so there is NO
            // fold/group here (the inline-element fold below would pop that break_after doc and
            // strand a leading space — `space_after_block_prettier_divergence`).
            trim_left = true;
            add_leading_space = false;
        } else if has_leading_ws && !is_first && position.prev_is_inline() {
            if prev_is_tag && (is_last || !prev_will_break) {
                // Text after expression/html/render tag.
                trim_left = true;
                if breakable_exprs {
                    // Hard-width context (a break-capable expression tag is present): emit a
                    // plain leading space instead of a fill `line`. A `line` here renders in
                    // fits()-Break mode and short-circuits the lookahead of an *earlier*
                    // expression group (the `_ if Break => return true` arm), stranding it flat
                    // and overshooting printWidth. A plain space keeps that group's full fits()
                    // obligation so it breaks instead (the `fill_multiple_expr_long` divergence).
                    add_leading_space = true;
                } else {
                    // Use leading_line in fill instead of wrapping the tag with
                    // group([tag, line()]). The group approach forces line() to break after
                    // multiline tags, pushing text to a new line. leading_line lets fill
                    // continue on the tag's continuation line (line() → space in flat, newline
                    // in break).
                    add_leading_space = false;
                    leading_line = true;
                }
            } else if is_last && prev_will_break {
                // Last child after breaking element (e.g. multiline attrs):
                // skip wrapping because group([breaking_element, line()]) forces
                // line() to break too, incorrectly separating closing tag from text.
                // That forced break is exactly what a TERMINAL tail must not take — it hugs the
                // intact closing tag per the author's space (`inline_wide_content_trailing_long`).
                // Note: non-last text after a breaking *tag* (`prev_is_tag && !is_last &&
                // prev_will_break`) still falls through without action — group() would force
                // line() to break, and leading_line is only for non-breaking continuation. The
                // text's leading ws handles spacing.
            } else if !prev_will_break || !prev_is_tag {
                // The second disjunct is the NON-TERMINAL guard. A text run followed by another
                // flowing element must keep its own line — hugging it onto the closing tag shifts
                // where that element lands, which feeds back into the fit decision
                // (`inline_wide_content_text_sibling_long` is the guard, and its README the
                // reasoning). The wrap below is what produces that own line, so a *breaking*
                // previous element must reach it too: `prev_will_break` says the element already
                // carries a hard break, and skipping the wrap there left the boundary unhandled —
                // the leading run then rode `sibling_newline_flows`' space arm and the text hugged.
                // Reachable only since that flow rule landed: before it a leading newline pinned a
                // hardline, so the fall-through happened to render as the own line anyway.
                trim_left = true;
                add_leading_space = false; // line() handles the space
                // Pop the last doc (the inline element) and rejoin it with the trailing text.
                if let Some(last_doc) = child_docs.pop() {
                    if is_last {
                        // Last child: fold the element and the trailing words into ONE fill so a
                        // wide element wraps its own content within printWidth and the words pack
                        // after it — see `build_after_element_fold`.
                        //
                        // The fold's head is the popped unit, so its leading boundary is the one in
                        // front of `prev_sibling_head` — the same question `glued_lead` asks of a
                        // text run, asked here of an element. A glued head must never drop to a
                        // fresh line: there is no whitespace at that boundary, so the drop would
                        // INJECT a rendered space (`</code>/` `<code>`), and the mangled form is
                        // its own fixed point, so F1 cannot see it.
                        let glued_head = self.leading_boundary_glued(
                            trimmed_nodes,
                            prev_sibling_head,
                            content_bounds.0,
                        );
                        let folded = self.rejoin_inside_leading_wrap(last_doc, |el| {
                            self.build_after_element_fold(el, raw, glued_head)
                        });
                        child_docs.push(folded);
                        return;
                    }
                    // Non-last (text between two inline elements): keep the trailing boundary
                    // grouped WITH the element. The following element supplies the next break
                    // point, and folding the middle text into the element (packing it onto the
                    // dangled `>` line) is non-convergent — it shifts where the following element
                    // lands, flip-flopping across passes. Pinned by
                    // `inline_wide_content_text_sibling_long`.
                    //
                    // A popped element that carries the welded-run marker (`LeadBoundary::Glued`)
                    // takes the marker-hoisting join instead: a bare group would bury the marker,
                    // the preceding boundary's welded walk would stop one node short, and the run
                    // would stand and tear its last element open instead of travelling
                    // (`inline_welded_run_travel_long`'s non-terminal-follower case).
                    let joined = d.try_welded_sibling_join(last_doc).unwrap_or_else(|| {
                        self.rejoin_inside_leading_wrap(last_doc, |el| {
                            d.group(d.concat(&[el, d.line()]))
                        })
                    });
                    child_docs.push(joined);
                }
            }
        } else if multiline && has_leading_ws && !is_first {
            // Same-line space boundary after a sibling none of the arms above claims — a comment or
            // a control-flow block (an inline element, tag, block element and own-line declaration
            // each have their own arm; the linebreak-authored boundary is arm 2). The boundary is
            // inter-node whitespace, so it collapses to one rendered space and a break there is
            // render-equivalent — but it must be the fill's OWN `line`, not a space baked into its
            // first word. Baked in, the fresh-line drop carries the space to the head of the
            // continuation line (`-->⏎\t text1`), which the next pass reads as indentation and drops:
            // the format has no fixed point. `leading_line` is the same parity-shifted mechanism the
            // after-a-tag boundary uses, and the fill renders it Flat (the space) or Break (the
            // newline) by width.
            trim_left = true;
            add_leading_space = false;
            leading_line = true;
        }

        // If text ends with whitespace and next is inline element:
        // trim the trailing ws and either use trailing_line in fill or set flag for next element.
        //
        // For tags (ExpressionTag, HtmlTag, RenderTag): use trailing_line in the fill.
        // group([line, expr]) wrapping forces a newline before multiline expressions;
        // trailing_line lets fill decide whether to break (same approach as leading_line).
        //
        // For non-tag inline elements: set handle_whitespace_of_prev_text so the next
        // element gets wrapped with group([line, element]).
        let mut trailing_line = false;
        // Count newlines in the trailing whitespace run (multiline structural-break detection).
        let trailing_ws_newlines = if has_trailing_ws {
            let content_end = raw.trim_end_matches(is_collapsible_ws_char).len();
            raw[content_end..].matches('\n').count()
        } else {
            0
        };
        let mut trailing_hardlines = 0usize;
        // The third face of the same rule (after the whitespace-only separator and a content
        // text's leading run): a SINGLE trailing newline before flowing inline content is a
        // spelling difference only, so it falls through to the space arms below and reflows with
        // the fill. Blank lines, comments and block elements keep the structural hardline. See
        // [`Self::sibling_newline_flows`].
        let trailing_newline_flows = trailing_ws_newlines == 1
            && !separator_like_text
            && next_node.is_some_and(|n| self.sibling_newline_flows(n));
        if multiline && next_owns_line {
            // Mirror of the leading arm: the tag below supplies the line, so this run is trimmed
            // rather than printed. Reached at a fragment edge too, where `is_last` already trims —
            // the arm is what carries the *interior* position, and the blank line in both.
            trim_right = true;
            add_trailing_space = false;
            if trailing_ws_newlines >= 2 {
                trailing_hardlines = 1;
            }
        } else if multiline && trailing_ws_newlines >= 1 && !is_last && !trailing_newline_flows {
            // splitTextToDocs (prettier-plugin-svelte): a content text whose trailing whitespace
            // carries a newline ends with a structural `hardline` (a blank line — 2+ newlines —
            // becomes `[hardline, hardline]`). prettier never trims a linebreak boundary, so this
            // fires before *every* next-sibling kind (inline element, component, tag, control-flow
            // block, comment, block element). Matches path 1, whose line buffer flushes on the
            // trailing newline — replacing the collapsible `group([line, …])` / `trailing_line`
            // the inline path uses for a same-line (space-only) boundary.
            trim_right = true;
            add_trailing_space = false;
            trailing_hardlines = if trailing_ws_newlines >= 2 { 2 } else { 1 };
        } else if has_trailing_ws && !is_last && position.next_is_inline() {
            if is_first || next_is_tag {
                if breakable_exprs && !is_first {
                    // Hard-width context: plain trailing space before the tag instead of a fill
                    // `line` (a `line` short-circuits this node's own preceding expression group;
                    // see the leading branch). A first child has no preceding group, so it falls
                    // through to the fill's own trailing space (matching the plain path-3 layout).
                    trim_right = true;
                    add_trailing_space = true;
                } else {
                    // First child or middle child before tag: trailing line in fill
                    add_trailing_space = false;
                    trailing_line = true;
                    if !is_first {
                        trim_right = true;
                    }
                }
            } else if multiline && next_is_flow {
                // Multiline middle child before a flowing inline element / component (space-only
                // boundary): end the fill with a trailing `line` so the boundary breaks per width
                // inside the fill — the `next_is_flow` boundary, which keeps the run idempotent.
                // A `group([line, node])` here breaks all-or-nothing and flip-flops across passes
                // (the Fill-idempotency bug class).
                trim_right = true;
                add_trailing_space = false;
                trailing_line = true;
            } else if !is_first {
                // Non-multiline inline callers: wrap the next element with `group([line, element])`.
                trim_right = true;
                add_trailing_space = false;
                *handle_whitespace_of_prev_text = true;
            }
        }

        // The run's LEADING boundary is byte-glued to the previous sibling — no whitespace there,
        // so there is no break point WITHIN the boundary: moving the run's first word alone to a
        // fresh line would inject a rendered space (and the mangled form is a fixed point, so F1 is
        // blind to it). A first node's boundary is the parent's (trimmed), and a previous sibling
        // that owns its own line supplies the break itself, so neither is glued.
        //
        // There are two ways to honor that, and which one applies is decided by whether the prefix
        // can be **carried**:
        // - A glued COMMENT run is fused into the fill's first item (`glued_prefix`, supplied by the
        //   caller). The boundary is then unbreakable *structurally* — a fill breaks only between
        //   items — and, because the unit is one item, it travels to a fresh line **together** when
        //   it doesn't fit. That is the mirror of `try_build_glued_comment_prefixed_element`, which
        //   does the same for a comment run prefixing an inline element.
        // - Any other glued predecessor (an element, a tag, a control-flow block) is a sibling with
        //   its own layout, already built and placed, so there is nothing to fuse. There
        //   [`DocContext::glued_lead`] suppresses the fill's fresh-line drop at the head instead —
        //   the run renders in place and breaks at its first *internal* whitespace boundary. It is
        //   the mirror of `break_before_wide_flow`'s glued half, on the other end of the run.
        //
        // Both arms now also travel: an overflowing unit moves to the breakable boundary one hop
        // back instead of standing and overrunning. The flagged arm gets it from the RENDERER
        // rather than from fusion — the flag puts this run into the welded unit a preceding text's
        // `break_before_wide_flow` look-ahead walks (`flow_lookahead`), so the boundary in front of
        // the whole unit is what breaks. Fusion could never have closed it: an element carries its
        // own groups, so folding it into the fill's item 0 would force the fits check to measure it
        // flat.
        //
        // ⚠️ **Fusing does not retire the flag — it MOVES which boundary the flag is about.** The
        // fused unit begins at the comment, so the break point in front of it is the one in front of
        // the *comment*, and that one can be glued too (`0<!--c-->text`). Before the fusion the
        // comment was its own sibling doc and that boundary was safe by construction — sibling docs
        // in a concat have no break between them — so asking only about the text's own edge was
        // enough; afterwards the boundary is a real fill boundary and must be guarded. Missing this
        // injected a rendered space at `0<!--`, found by the seeded fuzzer mutating the fixture, and
        // by nothing else: the mangled form is a fixed point, so F1 is blind and the corpus does not
        // carry the shape.
        let unit_head = glued_prefix.map_or(i, |(_, head)| head);
        let glued_lead = (glued_prefix.is_some() || !has_leading_ws)
            && self.leading_boundary_glued(trimmed_nodes, unit_head, content_bounds.0);

        // Build fill for this text node's words.
        // leading_line: fill starts with line() (text after expression tag)
        // trailing_line: fill ends with line() (text before expression tag or first-child)
        if add_leading_space {
            child_docs.push(d.text(" "));
        }
        if let Some(fill_doc) = self.build_text_fill_doc_trimmed(
            raw,
            trim_left,
            trim_right,
            leading_line,
            trailing_line,
            glued_prefix.map(|(doc, _)| doc),
        ) {
            // Text immediately before a flowing inline element/component ends with a trailing
            // `line`. Couple that boundary to the wide-element drop at render position: if the
            // following element won't fit flat as a whole, the trailing `line` breaks so the
            // element drops to its own line whole rather than packing onto the text line and
            // breaking its own tag in place. The newline-authored boundary already does this (it
            // emits a hardline); this makes the space-authored boundary converge to the same form.
            //
            // Couple the break to the wide-element drop whether the preceding text is a first or a
            // middle child: an inline element preceded by same-line content that must wrap starts on
            // a fresh line rather than dangling its opening tag at the end of the text line (the
            // `inline_break_before_*` divergences). tsv converges every authoring to that form where
            // prettier keeps the opening tag on the text line — see conformance_prettier.md §Svelte:
            // Inline content block-style. A first-child element with NO preceding text is unaffected
            // (it never reaches this text handler; the fold's head hug still guards its idempotency).
            //
            // The run's two ends are INDEPENDENT questions, so they are answered independently and
            // carried on one context rather than by an if/else chain that made them look exclusive:
            // a run can be glued at both ends at once, and the three flags reach disjoint fill cases
            // (the head's drop at `offset == 0`, the trailing measurement at `is_final_segment`).
            //
            // `break_before_wide_flow` — one boundary rule, both authored shapes (the render side
            // routes each to the right fill case by parity — see the flag's doc):
            // - **space-separated** (`… word <a…>`, `trailing_line`): the trailing `line` is the
            //   Case-2 separator; measuring the following element/run flat breaks it so a wide
            //   element drops to its own line whole rather than packing onto the text line.
            // - **glued** (`… glued<a…>`, `!has_trailing_ws`, no separator): the glued word is the
            //   Case-1 last item; the same flat measurement breaks at the whitespace boundary BEFORE
            //   the glued word so the whole glued run moves to a fresh line together, never splitting
            //   the glued boundary (which would inject a rendered space).
            // A glued TAG joins on both shapes — as the smallest welded unit (`… glued{x}`, the
            // word and its tag travel together) or welded onward through the run (`… glued{x}<a…>`,
            // `… word {expr}.w<b>…`): which member of the welded unit crosses the width cannot
            // matter, so the unit is measured through the tag and travels whole. There is no
            // run-ending carve-out: prettier instead keeps a run-ending tag outside the fill and
            // lets it ride past printWidth after the word it is welded to — tsv breaks at the
            // whitespace boundary in front of the word, holding the hard limit (a cataloged
            // divergence — conformance_prettier.md §Print Width Philosophy,
            // fill_glued_tag_travel_long).
            //
            // Either way an inline element preceded by same-line content that must wrap starts on a
            // fresh line rather than dangling its opening tag at the text line's end (the
            // `inline_break_before_*` divergences) — tsv converges every authoring to that form where
            // prettier keeps the opening tag on the text line (conformance_prettier.md §Svelte:
            // Inline content block-style). Not `multiline`-gated: a single-line-authored run that
            // must wrap by width still converges to the fresh-line form (a short run that fits is a
            // no-op).
            //
            // The boundary's two shapes, split as the flag's own contract describes them
            // ([`tsv_lang::doc::DocContext::break_before_wide_flow`] carries the render-side
            // mechanics — the whole-flat pairwise measurement and the welded walk):
            let break_before_wide_flow = if has_trailing_ws {
                // SPACED half: the trailing `line` is the separator; a flowing element — or a
                // tag heading a welded run ([`Self::tag_heads_welded_run`], which carries the
                // member set; a spaced tag that ENDS the run keeps the ordinary Case-2
                // measurement) — couples it to the whole-unit measurement.
                trailing_line
                    && (next_is_flow
                        || (next_is_tag && self.tag_heads_welded_run(trimmed_nodes, i + 1)))
            } else {
                // GLUED half: no separator — the boundary in front of the last word is the
                // break point, and ANY tag joins: the welded word+tag pair is the smallest
                // welded unit (conformance_prettier.md §Print Width Philosophy,
                // fill_glued_tag_travel_long), and the render walk extends the measurement
                // through whatever glue actually SURVIVES in the output, stopping at the
                // first non-glued entry.
                next_is_flow || next_is_tag
            };
            let fill_doc = if break_before_wide_flow || glued_lead {
                d.with_context(
                    fill_doc,
                    tsv_lang::doc::DocContext::default()
                        .with_break_before_wide_flow(break_before_wide_flow)
                        .with_glued_lead(glued_lead),
                )
            } else {
                fill_doc
            };
            child_docs.push(fill_doc);
        }
        if add_trailing_space {
            child_docs.push(d.text(" "));
        }
        for _ in 0..trailing_hardlines {
            child_docs.push(d.hardline());
        }
    }

    /// Rejoin a popped inline element with the trailing text `build_tail` builds around it,
    /// keeping the element's **leading** boundary outside that tail.
    ///
    /// `handle_text_child` pops the previous sibling to rejoin it with the text that follows, and
    /// the popped doc is either the bare element or `push_inline_child_doc`'s inline-sibling wrap
    /// `group([line, X])` — the collapsible boundary to the sibling before it. Two boundaries then
    /// meet on one element, and they are **independent decisions**: the leading one asks whether
    /// the element fits after its sibling, the trailing one whether the text fits after the
    /// element. Building the tail around the whole wrap welds them into one group, where either
    /// breaking forces the other. Hoisting the boundary back out afterwards keeps them separate,
    /// and is why both arms route through here rather than each re-deriving the shape.
    ///
    /// The weld is not merely untidy — it costs the document its fixed point, differently per arm:
    ///
    /// - **Terminal tail** (the after-element fold): the boundary is *double-counted* — the fill
    ///   breaks before the fold AND the wrapping group re-renders its own leading line flat,
    ///   stranding a leading space (`inline_break_before_prev_inline_long`).
    /// - **Non-terminal tail**: the welded group measures the trailing boundary against the column
    ///   *before* the leading break — a column that no longer exists once the leading boundary
    ///   breaks and the element starts a fresh line. The next pass, reading that fresh line,
    ///   measures the trailing boundary from it and answers differently, so the two passes
    ///   disagree forever (`inline_sibling_drop_tail_flow_long`). Breaking outside-in is what makes
    ///   the first pass ask the question the second pass will ask.
    ///
    /// The tail keeps the trailing boundary grouped *with* the element on purpose: an element too
    /// wide to sit flat must push its tail to the next line, and only measuring the two together
    /// sees that. Detaching the trailing line to decide on its own column packs a tail after an
    /// element that wrapped its own attributes, which the next pass then unpacks.
    fn rejoin_inside_leading_wrap(
        &self,
        last_doc: DocId,
        build_tail: impl FnOnce(DocId) -> DocId,
    ) -> DocId {
        let d = self.d();
        match d.strip_leading_line_group(last_doc) {
            Some(inner) => d.inline_sibling_line_group(build_tail(inner)),
            None => build_tail(last_doc),
        }
    }

    /// Handle an inline child element - matches prettier-plugin-svelte's handleInlineChild
    fn handle_inline_child(
        &self,
        node: &FragmentNode<'_>,
        child_docs: &mut DocBuf,
        lead: LeadBoundary,
    ) {
        if let Some(node_doc) = self.build_fragment_node_doc(node) {
            self.push_inline_child_doc(child_docs, node_doc, lead);
        }
    }

    /// Push an already-built inline child doc with its leading-boundary treatment
    /// ([`LeadBoundary`], whose variants carry each case's contract).
    ///
    /// Shared by the single-element path (`handle_inline_child`), the glued-element-run path and
    /// the comment-prefixed-unit path in `build_nodes_doc`, so a trimmed boundary space is never
    /// dropped before a byte-glued run (`</span>` ` ` `<br/><br/>`) — the single-sibling case
    /// already worked because a run of one falls through to `handle_inline_child`.
    fn push_inline_child_doc(&self, child_docs: &mut DocBuf, node_doc: DocId, lead: LeadBoundary) {
        match lead {
            LeadBoundary::Spaced => {
                // The single producer of the inline-sibling wrap; `DocArena::strip_leading_line_group`
                // (the after-element fold's matcher, a crate away) is its exact inverse. Routing through
                // the named constructor keeps the two in lockstep — a shape drift here would silently
                // return `None` there and reintroduce the stray-space non-idempotency.
                child_docs.push(self.d().inline_sibling_line_group(node_doc));
            }
            LeadBoundary::Glued => {
                child_docs.push(
                    self.d().with_context(
                        node_doc,
                        tsv_lang::doc::DocContext::default()
                            .with_glued_lead(true)
                            .with_glued_atom(true),
                    ),
                );
            }
            LeadBoundary::Plain => child_docs.push(node_doc),
        }
    }

    /// Whether `node` ends the inline run it sits in — anything that owns its own line (a block
    /// element, a control-flow block, a comment: the `sibling_newline_flows` complement) or an
    /// authored blank line. A whitespace-only node with a single newline is a *separator within*
    /// the run, and a content text is run content, so neither breaks it.
    ///
    /// ⚠️ The two `Text` arms must stay **ahead** of the delegation, not fold into it.
    /// [`Self::sibling_newline_flows`] deliberately does not model `Text` (its `_ => false` arm
    /// is about neighbours it never sees), so delegating a content text would read back as
    /// "breaks the run" and cut every run at its own prose — the one thing the run exists to
    /// find.
    pub(super) fn breaks_inline_run(&self, node: &FragmentNode<'_>) -> bool {
        match node {
            FragmentNode::Text(t) if t.is_collapsible_ws_only => t.newline_count >= 2,
            FragmentNode::Text(_) => false,
            _ => !self.sibling_newline_flows(node),
        }
    }

    /// Whether `node` is **prose** — a text node carrying a word for a `fill` to pack. An
    /// NBSP-only node is a separator wearing content's clothing
    /// ([`Self::is_separator_like_text`]) and is not prose, for the same reason it is excluded
    /// from the flow rule itself.
    ///
    /// This is the run-level spelling of the node-local `!separator_like_text` test the content
    /// text sites already make: at those sites the node *is* the run's fill, so asking about the
    /// node and asking about the run are the same question. The sites that own no fill have to
    /// ask it of the run instead — the whitespace-only separator here, and
    /// `Printer::content_is_reflowable_fill` in `element_analysis.rs`, which decides whether an
    /// element's render-free content boundary may select its layout. Both are the one "is there
    /// a fill to reflow into?" question, so they share this predicate rather than each spelling
    /// out what counts as prose.
    pub(super) fn is_run_prose(&self, node: &FragmentNode<'_>) -> bool {
        matches!(node, FragmentNode::Text(t)
            if !t.is_collapsible_ws_only && !Self::is_separator_like_text(&t.data(self.source)))
    }

    /// Scan the inline run beginning at `start`: its exclusive end, and whether it holds prose
    /// ([`Self::is_run_prose`]) — which is what puts a `fill` in the run for a flowing separator
    /// to reflow into.
    ///
    /// Called only when the caller's cursor reaches a fresh run, so the scans partition
    /// `trimmed_nodes` and cost O(n) across the whole fragment — not the O(n²) a per-separator
    /// rescan would cost on a long all-flowing run (a generated per-token `<span>` list). A
    /// run-breaking node at `start` is its own one-node span: the loop stops immediately and the
    /// `max` advances the cursor past it, so the caller cannot stall on it.
    fn scan_inline_run(&self, nodes: &[FragmentNode<'_>], start: usize) -> (usize, bool) {
        let mut end = start;
        let mut has_prose = false;
        while end < nodes.len() && !self.breaks_inline_run(&nodes[end]) {
            has_prose = has_prose || self.is_run_prose(&nodes[end]);
            end += 1;
        }
        (end.max(start + 1), has_prose)
    }

    /// Whether a **single-newline** separator beside `node` may collapse to a plain space.
    ///
    /// Svelte 5 collapses an inter-sibling whitespace run to one whitespace, so a space and a
    /// newline between two siblings render identically — the newline's *spelling* carries no
    /// signal and the fill may reflow it. (Its *presence* still does: a glued boundary is never
    /// split, since breaking there would inject a rendered space.) So an inline sibling isolated
    /// by authored newlines flows back onto the content line, converging those authorings.
    ///
    /// Four neighbours are excluded, none of them a mere spelling difference:
    /// - a **comment**, whose authored position is authorship — folding one into a text fill
    ///   would relocate it across a semantic boundary (§Comment Position Philosophy);
    /// - a **block element**, which owns its own line via `handle_block_child`;
    /// - a **blank line** (2+ newlines), a Tier-2 authoring signal, screened by the callers;
    /// - a **control-flow block** (`{#if}` / `{#each}` / `{#key}` / `{#await}` / `{#snippet}`),
    ///   which has **no way to pay an overflow except by tearing itself open**. An inline element
    ///   that cannot fit drops to its own line *whole*, tags intact (`break_before_wide_flow`) —
    ///   that escape is what makes flowing safe for elements, and it does not reach a block. A
    ///   block's head and tail wrap a *fragment*, so its only available break is at its own
    ///   head↔body seam: the body node lands on its own line and the flowed sibling text welds
    ///   to the tail (`{#key key}⏎text6⏎{/key}text7` — `root_text_control_flow_adjacent`), and
    ///   in a run of several blocks only the ones straddling the width boundary expand, so
    ///   identical constructs render differently by horizontal accident.
    ///
    ///   ⚠️ Do NOT re-derive this as "a block's width is not fixed" — that is false, and was the
    ///   doc's old wording: a breaking `{expr}` tag expands mid-run too (`{f(⏎…⏎)}text4`). The
    ///   difference is *where the break lands* — inside the tag's own expression (its call
    ///   arguments), leaving both of the tag's outer adjacencies untouched, versus at a block's
    ///   seam with its own children. So this exclusion is a consequence of a **missing
    ///   mechanism**, not a property of blocks: admitting them is gated on giving a block the
    ///   same whole-unit drop (widening the `next_is_flow` / `break_before_wide_flow` coupling
    ///   past `is_inline_el_or_comp`), not on widening this predicate. The yield is real —
    ///   admitting blocks as-is converges ~39 more `authoring_audit` sites — which is exactly
    ///   why the bar is the resulting layout rather than the count.
    ///
    /// Note this is orthogonal to whether the *element* lays out multiline, which an authored
    /// newline does still decide and which is preserved — so the convergence target is the
    /// multiline form, never a collapsed one-liner. See
    /// [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
    fn sibling_newline_flows(&self, node: &FragmentNode<'_>) -> bool {
        match node {
            // A tag has fixed width and no structure to protect — always flows.
            FragmentNode::ExpressionTag(_)
            | FragmentNode::RenderTag(_)
            | FragmentNode::HtmlTag(_) => true,
            // An inline element/component flows; a block one owns its line.
            FragmentNode::Element(_) | FragmentNode::SpecialElement(_) => {
                !self.is_block_element_node(node)
            }
            // Everything else keeps its authored line — the exclusions the doc comment argues
            // for: a `Comment` (its position is authorship) and a control-flow block (its width
            // is not fixed, so packing a run of them is paid for by expanding their bodies).
            // `Text` never appears here: a run this rule inspects belongs to the text node.
            _ => false,
        }
    }

    /// Whether this text node is a **separator** wearing content's clothing: every character it
    /// holds is whitespace (an NBSP / narrow NBSP acting as the gap between two siblings). Such
    /// a node carries no word for the fill to pack, so treating its surrounding run as
    /// reflowable would merely re-read a break the fill itself emitted —
    /// `<span>a</span>⏎<nbsp><span>b</span>` collapses onto one line on the next pass, an F1
    /// break. Its collapsible whitespace bounds a separator, not content, which is why it is
    /// excluded for the same reason a whitespace-only node is.
    ///
    /// Keyed on the **decoded** text, not the raw bytes: `&nbsp;` and a literal U+00A0 are the
    /// same document to the compiler, so spelling the separator as an entity must not buy it a
    /// different layout. (Every other read on this path uses `raw` — this is the one question
    /// about what the characters *are* rather than where they sit.)
    ///
    /// ⚠️ The test is over the **whole** decoded text, with no trim first. Trimming
    /// collapsible whitespace off the ends and asking about the remainder answers "not
    /// separator-like" for the node that decodes to nothing BUT collapsible whitespace —
    /// an entity-encoded space or tab (`&#9;`, `&#x20;`, `&Tab;`), whose raw bytes make it
    /// content to [`internal::Text::is_collapsible_ws_only`] while its decoded form carries
    /// no word at all. Such a node then reported as the run's *prose*, made the content a
    /// `fill`, and collapsed an authored break its literal-space twin keeps — see
    /// [inline_separator_entity_newline](../../../../../tests/fixtures/svelte/elements/inline_separator_entity_newline/).
    /// Both call sites are pre-guarded by `!is_collapsible_ws_only`, so an all-collapsible-
    /// whitespace node never reaches here and the empty-string arm is unreachable in practice.
    ///
    /// ⚠️ **The class is Rust's UNICODE `char::is_whitespace`, deliberately wider than the
    /// `is_collapsible_ws` class the rest of this path uses — do not "fix" it to match.** The
    /// printer asks two separator questions of a text node's decoded content, same shape,
    /// different class, and each needs its own:
    ///
    /// - *Does the fill have a word to pack here?* — **this** one, WIDE. An NBSP is not a word,
    ///   so a node made only of NBSPs is a separator and its run is not reflowable.
    /// - *Is this separator interchangeable with a plain space?* — `is_one_line_separator`
    ///   (`element_analysis.rs`), NARROW. An NBSP is **not**: it renders as itself and never
    ///   collapses, so it may not be respelled and may not pick a layout.
    ///
    /// An `&nbsp;` node therefore answers yes here and no there, which is correct on both
    /// counts — and a single run may legitimately hold one node of each kind, so neither
    /// predicate can stand in for the other. `Printer::run_is_one_line` documents the same
    /// split from the third side.
    fn is_separator_like_text(data: &str) -> bool {
        !data.is_empty() && data.chars().all(char::is_whitespace)
    }

    /// Whether a node is a block-level *element* — the `handleBlockChild` set in
    /// prettier-plugin-svelte (`isBlockElement`): an HTML block element, a block special
    /// element, or a block component. Excludes control-flow blocks (`{#if}` etc. — they
    /// separate via the whitespace-break path) and inline elements/components.
    pub(super) fn is_block_element_node(&self, node: &FragmentNode<'_>) -> bool {
        matches!(
            node,
            FragmentNode::Element(_) | FragmentNode::SpecialElement(_)
        ) && self.is_block_fragment_node(node)
    }

    /// Handle a block-element child — mirrors prettier-plugin-svelte's `handleBlockChild`:
    /// add a break before and/or after the block so it lands on its own line.
    ///
    /// `force_break` is prettier's `forceBreakContent` (the fragment mixes a block with >1
    /// child). When set, the boundary is a **hardline** rather than prettier's
    /// softline+`break_parent`: in tsv a `break_parent` sibling poisons a *preceding* group's
    /// `fits()` lookahead (`BreakParent => false`), wrongly expanding it, whereas a `hardline`
    /// forces the same break and `fits()` stops cleanly at it. With `force_break` true the two
    /// are equivalent (every block boundary breaks anyway); a lone block (`force_break` false)
    /// emits a collapsible `softline` and never reaches this hardening.
    ///
    /// - **before** when the previous sibling exists, is not itself a block element, and is
    ///   either a non-text node or a text whose boundary whitespace was consumed (the
    ///   `prev_text_ws` snapshot) or trimmed away (no longer ends with ws).
    /// - **after** when the next sibling exists and is either a non-text node, or content
    ///   text (or an empty text immediately followed by an inline element) that does **not**
    ///   start with a linebreak — a leading-linebreak text supplies its own break.
    fn handle_block_child(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
        force_break: bool,
        child_docs: &mut DocBuf,
        prev_text_ws: bool,
    ) {
        let d = self.d();
        let sep = || {
            if force_break {
                d.hardline()
            } else {
                d.softline()
            }
        };
        let prev = i.checked_sub(1).map(|j| (j, &trimmed_nodes[j]));
        let next = trimmed_nodes.get(i + 1);

        // A previous sibling that owns its own line ([`Self::owns_own_line`] — a block element or
        // a declaration tag) already emitted the break after itself; asking only about block
        // elements here is how the tag's line picks up a second one.
        let break_before = match prev {
            Some((j, _)) if self.owns_own_line(trimmed_nodes, j) => false,
            Some((_, FragmentNode::Text(t))) => {
                prev_text_ws || Self::text_glued_after(t.raw(self.source))
            }
            Some(_) => true,
            None => false,
        };
        if break_before {
            child_docs.push(sep());
        }

        if let Some(node_doc) = self.build_fragment_node_doc(&trimmed_nodes[i]) {
            child_docs.push(node_doc);
        }

        let break_after = match next {
            Some(FragmentNode::Text(t)) => {
                let raw = t.raw(self.source);
                let is_empty_ws = t.is_collapsible_ws_only;
                // idx+2 is an inline element (prettier's `isInlineElement`, excludes components)
                let next2_inline = self.next_is_inline_element(trimmed_nodes, i + 1);
                (!is_empty_ws || next2_inline) && !text_starts_with_linebreak(raw)
            }
            Some(_) => true,
            None => false,
        };
        if break_after {
            child_docs.push(sep());
        }
    }

    /// Emit a declaration on its own line — the `handle_block_child` of
    /// [`Self::is_own_line_declaration`].
    ///
    /// The break is emitted **only across a directly adjacent sibling**. A whitespace-only
    /// separator emits its own break instead (`handle_text_child`'s whitespace arm), and a
    /// neighbour that owns its own line already emitted a break after itself — so exactly one line
    /// lands at each side however the author spelled it. The adjacent content text contributes
    /// nothing: its facing run is render-free here, so its own handler trims it rather than
    /// printing a boundary.
    ///
    /// A `{#snippet}` builds in multiline context, exactly as the control-flow arm would build
    /// it — this handler decides only the node's LINE, never its body layout (a wrapped head
    /// must still break its params and dangle the `}`).
    fn handle_own_line_tag(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
        child_docs: &mut DocBuf,
    ) {
        let d = self.d();
        let break_before = i.checked_sub(1).is_some_and(|j| {
            !trimmed_nodes[j].is_whitespace_only_text() && !self.owns_own_line(trimmed_nodes, j)
        });
        if break_before {
            child_docs.push(d.hardline());
        }

        let node = &trimmed_nodes[i];
        let node_doc = if is_control_flow_block(node) {
            self.build_fragment_node_doc_in_multiline(node)
        } else {
            self.build_fragment_node_doc(node)
        };
        if let Some(node_doc) = node_doc {
            child_docs.push(node_doc);
        }

        if trimmed_nodes
            .get(i + 1)
            .is_some_and(|n| !n.is_whitespace_only_text())
        {
            child_docs.push(d.hardline());
        }
    }

    /// Build a doc for a node sequence in multiline / block context.
    ///
    /// The single entry point for the formerly-separate "path 1" line-buffer printer: it now
    /// delegates to the unified [`Self::build_nodes_doc_trimmed`] in `multiline` mode (trimmed
    /// boundaries; prettier's `printChildren` model — block-child softlines + `forceBreakContent`,
    /// `splitTextToDocs` boundary hardlines, the control-flow-block `in_multiline_context` /
    /// root-inline-run dispatch, and the sibling-`>` dangle). `breakable_exprs` opts a fragment
    /// carrying a break-capable expression tag into the hard-width multi-expression layout
    /// (`fill_multiple_expr_long`).
    pub(crate) fn build_nodes_doc_multiline(&self, nodes: &[FragmentNode<'_>]) -> DocId {
        let breakable_exprs = Self::nodes_have_breakable_expression(nodes);
        // `Structural`: these callers are the root fragment, block bodies, and special elements —
        // none of them has an enclosing element whose multiline-ness the content's own newlines
        // could flip, so the sibling-newline flow rule stays in force here.
        self.build_nodes_doc_trimmed(nodes, breakable_exprs, MultilineCause::Structural)
    }

    /// Build the content of a **whitespace-collapsing container** (`<table>`, `<select>`, … —
    /// `tsv_html::collapses_child_whitespace`) block-style: every non-whitespace child on its own
    /// line, with the inter-sibling whitespace **trimmed**. Svelte's compiler removes that
    /// whitespace entirely (`clean_nodes` `can_remove_entirely`), so this is render-equivalent to
    /// the inline form and reproduces the block-authored form both formatters already keep — see
    /// [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
    ///
    /// ⚠️ **That licence stops at a TEXT child, and the boundary is where it stops.**
    /// `can_remove_entirely` removes a node only when its data is exactly `' '` — a whitespace-only
    /// run between two *non-text* children. A boundary with a **content text** child lands *inside*
    /// that text node, where the run collapses to a rendered space instead of vanishing. So such a
    /// boundary is reproduced as authored rather than block-styled: a **glued** boundary gets no
    /// separator at all (a break there would inject a rendered space — and the mangled form is a
    /// fixed point, so F1 cannot see it), and an authored **space** is spent on the `hardline`, the
    /// text itself being built trimmed so the space is not written twice (which would strand a
    /// leading space the next pass reads as indentation and drops). The container's own **edges**
    /// stay free either way — `clean_nodes` strips the first and last text node's outer whitespace
    /// whatever the parent — so a lone text child still takes its own line. Pinned by
    /// [`ws_collapsing_container_text_child`](../../../../../tests/fixtures/svelte/elements/ws_collapsing_container_text_child_prettier_divergence/).
    ///
    /// Whitespace-only text nodes are dropped — with one carry-over: an **authored blank line**
    /// (2+ newlines) is a Tier-2 authoring signal preserved block-style everywhere else, so it
    /// survives (collapsed to a single blank) between the two children it separates, exactly as
    /// `handle_text_child`'s `newline_count >= 2` does on the general path. Every non-whitespace
    /// node (element, control-flow block, comment, tag) is built in multiline context and
    /// `hardline`-separated. A `<!-- prettier-ignore -->` directive still suppresses the next node
    /// (emitted raw), and a whitespace-only node between the directive and the ignored node is
    /// skipped without clearing the pending flag. `can_remove_entirely` keys on the **direct**
    /// element parent, so this runs only for the container's own content — a nested `{#each}` body
    /// builds through the ordinary path (its parent is the block, not the container), matching the
    /// compiler.
    pub(super) fn build_container_content_doc(&self, nodes: &[FragmentNode<'_>]) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        let mut format_ignore_next = false;
        // A skipped inter-sibling whitespace run carrying a blank line: the run itself is trimmed
        // (render-free), but the blank line is carried to the next child as a doubled separator.
        let mut pending_blank = false;
        // The previous child was a content TEXT node with no trailing whitespace, so the boundary
        // to whatever follows is glued: no separator may be emitted there.
        let mut prev_text_glued_after = false;
        for node in nodes {
            // Trim inter-sibling whitespace — render-free in this container — but remember an
            // authored blank line so the next child reintroduces it.
            if let FragmentNode::Text(t) = node
                && t.is_collapsible_ws_only
            {
                if t.newline_count >= 2 {
                    pending_blank = true;
                }
                continue;
            }
            // One dispatch, three outcomes: the node's doc plus whether each of its boundaries is
            // GLUED (no separator may be emitted there). Only a content text child can be glued —
            // every other kind sits behind a whitespace-only run the container removes entirely, so
            // its boundaries are always free. A content text is built TRIMMED (see the doc comment)
            // and its authored whitespace, not a re-emitted space, decides each separator.
            let (node_doc, glued_before, glued_after) = if format_ignore_next {
                format_ignore_next = false;
                (self.format_ignore_raw_doc(node), false, false)
            } else if let FragmentNode::Text(t) = node {
                let raw = t.raw(self.source);
                (
                    self.build_text_fill_doc_trimmed(raw, true, true, false, false, None),
                    Self::text_glued_before(raw),
                    Self::text_glued_after(raw),
                )
            } else {
                if Self::is_format_ignore_comment(node, self.source) {
                    format_ignore_next = true;
                }
                (
                    self.build_fragment_node_doc_in_multiline(node),
                    false,
                    false,
                )
            };
            if let Some(node_doc) = node_doc {
                if !parts.is_empty() && !prev_text_glued_after && !glued_before {
                    parts.push(d.hardline());
                    if pending_blank {
                        parts.push(d.hardline());
                    }
                }
                pending_blank = false;
                prev_text_glued_after = glued_after;
                parts.push(node_doc);
            }
        }
        d.concat(&parts)
    }

    /// Check if a fragment node is a block-level node (needs its own line)
    ///
    /// Components are NOT treated as blocks - like Prettier, they're printed inline.
    /// The line structure comes from whitespace in text nodes, not from node types.
    fn is_block_fragment_node(&self, node: &FragmentNode<'_>) -> bool {
        match node {
            // Defer to the one block-element adapter (component + script/style overlay).
            FragmentNode::Element(el) => self.is_block_element(el),
            FragmentNode::SpecialElement(el) => el.kind.is_block(),
            _ => is_control_flow_block(node),
        }
    }

    /// Check if fragment content should force breaking due to block elements.
    ///
    /// Matches prettier's `forceBreakContent`: when there are multiple non-whitespace
    /// children and at least one is a block element, content should break.
    /// This forces the multiline path even for "inline" Svelte block bodies.
    ///
    /// A declaration tag that owns its own line ([`Self::has_own_line_declaration`]) forces
    /// the same break — its line is one the fragment must have room for, so the block's
    /// inline/expanding fast path (`fragment_inline_authored`) may not take an inline authoring
    /// at its word.
    pub(super) fn fragment_should_force_break_content(&self, nodes: &[FragmentNode<'_>]) -> bool {
        let non_ws_count = nodes
            .iter()
            .filter(|n| !n.is_whitespace_only_text())
            .count();
        (non_ws_count > 1 && nodes.iter().any(|n| self.is_block_fragment_node(n)))
            || self.has_own_line_declaration(nodes)
    }

    /// Whether the node at `trimmed_nodes[i + 1]` is an **inline HTML element** (`<span>`, `<a>`,
    /// an inline special element) — prettier-plugin-svelte's `isInlineElement`, which **excludes
    /// components** (they are neither inline nor block). Used by `handle_text_child` (a space-only
    /// boundary before such an element trims to a collapsible `group([line, element])`) and by
    /// `handle_block_child` (the `idx + 2` inline-element lookahead). The broader
    /// element-or-component flow set is [`Self::is_inline_el_or_comp`].
    fn next_is_inline_element(&self, trimmed_nodes: &[FragmentNode<'_>], i: usize) -> bool {
        match trimmed_nodes.get(i + 1) {
            Some(FragmentNode::Element(el)) => {
                el.kind != internal::ElementKind::Component && !self.is_block_element(el)
            }
            Some(node @ FragmentNode::SpecialElement(_)) => !self.is_block_fragment_node(node),
            _ => false,
        }
    }

    /// Whether a node is a flowing inline element or **component** — the set that participates
    /// in a text↔element fill boundary on *either* side (the preceding-element fold trigger and
    /// the following-element flow boundary). Any non-block `Element`/`SpecialElement`; block
    /// elements and every non-element node are excluded. Unlike [`Self::next_is_inline_element`]
    /// (a sibling-only predicate that *excludes* components, because a space-separated component
    /// sibling breaks to its own line), this includes components: a wide `<Comp>` adjacent to
    /// flowing text is the case the Fill-idempotency fix targets.
    fn is_inline_el_or_comp(&self, node: &FragmentNode<'_>) -> bool {
        matches!(
            node,
            FragmentNode::Element(_) | FragmentNode::SpecialElement(_)
        ) && !self.is_block_fragment_node(node)
    }

    /// Build a doc for a single fragment node.
    ///
    /// Returns None for whitespace-only text nodes that should be skipped.
    fn build_fragment_node_doc(&self, node: &FragmentNode<'_>) -> Option<DocId> {
        self.build_fragment_node_doc_impl(node, false, false)
    }

    /// Build a fragment node doc with multiline context awareness.
    ///
    /// When `in_multiline_context` is true, blocks with symmetric spaces
    /// (spaces but no newlines) will expand to multiline format.
    fn build_fragment_node_doc_in_multiline(&self, node: &FragmentNode<'_>) -> Option<DocId> {
        self.build_fragment_node_doc_impl(node, true, false)
    }

    /// Build a fragment node doc with preceding content context.
    ///
    /// When `has_preceding_breakable` is true, block conditions will use remove_lines()
    /// to ensure earlier content breaks before the condition.
    fn build_fragment_node_doc_with_preceding_context(
        &self,
        node: &FragmentNode<'_>,
        has_preceding_breakable: bool,
    ) -> Option<DocId> {
        self.build_fragment_node_doc_impl(node, false, has_preceding_breakable)
    }

    fn build_fragment_node_doc_impl(
        &self,
        node: &FragmentNode<'_>,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> Option<DocId> {
        match node {
            FragmentNode::Text(text) => self.build_text_doc(text),
            FragmentNode::Element(element) => Some(self.build_element_doc(element)),
            FragmentNode::SpecialElement(element) => Some(self.build_special_element_doc(element)),
            FragmentNode::ExpressionTag(tag) => Some(self.build_expression_tag_doc(tag)),
            FragmentNode::Comment(comment) => Some(self.build_html_comment_doc(comment)),
            FragmentNode::IfBlock(_)
            | FragmentNode::EachBlock(_)
            | FragmentNode::AwaitBlock(_)
            | FragmentNode::KeyBlock(_)
            | FragmentNode::SnippetBlock(_) => self.build_control_flow_block_doc(
                node,
                in_multiline_context,
                has_preceding_breakable,
                None,
            ),
            FragmentNode::HtmlTag(tag) => Some(self.build_html_tag_doc(tag)),
            FragmentNode::ConstTag(tag) => Some(self.build_const_tag_doc(tag)),
            FragmentNode::DeclarationTag(tag) => Some(self.build_declaration_tag_doc(tag)),
            FragmentNode::DebugTag(tag) => Some(self.build_debug_tag_doc(tag)),
            FragmentNode::RenderTag(tag) => Some(self.build_render_tag_doc(tag)),
        }
    }

    /// Axis-3 sibling-`>` dangle: when a control-flow block directly follows (no
    /// whitespace) an inline-element sibling, build the element without its closing `>`
    /// and hand that `>` to the block so it dangles onto the block-head line when the
    /// block renders multiline. Returns `(element_without_gt, block_with_gt)`, or `None`
    /// to keep the pair hugged. The `>` only moves *into* the closing tag (`</tag⏎>{#…}`),
    /// injecting no render-significant whitespace.
    ///
    /// The dangle keys on whether the block actually renders multiline, not on how its
    /// body is authored — so it is a fixed point on its own output (the dangled form's
    /// own-line body would otherwise read as authored-multiline on a second pass):
    /// - a conditional block (an inline-authored body that may stay inline or expand on
    ///   width) folds the `>` into its own inline-vs-multiline `conditional_group`
    ///   (`build_expanding_construct`/`build_expanding_block` via `fold_gt`);
    /// - a block that unconditionally breaks (authored-multiline / forced) dangles the `>`
    ///   onto its own line (`⏎>` prefix), applied on the non-expanding tails by `dangle_gt`.
    ///
    /// Both happen inside the single `build_block_node_doc_with_gt` build — the block is
    /// built **once**, with the `>` threaded in, so a nested chain of dangles stays linear
    /// (an earlier two-build probe-then-rebuild was O(2^depth) in nesting).
    ///
    /// Applies to the four rendering block heads (`{#if}` / `{#each}` / `{#key}` /
    /// `{#await}`) — and to the one `{#snippet}` shape that still reaches the control-flow
    /// arm, a snippet glued to content on BOTH sides (an own-line snippet takes its line
    /// via [`Self::is_own_line_declaration`] before this can fire). A control-flow block
    /// with any preceding sibling routes its block parent through the multiline-fragment
    /// layout (`has_control_flow_after_sibling` → `compute_multiline_cause`), so the
    /// block's body-drop keys on `can_wrap` (true here) and the dangle is a one-pass fixed
    /// point — including for `{#await}`, whose body-drop is likewise gated on `can_wrap`.
    fn try_block_sibling_gt_dangle(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
    ) -> Option<(DocId, DocId)> {
        let block = trimmed_nodes.get(i)?;
        if !is_control_flow_block(block) {
            return None;
        }
        let prev = trimmed_nodes.get(i.checked_sub(1)?)?;
        let FragmentNode::Element(element) = prev else {
            return None;
        };
        // Inline element, directly adjacent (no whitespace between it and the block).
        if self.is_block_fragment_node(prev) || !Self::byte_glued(prev, block) {
            return None;
        }
        let element_doc = self.build_inline_element_omit_close_gt(element)?;
        let gt = self.d().text(">");
        // Build the block exactly once with the `>` threaded in: the expanding path folds
        // it into the inline-vs-multiline `conditional_group` (hug inline, dangle when the
        // block expands); the non-expanding tails dangle it via `dangle_gt` when they break.
        // (An earlier form built the block twice — a throwaway no-`gt` probe to test
        // `will_break`, then a rebuild — which made nested dangles O(2^depth).)
        let block_doc = self.build_block_node_doc_with_gt(block, gt)?;
        Some((element_doc, block_doc))
    }

    /// The element→element analog of [`Self::try_block_sibling_gt_dangle`] ("G2"), generalized from
    /// a pair to a maximal glued RUN: when `nodes[i]` HEADS a run of 2+ byte-glued inline elements,
    /// build the whole run as one concat (see [`Self::build_glued_element_run`]) and return
    /// `(run_doc, run_end)` — the last index the run covers, so the caller can skip the tail.
    /// `None` when `nodes[i]` is not an inline element or has no glued inline-element follower (the
    /// caller handles it as an ordinary inline child). Detecting at the head and skipping the tail
    /// keeps the build O(run length); a walk-back-and-rebuild at each element would be O(length²).
    /// The closing-`>` dangle onto glued following TEXT: when the inline element at `i` is
    /// byte-glued to content text on **both** sides — no whitespace either side, so the
    /// break-before rule cannot fire — build it as
    /// [`Printer::build_inline_element_close_gt_dangle`], the three-state group that dangles the
    /// closing `>` onto the following text's line when that fits and block-styles otherwise. The
    /// text-follower analog of the element→element run ([`Self::try_build_glued_element_run`]) and
    /// the element→block dangle ([`Self::try_block_sibling_gt_dangle`]). `None` unless the
    /// glued-both-text shape holds and the element is the eligible flat hug-both form.
    fn try_build_glued_both_text_dangle(
        &self,
        nodes: &[FragmentNode<'_>],
        i: usize,
    ) -> Option<DocId> {
        let node = nodes.get(i)?;
        // Inline element only — a block `<div>` reaching this arm goes multiline, never dangles.
        let FragmentNode::Element(element) = node else {
            return None;
        };
        if self.is_block_fragment_node(node) {
            return None;
        }
        // glued-before: the previous node is content text byte-glued with no trailing whitespace
        // (a trailing space would be a break-before boundary, handled elsewhere). Symmetric with the
        // glued-after check below — `is_collapsible_ws_only` excludes an empty / whitespace-only prev text
        // (which carries no content the element could be glued *to*).
        let prev = nodes.get(i.checked_sub(1)?)?;
        let FragmentNode::Text(pt) = prev else {
            return None;
        };
        if pt.is_collapsible_ws_only
            || !Self::byte_glued(prev, node)
            || !Self::text_glued_after(pt.raw(self.source))
        {
            return None;
        }
        // glued-after: the next node is content text byte-glued with no leading whitespace (so the
        // dangled `>` leads that text's line; a leading space would wrap at the space instead).
        let next = nodes.get(i + 1)?;
        let FragmentNode::Text(nt) = next else {
            return None;
        };
        if nt.is_collapsible_ws_only
            || !Self::byte_glued(node, next)
            || !Self::text_glued_before(nt.raw(self.source))
        {
            return None;
        }
        self.build_inline_element_close_gt_dangle(element)
    }

    fn try_build_glued_element_run(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
    ) -> Option<(DocId, usize)> {
        let node = trimmed_nodes.get(i)?;
        if !matches!(node, FragmentNode::Element(_)) || self.is_block_fragment_node(node) {
            return None;
        }
        // Extend forward over the unbroken byte-glued chain of inline elements.
        let mut end = i;
        while let Some(next) = trimmed_nodes.get(end + 1) {
            if matches!(next, FragmentNode::Element(_))
                && !self.is_block_fragment_node(next)
                && Self::byte_glued(&trimmed_nodes[end], next)
            {
                end += 1;
            } else {
                break;
            }
        }
        // A lone element (no glued follower) is an ordinary inline child.
        if end == i {
            return None;
        }
        let run_doc = self.build_glued_element_run(trimmed_nodes, i, end)?;
        Some((run_doc, end))
    }

    /// If `nodes[i]` **begins** a byte-glued run of one or more HTML comments, return the index of
    /// the node the run ends at — the first non-comment member. Every comment in the run must be
    /// byte-adjacent to the next node (`<!--a--><!--b-->X`); any whitespace inside the run stops it
    /// (`None`), as does running off the end, and a **format-ignore directive** anywhere in the run.
    /// Whitespace *before* `nodes[i]` is the boundary the break lands on — but a *glued comment*
    /// before `nodes[i]` makes it a non-head member of a longer run, and only the head opens the
    /// unit (`None` otherwise).
    ///
    /// The run is the terminator's glued **prefix**: no break may land between them, so the two
    /// travel as one. Which terminators qualify is the caller's question, and there are two, one
    /// per way of carrying a prefix — [`Self::glued_comment_run_element`] (built with the element as
    /// one concat) and [`Self::glued_comment_run_text`] (fused into the text run's fill).
    ///
    /// Two bail conditions beyond "not a clean glued run":
    /// - **Head-only** — a comment byte-glued *after* another comment is a non-head member, and the
    ///   head already decided the run's fate (a suffix of a run that failed to resolve fails the
    ///   same way). Bailing in O(1) here, rather than re-scanning from each member, keeps a long
    ///   *unresolved* glued-comment run linear instead of O(run length²): the member then builds
    ///   individually via the ordinary path — identical output, since it would have returned `None`.
    /// - **Directive** — a `<!-- prettier-ignore -->` / `format-ignore` comment must reach the
    ///   per-node path so it suppresses its target; absorbing it into a glued unit would format the
    ///   very node it means to pin.
    fn glued_comment_run_end(&self, nodes: &[FragmentNode<'_>], i: usize) -> Option<usize> {
        if !matches!(nodes.get(i)?, FragmentNode::Comment(_)) {
            return None;
        }
        // Head-only guard (linear-cost): a comment glued after another comment is a non-head member.
        if let Some(p) = i.checked_sub(1)
            && matches!(&nodes[p], FragmentNode::Comment(_))
            && Self::byte_glued(&nodes[p], &nodes[i])
        {
            return None;
        }
        let mut j = i;
        loop {
            // A format-ignore directive anywhere in the run (head or interior) routes to the
            // per-node path so the directive is honored — never swallowed into the glued unit.
            if Self::is_format_ignore_comment(&nodes[j], self.source) {
                return None;
            }
            let next = nodes.get(j + 1)?;
            if !Self::byte_glued(&nodes[j], next) {
                return None; // whitespace inside the run — not a single glued unit
            }
            match next {
                FragmentNode::Comment(_) => j += 1,
                _ => return Some(j + 1),
            }
        }
    }

    /// The [`Self::glued_comment_run_end`] run that ends at an **inline element/component**
    /// (`<!--a--><!--b--><a…>`), returning that element's index. The break-before machinery then
    /// measures comments + element as one unit (see
    /// [`Self::try_build_glued_comment_prefixed_element`] and [`Self::handle_text_child`]'s
    /// `comment_glued_next_flow`).
    fn glued_comment_run_element(&self, nodes: &[FragmentNode<'_>], i: usize) -> Option<usize> {
        let end = self.glued_comment_run_end(nodes, i)?;
        self.is_inline_el_or_comp(&nodes[end]).then_some(end)
    }

    /// The [`Self::glued_comment_run_end`] run that ends at a **content text**
    /// (`<!--a-->text1 text2`), returning that text's index.
    ///
    /// The text-terminated sibling of [`Self::glued_comment_run_element`], and the two are the same
    /// claim about the same boundary — only the way of carrying the prefix differs, because a text
    /// run is a `fill` rather than a single doc. Here the comments are **fused into the fill's first
    /// item** ([`Self::build_text_fill_doc_trimmed`]'s `glued_prefix`), so the unit is one fill item
    /// and the fill physically cannot break inside it — the same guarantee the element arm gets from
    /// building one concat. A **whitespace-only** text does not qualify: it is the separator, not
    /// content, so there is nothing to be glued to.
    ///
    /// ⚠️ **Span adjacency is not enough here, and this is where the two terminators genuinely
    /// differ.** An element's leading boundary is the byte gap before its `<`, so
    /// [`Self::byte_glued`] settles it; a text node's boundary lives *inside* the node, as its own
    /// leading whitespace run. `<!--c--> text1` tiles exactly as `<!--c-->text1` does — the space is
    /// the text's first byte — so the edge must be asked separately
    /// ([`Self::text_glued_before`]). Without that the spaced boundary would be fused too, welding a
    /// run that has a perfectly good break point (`fill_after_comment_spaced_long_prettier_divergence`
    /// is the fixture that says so).
    fn glued_comment_run_text(&self, nodes: &[FragmentNode<'_>], i: usize) -> Option<usize> {
        let end = self.glued_comment_run_end(nodes, i)?;
        matches!(&nodes[end], FragmentNode::Text(t)
            if !t.is_collapsible_ws_only && Self::text_glued_before(t.raw(self.source)))
        .then_some(end)
    }

    /// Build the comments of a glued run — `nodes[start..end]`, the members before its terminator —
    /// as ONE concat. The single producer for both carriers
    /// ([`Self::try_build_glued_comment_prefixed_element`] and the text arm in
    /// [`Self::build_nodes_doc_trimmed`]), so the prefix a run resolves to cannot depend on which
    /// terminator claimed it.
    fn build_glued_comment_run_doc(
        &self,
        nodes: &[FragmentNode<'_>],
        start: usize,
        end: usize,
    ) -> Option<DocId> {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        for node in &nodes[start..end] {
            parts.push(self.build_fragment_node_doc(node)?);
        }
        Some(d.concat(&parts))
    }

    /// When `nodes[i]` heads a glued HTML-comment run ending in an inline element
    /// ([`Self::glued_comment_run_element`]), build the comments + the element as ONE concat and
    /// return `(unit_doc, end)` — the last index the unit covers, so the caller skips the tail via
    /// `glued_run_consumed_until`. The comment prefix travels with the element: because the unit is
    /// a plain concat, the preceding text's break-before-flow measurement sees the whole thing flat
    /// (`welded_atom` → `None`), so a wide element pulls its comment prefix to the fresh
    /// line together rather than dangling the opening tag after a space. The element may itself head
    /// a glued-element run (G2) — reuse [`Self::try_build_glued_element_run`] there — else it is an
    /// ordinary inline child. `None` when `nodes[i]` is not a glued-comment prefix.
    fn try_build_glued_comment_prefixed_element(
        &self,
        nodes: &[FragmentNode<'_>],
        i: usize,
    ) -> Option<(DocId, usize)> {
        let elem_idx = self.glued_comment_run_element(nodes, i)?;
        // Build the element (or the glued-element run it heads), then prepend the comment docs.
        let (elem_doc, end) = match self.try_build_glued_element_run(nodes, elem_idx) {
            Some((run_doc, run_end)) => (run_doc, run_end),
            None => (self.build_fragment_node_doc(&nodes[elem_idx])?, elem_idx),
        };
        let prefix = self.build_glued_comment_run_doc(nodes, i, elem_idx)?;
        Some((self.d().concat(&[prefix, elem_doc]), end))
    }

    /// When `nodes[i]` heads a glued HTML-comment run ending in a content TEXT
    /// ([`Self::glued_comment_run_text`]), build the comments as ONE concat and return
    /// `(prefix_doc, text_idx)` — the doc the text's fill fuses into its first item, and the index
    /// of the text that takes it (also the exclusive bound of what this consumes, since the text
    /// itself still has to be visited).
    ///
    /// The text-terminated sibling of [`Self::try_build_glued_comment_prefixed_element`]: same run,
    /// same prefix, different **carrier**. There the prefix is concatenated with the element and
    /// pushed as one child doc; here it is handed forward, because a text run's doc is a `fill` and
    /// a `fill` breaks between its items — so the only place a prefix is safe is *inside* item 0.
    fn try_build_glued_comment_prefix_for_text(
        &self,
        nodes: &[FragmentNode<'_>],
        i: usize,
    ) -> Option<(DocId, usize)> {
        let text_idx = self.glued_comment_run_text(nodes, i)?;
        Some((
            self.build_glued_comment_run_doc(nodes, i, text_idx)?,
            text_idx,
        ))
    }

    /// Build a maximal run of byte-adjacent (glued) inline **elements** — `nodes[start..=end]`,
    /// all plain non-block `Element`s (`None` if any isn't) — as ONE concat. Two effects, both the
    /// point of the "run travels together" posture:
    ///
    /// - **break-before as a unit**: the preceding text's break-before-flow measurement measures
    ///   this whole concat flat (`welded_atom` returns `None` for a plain concat → the
    ///   whole thing), so a wide element anywhere in the run pulls the *entire* run to a fresh line
    ///   rather than stranding an opening tag after a space.
    /// - **per-pair sibling-`>` dangle (G2)**: each adjacent pair whose BOTH elements are
    ///   Soft-eligible sheds the first's closing `>` onto the second's line (`</span⏎><a⏎…`); the
    ///   receiver renders it as a leading `if_break` inside its attrs group, so it hugs when the
    ///   attrs fit and dangles when they wrap. A mid-run element both receives (from its left) and
    ///   sheds (to its right).
    ///
    /// Eligibility is a per-element property (a flat hug-both `Soft` layout), computed up front for
    /// every element because a pair's shed decision needs BOTH neighbours' eligibility — a shed
    /// whose receiver turned out ineligible would strand the `>`. Against an ineligible neighbour
    /// the boundary stays an intact `>` (the element renders its ordinary doc), so nothing is ever
    /// lost. The `>` moves only *inside* a closing tag, so every reparse is byte-identical —
    /// render-safe.
    fn build_glued_element_run(
        &self,
        nodes: &[FragmentNode<'_>],
        start: usize,
        end: usize,
    ) -> Option<DocId> {
        let d = self.d();
        let mut els: SmallVec<[&internal::Element<'_>; 8]> = SmallVec::new();
        let mut eligible: SmallVec<[bool; 8]> = SmallVec::new();
        for node in &nodes[start..=end] {
            let FragmentNode::Element(el) = node else {
                return None;
            };
            if self.is_block_fragment_node(node) {
                return None;
            }
            eligible.push(self.build_inline_element_omit_close_gt(el).is_some());
            els.push(el);
        }
        let n = els.len();
        let mut parts: SmallVec<[DocId; 8]> = SmallVec::new();
        for idx in 0..n {
            let sheds = idx + 1 < n && eligible[idx] && eligible[idx + 1];
            let receives = idx > 0 && eligible[idx] && eligible[idx - 1];
            let doc = if sheds || receives {
                let gt = if receives { Some(d.text(">")) } else { None };
                // `sheds || receives` implies `eligible[idx]`, so this is `Some`.
                self.build_inline_element_sibling_gt(els[idx], sheds, gt)?
            } else {
                self.build_fragment_node_doc(&nodes[start + idx])?
            };
            parts.push(doc);
        }
        Some(d.concat(&parts))
    }

    /// Dispatch a control-flow block (`{#if}` / `{#each}` / `{#key}` / `{#await}` /
    /// `{#snippet}`) to its `_with_full_context` builder with shared context: multiline
    /// flag, preceding-breakable flag, and an optional preceding sibling's split-off closing
    /// `>` (`gt_prefix`) to fold into the expanding layout. Returns `None` for any
    /// non-control-flow node. The single wiring point for both the normal fragment dispatch
    /// (`build_fragment_node_doc_impl`) and the sibling-`>` dangle (`build_block_node_doc_with_gt`).
    fn build_control_flow_block_doc(
        &self,
        node: &FragmentNode<'_>,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> Option<DocId> {
        Some(match node {
            FragmentNode::IfBlock(b) => self.build_if_block_doc_with_full_context(
                b,
                in_multiline_context,
                has_preceding_breakable,
                gt_prefix,
            ),
            FragmentNode::EachBlock(b) => self.build_each_block_doc_with_full_context(
                b,
                in_multiline_context,
                has_preceding_breakable,
                gt_prefix,
            ),
            FragmentNode::KeyBlock(b) => self.build_key_block_doc_with_full_context(
                b,
                in_multiline_context,
                has_preceding_breakable,
                gt_prefix,
            ),
            FragmentNode::AwaitBlock(b) => self.build_await_block_doc_with_full_context(
                b,
                in_multiline_context,
                has_preceding_breakable,
                gt_prefix,
            ),
            FragmentNode::SnippetBlock(b) => {
                self.build_snippet_block_doc_with_full_context(b, gt_prefix)
            }
            _ => return None,
        })
    }

    /// Dispatch a control-flow block, threading a preceding sibling's split-off closing `>`
    /// (`gt`) into its expanding layout (in-multiline context, no preceding breakable — the
    /// dangle path forces both). See `build_control_flow_block_doc` and the caller's gate.
    fn build_block_node_doc_with_gt(&self, node: &FragmentNode<'_>, gt: DocId) -> Option<DocId> {
        self.build_control_flow_block_doc(node, true, false, Some(gt))
    }

    //
    // Text nodes
    //

    /// Append `s` as `[word, line, word, …]` fill parts (a `line` between words, none before
    /// the first / after the last) directly into `parts` — no intermediate buffer. Split on
    /// [`split_collapsible_ws`], matching `build_text_fill_doc_trimmed`'s word split (so a
    /// non-breaking space or form feed stays attached). Used by the inline-element fold so the
    /// words after a folded element pack greedily into the surrounding fill rather than moving as
    /// one nested unit.
    fn extend_with_word_fill(&self, parts: &mut DocBuf, s: &str) {
        let d = self.d();
        let mut first = true;
        for word in split_collapsible_ws(s) {
            if !first {
                parts.push(d.line());
            }
            first = false;
            parts.push(d.text_pooled(word));
        }
    }

    /// Build the after-element fold doc: one `fill([element, line, word …])` so the element's
    /// closing `>` stays intact while the words pack greedily after it. A wide element whose
    /// content overflows wraps within print width and dangles its closing `>` on a low column;
    /// the trailing text then packs after it. Used by the inline/trimmed text path
    /// ([`Self::handle_text_child`]) when an inline element is the **last** child before trailing
    /// text — the only position that folds. A non-terminal text run (one followed by another
    /// flowing element) is never folded here: packing it onto the dangled `>` line is
    /// non-convergent, pinned by
    /// [`inline_wide_content_text_sibling_long`](../../../../../tests/fixtures/svelte/elements/inline_wide_content_text_sibling_long_prettier_divergence/).
    ///
    /// A **short** element (its content fits flat) packs like every other fill word: when it drops
    /// to its own line — whether pushed there by the preceding text or dropped mid-fill — the
    /// trailing text flows greedily after it rather than being isolated (matching prettier's
    /// pairwise fill; a preceding sibling doesn't change that). A **wide** element that wraps still
    /// dangles its `>` and the terminal tail hugs it (`DocContext::after_element_fold`).
    fn build_after_element_fold(&self, prev: DocId, raw: &str, glued_lead: bool) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        parts.push(prev);
        parts.push(d.line());
        self.extend_with_word_fill(&mut parts, raw);
        let fill = d.fill(&parts);
        // The fold's marker — a statement of what this fill IS, not of a layout preference, and the
        // sole site that sets it. Three things follow from it (see [`DocContext::after_element_fold`]):
        // the head hugs rather than drops when it is too wide for its own line anyway (dropping would
        // strand a spurious `>⏎<child` break — the nested-`<span>` non-idempotency); the terminal
        // trailing text hugs the dangled `>` once the head has wrapped, respecting the author's space
        // boundary (the fold only ever runs for terminal text); and a *preceding* text run's
        // break-before measurement can extract the head alone via `welded_atom`.
        //
        // `glued_lead` is the fold's OTHER end, and it is the ordinary flag with the ordinary
        // meaning — the head element is byte-glued to the previous sibling, so no break may land in
        // front of it. Two readers, and both were blind while the fold omitted it: the head's
        // fresh-line drop (which would inject a rendered space at the glued boundary) and
        // `welded_entry`, by which a preceding text run's break-before measurement extends across
        // a welded run to the element on its far side. Nothing but the render oracle can catch a
        // regression in the first: the split output is its own fixed point, so F1, the fuzzer and
        // `authoring_audit` all pass straight through it.
        d.with_context(
            fill,
            tsv_lang::doc::DocContext::default()
                .with_after_element_fold(true)
                .with_glued_lead(glued_lead),
        )
    }

    /// Build a doc for a text node
    ///
    /// Returns None for empty text; a whitespace-only node collapses to a single
    /// inter-sibling space. For text with content, normalizes internal whitespace to
    /// single spaces (fill).
    fn build_text_doc(&self, text: &internal::Text) -> Option<DocId> {
        let raw = text.raw(self.source);
        // ASCII (collapsible) whitespace only: a non-breaking space (U+00A0) is content,
        // so a node made only of NBSP is NOT empty here and flows to the fill path below
        // (preserved verbatim), never dropped or collapsed to a regular space.
        let trimmed = raw.trim_matches(is_collapsible_ws_char);
        if trimmed.is_empty() {
            // Pure (ASCII) whitespace: collapse to a single inter-sibling space
            if raw.bytes().any(is_collapsible_ws) {
                Some(self.d().text(" "))
            } else {
                None
            }
        } else {
            // Has content: use fill() for word-level line breaking
            // This matches Prettier's splitTextToDocs behavior
            self.build_text_fill_doc_trimmed(raw, false, false, false, false, None)
        }
    }

    /// Build a fill doc for text with separate control over leading/trailing trimming.
    ///
    /// Used by build_nodes_doc_trimmed where first node trims leading, last trims trailing.
    /// When `leading_line` or `trailing_line` is true, the fill uses `line()` at the
    /// boundary instead of wrapping the adjacent expression in a group. This lets fill
    /// continue on the expression's continuation line rather than forcing a newline.
    ///
    /// `glued_prefix` is a doc byte-glued to the run's first word (a
    /// [`Self::glued_comment_run_text`] comment prefix), **fused into the fill's first item** rather
    /// than pushed as a sibling doc. That is what makes the boundary unbreakable *structurally*: a
    /// fill can only break between items, so a prefix inside item 0 can never be split from the word
    /// it is glued to, and the whole unit travels to a fresh line together when it moves. It is
    /// mutually exclusive with a leading boundary space by construction — a glued prefix means the
    /// run starts with no collapsible whitespace, so `leading_line` is false and the `prepend_space`
    /// path below cannot fire.
    fn build_text_fill_doc_trimmed(
        &self,
        raw: &str,
        trim_leading: bool,
        trim_trailing: bool,
        leading_line: bool,
        trailing_line: bool,
        glued_prefix: Option<DocId>,
    ) -> Option<DocId> {
        let d = self.d();
        // Collapsible whitespace only (matching the word split below): a boundary
        // space is emitted only when the split consumed a collapsible-whitespace
        // run. A boundary non-breaking space (U+00A0 / U+202F) stays attached to its
        // word and must not get a spurious regular space prepended/appended.
        let has_leading_ws = !Self::text_glued_before(raw);
        let has_trailing_ws = !Self::text_glued_after(raw);

        // Split on collapsible whitespace only and collect non-empty words, so every
        // non-collapsible separator stays attached to its word and is preserved verbatim:
        // a non-breaking space (U+00A0) / narrow NBSP (U+202F), which Rust's Unicode-aware
        // `split_whitespace` would split on and drop, and a form feed, which its
        // `split_ascii_whitespace` would (prettier's `/[\t\n\f\r ]+/` drops it too).
        let words: SmallVec<[&str; 8]> = split_collapsible_ws(raw).collect();
        if words.is_empty() {
            return None;
        }

        // Fuse the glued prefix into whatever becomes the run's FIRST item — the one place it may
        // go, since a fill breaks only *between* items.
        let fuse_head = |head: DocId| match glued_prefix {
            Some(prefix) => d.concat(&[prefix, head]),
            None => head,
        };

        // Single word: return text (with boundary handling)
        if words.len() == 1 && !leading_line {
            if trailing_line && has_trailing_ws {
                let word = if !trim_leading && has_leading_ws {
                    let mut w = d.pool_writer();
                    w.push(' ');
                    w.push_str(words[0]);
                    w.finish_text()
                } else {
                    d.text_pooled(words[0])
                };
                let parts = [fuse_head(word), d.line()];
                return Some(d.fill(&parts));
            }
            let mut result = d.pool_writer();
            if !trim_leading && has_leading_ws {
                result.push(' ');
            }
            result.push_str(words[0]);
            if !trim_trailing && has_trailing_ws {
                result.push(' ');
            }
            return Some(fuse_head(result.finish_text()));
        }

        // Multiple words (or leading_line): build fill parts
        // leading_line: [line, word, line, word, ...] — text after expression tag
        // trailing_line: [..., word, line] — text before expression tag
        // both: [line, word, line, ..., word, line]
        let prepend_space = !leading_line && !trim_leading && has_leading_ws;
        let append_space = !trim_trailing && has_trailing_ws && !trailing_line;
        let mut parts = d.pooled_docbuf();

        if leading_line {
            parts.push(d.line());
        }

        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                parts.push(d.line());
            }
            let word_doc = if i == 0 && prepend_space {
                let mut w = d.pool_writer();
                w.push(' ');
                w.push_str(word);
                w.finish_text()
            } else if i == words.len() - 1 && append_space {
                let mut w = d.pool_writer();
                w.push_str(word);
                w.push(' ');
                w.finish_text()
            } else {
                d.text_pooled(word)
            };
            // `leading_line` puts a `line` in the first slot instead, and it never coexists with a
            // glued prefix (the run would have to both start with whitespace and not) — so the
            // prefix always lands on word 0, which is the fill's first item.
            parts.push(if i == 0 {
                fuse_head(word_doc)
            } else {
                word_doc
            });
        }

        if trailing_line && has_trailing_ws {
            parts.push(d.line());
        }

        Some(d.fill(&parts))
    }

    //
    // Comment nodes
    //

    /// Build a doc for an HTML comment
    ///
    /// The whole span is verbatim `<!--…-->`, so it emits as one source slice rather than
    /// re-assembling the delimiters around `content_span` — the same rule every comment
    /// emitter in this crate follows (`Printer::js_comment_text_doc` carries the full
    /// rationale; here it is only uniformity, since `<!-- -->` closes at its own delimiter
    /// and so can never swallow).
    pub(crate) fn build_html_comment_doc(&self, comment: &internal::HtmlComment) -> DocId {
        let d = self.d();
        let doc = d.source_span(comment.span, self.source);
        // The renderer records the emit when it reaches the node — see
        // `tsv_lang::comment_ledger`. `<!-- -->` comments register by span in
        // `format_svelte_in`; this is the template (doc) emit path, `print_comment` the
        // hoisted-section (direct-write) one.
        #[cfg(feature = "comment_check")]
        d.tag_comment_doc(doc, comment.span, self.source);
        doc
    }

    //
    // Helper methods
    //
}
