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

use super::helpers::{is_control_flow_block, is_inline_content};
use crate::ast::internal::{self, Fragment, FragmentNode};
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

/// Whether `raw` begins with a linebreak, ignoring leading horizontal whitespace — matches
/// prettier-plugin-svelte's `startsWithLinebreak` (`^([\t\f\r ]*\n)`). Used by the block-child
/// boundary logic to tell a leading-linebreak text (which supplies its own break) from
/// content/space text (which needs a `softline`).
fn text_starts_with_linebreak(raw: &str) -> bool {
    raw.trim_start_matches([' ', '\t', '\x0c', '\r'])
        .starts_with('\n')
}

impl<'a> Printer<'a> {
    /// Build a doc for an entire fragment (sequence of nodes)
    ///
    /// This is the entry point for doc-based inline content formatting.
    /// The resulting doc includes all nodes, so fits() checks will
    /// naturally account for siblings.
    pub(crate) fn build_fragment_doc(&self, fragment: &Fragment<'_>) -> DocId {
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
    /// - `multiline`: the convergence mode — set only by the element multiline arm
    ///   (`compute_needs_multiline`). It turns on the ported prettier-plugin-svelte printChildren
    ///   handling that the legacy inline callers don't need (and would be churned by): block
    ///   children via `handle_block_child` + `forceBreakContent`; `printWhitespace` (a
    ///   whitespace-only text at a non-HTML-element boundary becomes a hardline/blank/bare-line);
    ///   the `splitTextToDocs` leading-linebreak rule (content text with a leading newline emits a
    ///   hardline rather than folding into the prev element); and the first/last whitespace-only
    ///   boundary deferring to the parent's leading/trailing break (emit nothing) instead of the
    ///   inline single space. The legacy callers pass `false` and stay byte-identical. (Path 1,
    ///   `build_nodes_doc_multiline`, still serves block bodies / root / special elements — its
    ///   reroute onto this path + deletion is the remaining Slice-2/3 work.)
    pub(crate) fn build_nodes_doc_trimmed(
        &self,
        nodes: &[FragmentNode<'_>],
        breakable_exprs: bool,
        multiline: bool,
    ) -> DocId {
        let d = self.d();
        if nodes.is_empty() {
            return d.empty();
        }

        // Skip whitespace-only text nodes at the fragment boundaries (ASCII whitespace only —
        // a non-breaking space (U+00A0) is content, not a collapsible boundary, so an
        // NBSP-only node is never skipped).
        let source = self.source;
        let should_skip_at_boundary = |n: &FragmentNode<'_>| -> bool {
            matches!(n, FragmentNode::Text(text) if text.is_ascii_ws_only)
        };

        let Some((start_idx, end_idx)) = Self::trimmed_node_bounds(nodes, should_skip_at_boundary)
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
        let force_break = multiline
            && trimmed_len > 1
            && trimmed_nodes.iter().any(|n| self.is_block_element_node(n));

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
        for (i, node) in trimmed_nodes.iter().enumerate() {
            if i > 0 && is_inline_content(&trimmed_nodes[i - 1]) {
                has_preceding_breakable = true;
            }
            // Tail of a glued element run already built at its head — skip (its doc is in place).
            if i < glued_run_consumed_until {
                continue;
            }
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
                && matches!(node, FragmentNode::Text(t) if t.is_ascii_ws_only)
                && matches!(&trimmed_nodes[i - 1], FragmentNode::Text(p) if p.is_ascii_ws_only)
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
                    breakable_exprs,
                    multiline,
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
            } else if multiline && is_control_flow_block(node) {
                // Control-flow block (`{#if}`/`{#each}`/`{#await}`/`{#key}`/`{#snippet}`) in the
                // convergence path. Mirror path 1's block dispatch.
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
                // Axis-3 sibling-`>` dangle onto glued following TEXT: an inline element byte-glued
                // to text on both sides (no whitespace either side, so break-before can't fire)
                // dangles its closing `>` onto the following text's line when that fits, else
                // block-styles — the text-follower analog of the element→element run and the
                // element→block dangle. Checked before the element-run (disjoint: this needs a
                // following TEXT, the run a following element).
                if let Some(dangle_doc) = self.try_build_glued_both_text_dangle(trimmed_nodes, i) {
                    self.push_inline_child_doc(&mut child_docs, dangle_doc, prev_text_ws);
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
                    self.push_inline_child_doc(&mut child_docs, run_doc, prev_text_ws);
                    glued_run_consumed_until = run_end + 1;
                } else {
                    self.handle_inline_child(node, &mut child_docs, prev_text_ws);
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
                self.push_inline_child_doc(&mut child_docs, unit_doc, prev_text_ws);
                glued_run_consumed_until = run_end + 1;
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
    /// start). The core adjacency test behind every "glued run" in this file: a glued boundary is
    /// render-significant (breaking it would inject a rendered space), so a glued prefix or element
    /// run always travels as one unit. Any node — including whitespace-only text — between them
    /// makes them non-adjacent.
    fn byte_glued(a: &FragmentNode<'_>, b: &FragmentNode<'_>) -> bool {
        a.span().end == b.span().start
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
            && text.is_ascii_ws_only
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
        breakable_exprs: bool,
        multiline: bool,
        child_docs: &mut DocBuf,
        handle_whitespace_of_prev_text: &mut bool,
    ) {
        let FragmentNode::Text(text) = &trimmed_nodes[i] else {
            return;
        };
        let raw: &str = text.raw(self.source);

        // Sibling-kind facts, derived from the node's position in `trimmed_nodes`.
        let is_first = i == 0;
        let is_last = i + 1 == trimmed_nodes.len();
        let prev_node = i.checked_sub(1).map(|j| &trimmed_nodes[j]);
        let next_node = trimmed_nodes.get(i + 1);
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
        // Whether the next sibling is a flowing inline element OR component — the path-1
        // `next_node_is_flow` set (the Fill-idempotency boundary). Text before such a node
        // ends its fill with a trailing `line` so the boundary breaks per width inside the
        // fill (keeping the run idempotent), rather than a `group([line, node])` whose
        // all-or-nothing break flip-flops across passes.
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

        // ASCII whitespace class `[\t\n\f\r ]`, matching prettier-plugin-svelte's
        // text split (`splitTextToDocs`). A leading/trailing non-breaking space (or
        // any non-ASCII whitespace) is content, so a node made only of those is not
        // whitespace-only and is preserved verbatim.
        let has_leading_ws = raw.starts_with(|c: char| c.is_ascii_whitespace());
        let has_trailing_ws = raw.ends_with(|c: char| c.is_ascii_whitespace());

        if text.is_ascii_ws_only {
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
            let newline_count = text.newline_count as usize;
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
        if multiline && text_starts_with_linebreak(raw) && !is_first {
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
            let content_start = raw.len()
                - raw
                    .trim_start_matches(|c: char| c.is_ascii_whitespace())
                    .len();
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
                // Note: non-last text after a breaking tag (prev_is_tag && !is_last
                // && prev_will_break) also falls through without action — group()
                // would force line() to break, and leading_line is only for
                // non-breaking continuation. The text's leading ws handles spacing.
            } else if !prev_will_break {
                trim_left = true;
                add_leading_space = false; // line() handles the space
                // Pop the last doc (the inline element) and rejoin it with the trailing text.
                if let Some(last_doc) = child_docs.pop() {
                    if is_last {
                        // Last child: fold the element and the trailing words into ONE fill so a
                        // wide element wraps its own content within printWidth and the words pack
                        // after it — see `build_after_element_fold`.
                        //
                        // If the popped element is `handle_inline_child`'s `group([line, X])`
                        // inline-sibling wrap (an element preceded by an inline sibling across a
                        // bare space), keep X bare in the fold's lead content slot and hoist the
                        // boundary line OUTSIDE the fold, reusing the standalone `group([line, …])`
                        // shape. Folding the *line* into the lead double-counts the boundary: the
                        // fill breaks before the fold AND the wrapping group re-renders its own
                        // leading line flat → a stray leading space, non-idempotent
                        // (`inline_break_before_prev_inline_long`).
                        let folded = match d.strip_leading_line_group(last_doc) {
                            Some(inner) => {
                                let fold = self.build_after_element_fold(inner, raw);
                                d.group(d.concat(&[d.line(), fold]))
                            }
                            None => self.build_after_element_fold(last_doc, raw),
                        };
                        child_docs.push(folded);
                        return;
                    }
                    // Non-last (text between two inline elements): keep the group-wrapped boundary.
                    // The following element supplies the next break point, and folding the middle
                    // text into the element (packing it onto the dangled `>` line) is non-convergent
                    // — it shifts where the following element lands, flip-flopping across passes.
                    // Pinned by `inline_wide_content_text_sibling_long`.
                    let line = d.line();
                    let inner = d.concat(&[last_doc, line]);
                    child_docs.push(d.group(inner));
                }
            }
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
            let content_end = raw
                .trim_end_matches(|c: char| c.is_ascii_whitespace())
                .len();
            raw[content_end..].matches('\n').count()
        } else {
            0
        };
        let mut trailing_hardlines = 0usize;
        if multiline && trailing_ws_newlines >= 1 && !is_last {
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
                // inside the fill — matching path 1's `next_node_is_flow` boundary, which keeps the
                // run idempotent. A `group([line, node])` here breaks all-or-nothing and flip-flops
                // across passes (the Fill-idempotency bug class).
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
            // (it never reaches this text handler; `hug_wide_first` still guards its idempotency).
            let fill_doc = if next_is_flow && (trailing_line || !has_trailing_ws) {
                // One `break_before_wide_flow` boundary rule, both authored shapes (the render side
                // routes each to the right fill case by parity — see the flag's doc):
                // - **space-separated** (`… word <a…>`, `trailing_line`): the trailing `line` is the
                //   Case-2 separator; measuring the following element/run flat breaks it so a wide
                //   element drops to its own line whole rather than packing onto the text line.
                // - **glued** (`… glued<a…>`, `!has_trailing_ws`, no separator): the glued word is
                //   the Case-1 last item; the same flat measurement breaks at the whitespace boundary
                //   BEFORE the glued word so the whole glued run moves to a fresh line together,
                //   never splitting the glued boundary (which would inject a rendered space).
                //
                // Either way an inline element preceded by same-line content that must wrap starts on
                // a fresh line rather than dangling its opening tag at the text line's end (the
                // `inline_break_before_*` divergences) — tsv converges every authoring to that form
                // where prettier keeps the opening tag on the text line (conformance_prettier.md
                // §Svelte: Inline content block-style). Coupled whether the preceding text is a first
                // or middle child; a first-child element with NO preceding text is unaffected (it
                // never reaches this handler; `hug_wide_first` still guards its idempotency). Not
                // `multiline`-gated: a single-line-authored run that must wrap by width still
                // converges to the fresh-line form (a short run that fits is a no-op).
                d.with_context(
                    fill_doc,
                    tsv_lang::doc::DocContext {
                        break_before_wide_flow: true,
                        ..Default::default()
                    },
                )
            } else if next_is_tag && !has_trailing_ws {
                // The text's last word is welded to a following tag with no whitespace
                // (`… tsv is ~{ratio}`). prettier keeps the tag outside the fill, so the fill never
                // breaks before that word and the tag rides past printWidth after it. Measure the
                // last word alone so tsv matches — otherwise the glued tag folds into the word's fit
                // check and strands it on its own line. (A tag is never `next_is_flow`, so this is
                // disjoint from the flow branch above.)
                d.with_context(
                    fill_doc,
                    tsv_lang::doc::DocContext {
                        trailing_glued_tag: true,
                        ..Default::default()
                    },
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

    /// Handle an inline child element - matches prettier-plugin-svelte's handleInlineChild
    fn handle_inline_child(
        &self,
        node: &FragmentNode<'_>,
        child_docs: &mut DocBuf,
        prev_text_ws: bool,
    ) {
        if let Some(node_doc) = self.build_fragment_node_doc(node) {
            self.push_inline_child_doc(child_docs, node_doc, prev_text_ws);
        }
    }

    /// Push an already-built inline child doc, honoring a pending trimmed-boundary space
    /// from the previous text node. When `prev_text_ws` is set (the prev text trimmed a
    /// space-only boundary and deferred the separator to the next sibling — prettier's
    /// `handleWhitespaceOfPrevTextNode`), lead with a collapsible `line` inside a group: a
    /// space when the fill fits, a break when it wraps.
    ///
    /// Shared by the single-element path (`handle_inline_child`) and the glued-element-run
    /// path in `build_nodes_doc`, so a trimmed boundary space is never dropped before a
    /// byte-glued run (`</span>` ` ` `<br/><br/>`) — the single-sibling case already worked
    /// because a run of one falls through to `handle_inline_child`. The caller snapshots and
    /// clears the flag (`prev_text_ws`), so this only reads it.
    fn push_inline_child_doc(&self, child_docs: &mut DocBuf, node_doc: DocId, prev_text_ws: bool) {
        if prev_text_ws {
            // The single producer of the inline-sibling wrap; `DocArena::strip_leading_line_group`
            // (the after-element fold's matcher, a crate away) is its exact inverse. Routing through
            // the named constructor keeps the two in lockstep — a shape drift here would silently
            // return `None` there and reintroduce the stray-space non-idempotency.
            child_docs.push(self.d().inline_sibling_line_group(node_doc));
        } else {
            child_docs.push(node_doc);
        }
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
        let prev = i.checked_sub(1).map(|j| &trimmed_nodes[j]);
        let next = trimmed_nodes.get(i + 1);

        let break_before = match prev {
            Some(p) if !self.is_block_element_node(p) => match p {
                FragmentNode::Text(t) => {
                    prev_text_ws
                        || !t
                            .raw(self.source)
                            .ends_with(|c: char| c.is_ascii_whitespace())
                }
                _ => true,
            },
            _ => false,
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
                let is_empty_ws = t.is_ascii_ws_only;
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
        self.build_nodes_doc_trimmed(nodes, breakable_exprs, true)
    }

    /// Build the content of a **whitespace-collapsing container** (`<table>`, `<select>`, … —
    /// `tsv_html::collapses_child_whitespace`) block-style: every non-whitespace child on its own
    /// line, with the inter-sibling whitespace **trimmed**. Svelte's compiler removes that
    /// whitespace entirely (`clean_nodes` `can_remove_entirely`), so this is render-equivalent to
    /// the inline form and reproduces the block-authored form both formatters already keep — see
    /// [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
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
        for node in nodes {
            // Trim inter-sibling whitespace — render-free in this container — but remember an
            // authored blank line so the next child reintroduces it.
            if let FragmentNode::Text(t) = node
                && t.is_ascii_ws_only
            {
                if t.newline_count >= 2 {
                    pending_blank = true;
                }
                continue;
            }
            let node_doc = if format_ignore_next {
                format_ignore_next = false;
                self.format_ignore_raw_doc(node)
            } else {
                if Self::is_format_ignore_comment(node, self.source) {
                    format_ignore_next = true;
                }
                self.build_fragment_node_doc_in_multiline(node)
            };
            if let Some(node_doc) = node_doc {
                if !parts.is_empty() {
                    parts.push(d.hardline());
                    if pending_blank {
                        parts.push(d.hardline());
                    }
                }
                pending_blank = false;
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
    pub(super) fn fragment_should_force_break_content(&self, nodes: &[FragmentNode<'_>]) -> bool {
        let non_ws_count = nodes
            .iter()
            .filter(|n| !n.is_whitespace_only_text())
            .count();
        non_ws_count > 1 && nodes.iter().any(|n| self.is_block_fragment_node(n))
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
    /// Applies to all five block heads (`{#if}` / `{#each}` / `{#key}` / `{#await}` /
    /// `{#snippet}`). A control-flow block with any preceding sibling routes its block
    /// parent through the multiline-fragment layout (`has_control_flow_after_sibling` →
    /// `compute_needs_multiline`), so the block's body-drop keys on `can_wrap` (true here)
    /// and the dangle is a one-pass fixed point — including for `{#await}` / `{#snippet}`,
    /// whose body-drop is likewise gated on `can_wrap`.
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
        // glued-after check below — `is_ascii_ws_only` excludes an empty / whitespace-only prev text
        // (which carries no content the element could be glued *to*).
        let prev = nodes.get(i.checked_sub(1)?)?;
        let FragmentNode::Text(pt) = prev else {
            return None;
        };
        if pt.is_ascii_ws_only
            || !Self::byte_glued(prev, node)
            || pt
                .raw(self.source)
                .ends_with(|c: char| c.is_ascii_whitespace())
        {
            return None;
        }
        // glued-after: the next node is content text byte-glued with no leading whitespace (so the
        // dangled `>` leads that text's line; a leading space would wrap at the space instead).
        let next = nodes.get(i + 1)?;
        let FragmentNode::Text(nt) = next else {
            return None;
        };
        if nt.is_ascii_ws_only
            || !Self::byte_glued(node, next)
            || nt
                .raw(self.source)
                .starts_with(|c: char| c.is_ascii_whitespace())
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

    /// If `nodes[i]` **begins** a byte-glued run of one or more HTML comments that ends glued to an
    /// inline element/component, return that element's index. Every comment in the run must be
    /// byte-adjacent to the next node and the last one byte-adjacent to the element
    /// (`<!--a--><!--b--><a…>`); any whitespace inside the run stops it (`None`), as does a run
    /// glued to a non-inline node, and a **format-ignore directive** anywhere in the run (see below).
    /// Whitespace *before* `nodes[i]` is the boundary the break lands on, exactly as for a glued
    /// text prefix — but a *glued comment* before `nodes[i]` makes it a non-head member of a longer
    /// run, and only the head opens the unit (`None` otherwise). The comment run is the element's
    /// glued prefix; the break-before machinery then measures comments + element as one unit (see
    /// [`Self::try_build_glued_comment_prefixed_element`] and [`Self::handle_text_child`]'s
    /// `comment_glued_next_flow`).
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
    fn glued_comment_run_element(&self, nodes: &[FragmentNode<'_>], i: usize) -> Option<usize> {
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
                _ if self.is_inline_el_or_comp(next) => return Some(j + 1),
                _ => return None,
            }
        }
    }

    /// When `nodes[i]` heads a glued HTML-comment run ending in an inline element
    /// ([`Self::glued_comment_run_element`]), build the comments + the element as ONE concat and
    /// return `(unit_doc, end)` — the last index the unit covers, so the caller skips the tail via
    /// `glued_run_consumed_until`. The comment prefix travels with the element: because the unit is
    /// a plain concat, the preceding text's break-before-flow measurement sees the whole thing flat
    /// (`after_element_fold_lead` → `None`), so a wide element pulls its comment prefix to the fresh
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
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        for node in &nodes[i..elem_idx] {
            parts.push(self.build_fragment_node_doc(node)?);
        }
        parts.push(elem_doc);
        Some((d.concat(&parts), end))
    }

    /// Build a maximal run of byte-adjacent (glued) inline **elements** — `nodes[start..=end]`,
    /// all plain non-block `Element`s (`None` if any isn't) — as ONE concat. Two effects, both the
    /// point of the "run travels together" posture:
    ///
    /// - **break-before as a unit**: the preceding text's break-before-flow measurement measures
    ///   this whole concat flat (`after_element_fold_lead` returns `None` for a plain concat → the
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
    /// the first / after the last) directly into `parts` — no intermediate buffer. ASCII-whitespace
    /// separated, matching `build_text_fill_doc_trimmed`'s word split (so non-breaking spaces stay
    /// attached). Used by the inline-element fold so the words after a folded element pack
    /// greedily into the surrounding fill rather than moving as one nested unit.
    fn extend_with_word_fill(&self, parts: &mut DocBuf, s: &str) {
        let d = self.d();
        let mut first = true;
        for word in s.split_ascii_whitespace() {
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
    /// dangles its `>` and the terminal tail hugs it (`hug_terminal_after_break`).
    fn build_after_element_fold(&self, prev: DocId, raw: &str) -> DocId {
        let d = self.d();
        let mut parts = d.pooled_docbuf();
        parts.push(prev);
        parts.push(d.line());
        self.extend_with_word_fill(&mut parts, raw);
        let fill = d.fill(&parts);
        // `hug_wide_first` is always set: the fold's first item is the inline element, and when it
        // sits mid-line right after a parent element's `>` and is too wide for its own line, it must
        // hug-and-break-internally rather than drop (which would strand a spurious `>⏎<child` break —
        // the nested-`<span>` non-idempotency).
        d.with_context(
            fill,
            tsv_lang::doc::DocContext {
                hug_wide_first: true,
                // The fold only ever runs for terminal trailing text, which hugs the dangled `>`
                // (respecting the author's space boundary).
                hug_terminal_after_break: true,
                ..Default::default()
            },
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
        let trimmed = raw.trim_ascii();
        if trimmed.is_empty() {
            // Pure (ASCII) whitespace: collapse to a single inter-sibling space
            if raw.bytes().any(|b| b.is_ascii_whitespace()) {
                Some(self.d().text(" "))
            } else {
                None
            }
        } else {
            // Has content: use fill() for word-level line breaking
            // This matches Prettier's splitTextToDocs behavior
            self.build_text_fill_doc_trimmed(raw, false, false, false, false)
        }
    }

    /// Build a fill doc for text with separate control over leading/trailing trimming.
    ///
    /// Used by build_nodes_doc_trimmed where first node trims leading, last trims trailing.
    /// When `leading_line` or `trailing_line` is true, the fill uses `line()` at the
    /// boundary instead of wrapping the adjacent expression in a group. This lets fill
    /// continue on the expression's continuation line rather than forcing a newline.
    fn build_text_fill_doc_trimmed(
        &self,
        raw: &str,
        trim_leading: bool,
        trim_trailing: bool,
        leading_line: bool,
        trailing_line: bool,
    ) -> Option<DocId> {
        let d = self.d();
        // ASCII whitespace only (matching the word split below): a boundary space
        // is emitted only when the split consumed an ASCII-whitespace run. A
        // boundary non-breaking space (U+00A0 / U+202F) stays attached to its word
        // and must not get a spurious regular space prepended/appended.
        let has_leading_ws = raw.starts_with(|c: char| c.is_ascii_whitespace());
        let has_trailing_ws = raw.ends_with(|c: char| c.is_ascii_whitespace());

        // Split on ASCII whitespace only and collect non-empty words. Prettier's
        // splitTextToDocs splits on `/[\t\n\f\r ]+/`, so non-breaking spaces
        // (U+00A0) and narrow non-breaking spaces (U+202F) stay attached to their
        // words — they are not break points and are preserved verbatim. Rust's
        // `split_whitespace` is Unicode-aware and would split (and thus drop) them.
        let words: SmallVec<[&str; 8]> = raw.split_ascii_whitespace().collect();
        if words.is_empty() {
            return None;
        }

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
                let parts = [word, d.line()];
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
            return Some(result.finish_text());
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
            if i == 0 && prepend_space {
                let mut w = d.pool_writer();
                w.push(' ');
                w.push_str(word);
                parts.push(w.finish_text());
            } else if i == words.len() - 1 && append_space {
                let mut w = d.pool_writer();
                w.push_str(word);
                w.push(' ');
                parts.push(w.finish_text());
            } else {
                parts.push(d.text_pooled(word));
            }
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
    pub(crate) fn build_html_comment_doc(&self, comment: &internal::HtmlComment) -> DocId {
        let d = self.d();
        let doc = d.concat(&[
            d.text("<!--"),
            d.source_span(comment.content_span, self.source),
            d.text("-->"),
        ]);
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
