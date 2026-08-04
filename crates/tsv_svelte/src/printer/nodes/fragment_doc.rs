// Doc-based formatting for inline fragment content
//
// Builds Doc IR trees for fragment nodes, enabling proper fits() checks
// that account for siblings. This matches Prettier's architecture where
// the entire inline content is represented as a single doc tree.
//
// Entered through the `build_nodes_doc_*` family (and the element/block/root doc
// builders that call them) to format fragment content with correct attribute
// wrapping decisions that consider what comes after each element.
//
// The byte-glue predicates and glued-run builders live in `fragment_glue_doc.rs`;
// text-child handling and word-fill construction in `fragment_text_doc.rs`.

use super::element_doc::MultilineCause;
use super::fragment_text_doc::TextChildContext;
use super::helpers::{is_control_flow_block, is_inline_content};
use crate::ast::internal::{self, Fragment, FragmentNode};
use crate::printer::Printer;
use tsv_lang::doc::{DocBuf, arena::DocId};
use tsv_lang::is_format_ignore_directive;

/// The treatment of an inline child doc's LEADING boundary, decided at the unit's head — the
/// argument to [`Printer::push_inline_child_doc`]. Three mutually exclusive cases, and the
/// exclusivity is structural: a previous text that trimmed a boundary space cannot also be glued
/// ([`Printer::text_glued_after`] fails on a whitespace tail), so no caller ever holds two at once.
#[derive(Clone, Copy)]
pub(super) enum LeadBoundary {
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
///
/// [`is_collapsible_ws`]: crate::ast::internal::is_collapsible_ws
pub(super) fn text_starts_with_linebreak(raw: &str) -> bool {
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
    /// it — see conformance_prettier_svelte.md §Svelte: Inline content block-style.
    ///
    /// # Parameters
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
    /// See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
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
    pub(super) fn is_format_ignore_comment(node: &FragmentNode<'_>, source: &str) -> bool {
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
    /// [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
    pub(super) fn sibling_newline_flows(&self, node: &FragmentNode<'_>) -> bool {
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
    pub(super) fn is_separator_like_text(data: &str) -> bool {
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
    /// root-inline-run dispatch, and the sibling-`>` dangle).
    pub(crate) fn build_nodes_doc_multiline(&self, nodes: &[FragmentNode<'_>]) -> DocId {
        // `Structural`: these callers are the root fragment, block bodies, and special elements —
        // none of them has an enclosing element whose multiline-ness the content's own newlines
        // could flip, so the sibling-newline flow rule stays in force here.
        self.build_nodes_doc_trimmed(nodes, MultilineCause::Structural)
    }

    /// Build the content of a **whitespace-collapsing container** (`<table>`, `<select>`, … —
    /// `tsv_html::collapses_child_whitespace`) block-style: every non-whitespace child on its own
    /// line, with the inter-sibling whitespace **trimmed**. Svelte's compiler removes that
    /// whitespace entirely (`clean_nodes` `can_remove_entirely`), so this is render-equivalent to
    /// the inline form and reproduces the block-authored form both formatters already keep — see
    /// [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
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
    pub(super) fn is_block_fragment_node(&self, node: &FragmentNode<'_>) -> bool {
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
    pub(super) fn next_is_inline_element(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
    ) -> bool {
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
    pub(super) fn is_inline_el_or_comp(&self, node: &FragmentNode<'_>) -> bool {
        matches!(
            node,
            FragmentNode::Element(_) | FragmentNode::SpecialElement(_)
        ) && !self.is_block_fragment_node(node)
    }

    /// Build a doc for a single fragment node.
    ///
    /// Returns None for whitespace-only text nodes that should be skipped.
    pub(super) fn build_fragment_node_doc(&self, node: &FragmentNode<'_>) -> Option<DocId> {
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
    pub(super) fn build_block_node_doc_with_gt(
        &self,
        node: &FragmentNode<'_>,
        gt: DocId,
    ) -> Option<DocId> {
        self.build_control_flow_block_doc(node, true, false, Some(gt))
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
