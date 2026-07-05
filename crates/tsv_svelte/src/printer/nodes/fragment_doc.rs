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
use crate::printer::text::TextAnalysis;
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
        self.build_nodes_doc_with_context(nodes, false)
    }

    /// Build a doc for nodes with context about text trimming
    ///
    /// # Parameters
    /// - `trim_text`: If true, trim text completely (block context).
    ///   If false, preserve single space at boundaries (inline context).
    pub(crate) fn build_nodes_doc_with_context(
        &self,
        nodes: &[FragmentNode<'_>],
        trim_text: bool,
    ) -> DocId {
        let mut docs: DocBuf = DocBuf::new();
        let mut format_ignore_next = false;
        for (i, node) in nodes.iter().enumerate() {
            // format-ignore: skip whitespace, emit raw source for ignored node
            if format_ignore_next {
                if let Some(raw_doc) = self.format_ignore_raw_doc(node) {
                    docs.push(raw_doc);
                    format_ignore_next = false;
                }
                continue;
            }
            if Self::is_format_ignore_comment(node, self.source) {
                if let Some(doc) = self.build_fragment_node_doc_with_context(node, trim_text) {
                    docs.push(doc);
                }
                format_ignore_next = true;
                continue;
            }

            // For control flow blocks, check if there's preceding breakable content
            let is_control_flow = is_control_flow_block(node);
            let doc = if is_control_flow {
                // "Breakable preceding content" is exactly the inline-content set — text never
                // breaks before a control-flow block, so reuse the one predicate.
                let has_preceding_breakable = nodes[..i].iter().any(is_inline_content);
                self.build_fragment_node_doc_with_preceding_context(
                    node,
                    trim_text,
                    has_preceding_breakable,
                )
            } else {
                self.build_fragment_node_doc_with_context(node, trim_text)
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
    /// # Parameters
    /// - `trim_boundaries`: If true, trim leading ws of first node and trailing ws of last node.
    ///   For block elements: true (boundary whitespace is not semantic).
    ///   For inline elements: false (boundary whitespace is semantic, preserve it).
    /// - `breakable_exprs`: If true, boundary text adjacent to expression/html/render tags is
    ///   emitted as plain spaces instead of `fill` `line`s. Set when the fragment has a
    ///   break-capable expression tag (the hug-both hard-width divergence): a `line` in
    ///   fits()-Break mode short-circuits a preceding expression group's width check, stranding
    ///   it flat and overshooting printWidth (`fill_multiple_expr_long`). Plain spaces keep the
    ///   expression group's full `fits()` obligation so it breaks instead. The existing
    ///   (non-hug-both) callers pass `false` to stay byte-identical.
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
        trim_boundaries: bool,
        breakable_exprs: bool,
        multiline: bool,
    ) -> DocId {
        let d = self.d();
        if nodes.is_empty() {
            return d.empty();
        }

        // Find boundary indices based on trim_boundaries setting:
        // - Block elements (trim_boundaries=true): skip whitespace-only text at boundaries
        // - Inline elements (trim_boundaries=false): keep whitespace (normalize to space in handle_text_child)
        //
        // Helper: should we skip this node at the boundary?
        let source = self.source;
        let should_skip_at_boundary = |n: &FragmentNode<'_>| -> bool {
            if let FragmentNode::Text(text) = n {
                // Whitespace-only: skip only for block elements
                // Inline elements keep boundary whitespace (normalized to single space)
                // ASCII whitespace only — a non-breaking space (U+00A0) is content, not a
                // collapsible boundary, so an NBSP-only node is never skipped.
                text.is_ascii_ws_only && trim_boundaries
            } else {
                false // Not text, don't skip
            }
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
        for (i, node) in trimmed_nodes.iter().enumerate() {
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

            if matches!(node, FragmentNode::Text(_)) {
                self.handle_text_child(
                    trimmed_nodes,
                    i,
                    trim_boundaries,
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
                // currently parked on a corpus parity gap — see TODO_REFACTORING §#1 Slice 2).
                self.handle_block_child(
                    trimmed_nodes,
                    i,
                    force_break,
                    &mut child_docs,
                    &mut handle_whitespace_of_prev_text,
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
                    handle_whitespace_of_prev_text = false;
                } else {
                    // No dangle. A block the root marked as part of a SINGLE-LINE inline run builds
                    // in inline context (its long body inner-breaks rather than dropping to its own
                    // line — `is_root_inline_run_block`); every other block builds with
                    // `in_multiline_context=true`, which is what lets a wrapped head
                    // (`{#if a || b || …}`) break its condition and dangle the `}` (the block-tag
                    // wrapping work). The non-multiline callers keep the inline
                    // `build_fragment_node_doc_*` path below.
                    let node_doc = if self.is_root_inline_run_block(node) {
                        let has_preceding_breakable =
                            trimmed_nodes[..i].iter().any(is_inline_content);
                        self.build_fragment_node_doc_with_preceding_context(
                            node,
                            false,
                            has_preceding_breakable,
                        )
                    } else {
                        self.build_fragment_node_doc_in_multiline(node, true)
                    };
                    if let Some(node_doc) = node_doc {
                        child_docs.push(node_doc);
                    }
                    handle_whitespace_of_prev_text = false;
                }
            } else if is_inline_content(node) {
                self.handle_inline_child(
                    node,
                    &mut child_docs,
                    &mut handle_whitespace_of_prev_text,
                );
            } else {
                // Other nodes (comments, `{@const}`/`{@debug}`/`{const}`/`{let}` tags).
                // Check if there's preceding breakable content (expression tags or elements)
                // This affects whether block conditions should use remove_lines() or not:
                // - With preceding breakable content: use remove_lines() so that content breaks first
                // - Without preceding breakable content: allow wrapping to respect print_width
                let has_preceding_breakable = trimmed_nodes[..i].iter().any(is_inline_content);
                if let Some(node_doc) = self.build_fragment_node_doc_with_preceding_context(
                    node,
                    false,
                    has_preceding_breakable,
                ) {
                    child_docs.push(node_doc);
                }
                handle_whitespace_of_prev_text = false;
            }
        }

        // `concat` short-circuits the empty case to `empty()`.
        d.concat(&child_docs)
    }

    /// Check if a node is an expression-like tag (ExpressionTag, HtmlTag, RenderTag).
    ///
    /// These tags use the leading/trailing line fill approach instead of group wrapping,
    /// because group wrapping forces line breaks after multiline expressions.
    fn is_expression_tag(node: &FragmentNode<'_>) -> bool {
        matches!(
            node,
            FragmentNode::ExpressionTag(_) | FragmentNode::HtmlTag(_) | FragmentNode::RenderTag(_)
        )
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
        Some(self.d().source_span(node.span(), self.source))
    }

    /// Handle a text child node - matches prettier-plugin-svelte's handleTextChild.
    ///
    /// Takes `trimmed_nodes` + the node index `i` (the same shape as `handle_block_child`)
    /// and derives every sibling-kind fact internally, rather than receiving them as a long
    /// list of positional bools. `trimmed_nodes[i]` must be a `FragmentNode::Text`.
    #[allow(clippy::too_many_arguments)]
    fn handle_text_child(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
        trim_boundaries: bool,
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
        let prev_is_tag = prev_node.is_some_and(Self::is_expression_tag);
        let next_is_inline = next_node.is_some_and(is_inline_content);
        let next_is_tag = next_node.is_some_and(Self::is_expression_tag);
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
        let next_is_flow = next_node.is_some_and(|n| self.is_inline_el_or_comp(n));
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
            // Whitespace-only text node.
            if (is_first || is_last) && !trim_boundaries {
                // Boundary whitespace in an inline element. For single-line content the parent's
                // boundary break collapses to nothing, so the child supplies the single space
                // (`<span> text` / `<span>\n\ttext` → `<span> text`). For multiline content the
                // parent's leading/trailing break supplies the line itself, so emitting a space
                // here would strand it before the first child / after the last.
                if !multiline {
                    child_docs.push(d.text(" "));
                }
                return;
            }
            if !multiline {
                // Legacy inline callers: signal the next inline element to lead with a line.
                *handle_whitespace_of_prev_text = true;
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

        // Determine what whitespace to trim
        // For block elements: always trim first/last boundaries
        // For inline elements: preserve space-only boundaries, normalize newline boundaries to space
        let has_leading_space_only = raw.has_leading_space_only();
        let has_trailing_space_only = raw.has_trailing_space_only();

        // Track if we need to add a space to replace trimmed newline whitespace (inline only)
        let mut add_leading_space = false;
        let mut add_trailing_space = false;

        let mut trim_left = if is_first {
            if trim_boundaries {
                true // Block: always trim
            } else if has_leading_space_only {
                false // Inline with space-only: preserve
            } else if has_leading_ws {
                // Inline with newline: trim. For single-line inline content the boundary
                // collapses to a space (`<span>\ntext` → `<span> text`); for multiline content
                // the parent element's leading break already supplies the line (prettier's
                // `splitTextToDocs` startsWithLinebreak → hardline), so adding a space here
                // would strand it before the first word.
                add_leading_space = !multiline;
                true
            } else {
                false // No leading whitespace
            }
        } else {
            false
        };

        let mut trim_right = if is_last {
            if trim_boundaries {
                true // Block: always trim
            } else if has_trailing_space_only {
                false // Inline with space-only: preserve
            } else if has_trailing_ws {
                // Inline with newline: trim. Single-line → collapse to a space; multiline → the
                // parent's trailing break supplies the line (see the leading case above).
                add_trailing_space = !multiline;
                true
            } else {
                false // No trailing whitespace
            }
        } else {
            false
        };

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
                        // after it. `sandwiched` = there is content before the element (it can be
                        // pushed onto its own line by a preceding break); when it actually drops,
                        // the trailing text wraps to its own line rather than hugging the dropped
                        // element's `>` — see `build_after_element_fold`.
                        let sandwiched = !child_docs.is_empty();
                        child_docs
                            .push(self.build_after_element_fold(last_doc, raw, false, sandwiched));
                        return;
                    }
                    // Non-last (text between two inline elements): keep the existing
                    // group-wrapped boundary. The fold isn't needed here (a following element
                    // supplies the next break point) and changing it has no failing fixture —
                    // revisit fixtures-first if a mid-run wide element ever needs it.
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
            // Scoped to `!is_first`, mirroring prettier-plugin-svelte's `handleTextChild`: only a
            // MIDDLE text node trims its trailing whitespace and lets the following inline element be
            // wrapped in a droppable `group([line, element])`. A FIRST-child text leaves the element
            // bare, so it hugs and overflows (the sanctioned `inline_closing_text` shape). Matching
            // that split keeps the first-child hug cases unchanged while the in-flow boundaries drop.
            let fill_doc = if trailing_line && next_is_flow && !is_first {
                d.with_context(
                    fill_doc,
                    tsv_lang::doc::DocContext {
                        break_before_wide_flow: true,
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
        handle_whitespace_of_prev_text: &mut bool,
    ) {
        let d = self.d();
        if let Some(node_doc) = self.build_fragment_node_doc_with_context(node, false) {
            if *handle_whitespace_of_prev_text {
                // Previous text had trailing whitespace - wrap element with leading line
                let line = d.line();
                let inner = d.concat(&[line, node_doc]);
                child_docs.push(d.group(inner));
            } else {
                child_docs.push(node_doc);
            }
        }
        *handle_whitespace_of_prev_text = false;
    }

    /// Whether a node is a block-level *element* — the `handleBlockChild` set in
    /// prettier-plugin-svelte (`isBlockElement`): an HTML block element, a block special
    /// element, or a block component. Excludes control-flow blocks (`{#if}` etc. — they
    /// separate via the whitespace-break path) and inline elements/components.
    fn is_block_element_node(&self, node: &FragmentNode<'_>) -> bool {
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
    ///   `handle_whitespace_of_prev_text` flag) or trimmed away (no longer ends with ws).
    /// - **after** when the next sibling exists and is either a non-text node, or content
    ///   text (or an empty text immediately followed by an inline element) that does **not**
    ///   start with a linebreak — a leading-linebreak text supplies its own break.
    fn handle_block_child(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
        force_break: bool,
        child_docs: &mut DocBuf,
        handle_whitespace_of_prev_text: &mut bool,
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
                    *handle_whitespace_of_prev_text
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

        if let Some(node_doc) = self.build_fragment_node_doc_with_context(&trimmed_nodes[i], false)
        {
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

        *handle_whitespace_of_prev_text = false;
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
        self.build_nodes_doc_trimmed(nodes, true, breakable_exprs, true)
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

    /// Whether the fragment has 2+ whitespace-separated `{expr}` siblings — a multiline-*entry*
    /// signal consumed only by `compute_needs_multiline` (a `<Comp>` authored with a leading break
    /// and a non-hugged trailing boundary goes multiline so its exprs break). Sibling *separation*
    /// once multiline is decided is handled per-pair by the whitespace-space arm of
    /// [`Self::build_nodes_doc_multiline`] (break before every non-inline-element sibling); this
    /// predicate no longer drives layout directly.
    ///
    /// Returns true when:
    /// - There are 2+ expression tags
    /// - There's whitespace-only text BETWEEN expressions (layout whitespace)
    ///
    /// Returns false when:
    /// - Single expression
    /// - Expressions directly adjacent (no whitespace between)
    /// - Semantic text between expressions (e.g., `{'<'}div{'>'}`)
    pub(super) fn should_split_expressions_in_nodes(&self, nodes: &[FragmentNode<'_>]) -> bool {
        // Count expression nodes
        let expr_count = nodes
            .iter()
            .filter(|n| matches!(n, FragmentNode::ExpressionTag(_)))
            .count();

        if expr_count < 2 {
            return false;
        }

        // Find first and last expression indices
        let first_expr = nodes
            .iter()
            .position(|n| matches!(n, FragmentNode::ExpressionTag(_)));
        let last_expr = nodes
            .iter()
            .rposition(|n| matches!(n, FragmentNode::ExpressionTag(_)));

        match (first_expr, last_expr) {
            (Some(first), Some(last)) if first < last => {
                // Check if there's whitespace-only text between expressions
                // The decision to split is controlled by the outer condition
                // (source_has_leading_break && has_trailing_whitespace)
                nodes[first..=last]
                    .iter()
                    .any(FragmentNode::is_whitespace_only_text)
            }
            _ => false,
        }
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

    /// Build a doc for a single fragment node with text trimming context
    ///
    /// Returns None for whitespace-only text nodes that should be skipped.
    fn build_fragment_node_doc_with_context(
        &self,
        node: &FragmentNode<'_>,
        trim_text: bool,
    ) -> Option<DocId> {
        self.build_fragment_node_doc_impl(node, trim_text, false, false)
    }

    /// Build a fragment node doc with multiline context awareness.
    ///
    /// When `in_multiline_context` is true, blocks with symmetric spaces
    /// (spaces but no newlines) will expand to multiline format.
    fn build_fragment_node_doc_in_multiline(
        &self,
        node: &FragmentNode<'_>,
        trim_text: bool,
    ) -> Option<DocId> {
        self.build_fragment_node_doc_impl(node, trim_text, true, false)
    }

    /// Build a fragment node doc with preceding content context.
    ///
    /// When `has_preceding_breakable` is true, block conditions will use remove_lines()
    /// to ensure earlier content breaks before the condition.
    fn build_fragment_node_doc_with_preceding_context(
        &self,
        node: &FragmentNode<'_>,
        trim_text: bool,
        has_preceding_breakable: bool,
    ) -> Option<DocId> {
        self.build_fragment_node_doc_impl(node, trim_text, false, has_preceding_breakable)
    }

    fn build_fragment_node_doc_impl(
        &self,
        node: &FragmentNode<'_>,
        trim_text: bool,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
    ) -> Option<DocId> {
        match node {
            FragmentNode::Text(text) => self.build_text_doc(text, trim_text),
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
        if self.is_block_fragment_node(prev) || prev.span().end != block.span().start {
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
            FragmentNode::SnippetBlock(b) => self.build_snippet_block_doc_with_full_context(
                b,
                in_multiline_context,
                has_preceding_breakable,
                gt_prefix,
            ),
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

    /// Split `s` into `[word, line, word, …]` fill parts (ASCII-whitespace separated,
    /// matching `build_text_fill_doc_trimmed`'s word split, so non-breaking spaces stay
    /// attached). Used by the inline-element fold so the words after a folded element pack
    /// greedily into the surrounding fill rather than moving as one nested unit.
    fn word_fill_parts(&self, s: &str) -> DocBuf {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        for word in s.split_ascii_whitespace() {
            if !parts.is_empty() {
                parts.push(d.line());
            }
            parts.push(d.text_pooled(word));
        }
        parts
    }

    /// Build the after-element fold doc: one `fill([element, line, word …])` so the element's
    /// closing `>` stays intact while the words pack greedily after it, plus (when
    /// `trailing_line` — the next sibling is itself a flowing inline element/component) a
    /// trailing `line` so the boundary to that next child can break. A wide element whose
    /// content overflows wraps within print width and dangles its closing `>` on a low column;
    /// the trailing text then packs after it. Used by the inline/trimmed text path
    /// ([`Self::handle_text_child`]) when an inline element is the last child before trailing text.
    ///
    /// `sandwiched` (the element has a preceding sibling, so a preceding break can push it onto its
    /// own line) sets [`DocContext::break_after_dropped_first`]: when the element actually drops to
    /// its own line (renders at line start) the trailing text wraps to the next line instead of
    /// hugging the dropped element's `>` — a wide inline child owns its line, regardless of whether
    /// the drop came from the element's own content wrapping or from the preceding text being too
    /// long. A first-child element (`!sandwiched`) can't drop via a preceding sibling, so the
    /// trailing text packs after it normally.
    fn build_after_element_fold(
        &self,
        prev: DocId,
        raw: &str,
        trailing_line: bool,
        sandwiched: bool,
    ) -> DocId {
        let d = self.d();
        let words = self.word_fill_parts(raw);
        let mut parts = d.pooled_docbuf();
        parts.push(prev);
        parts.push(d.line());
        parts.extend(words);
        if trailing_line {
            parts.push(d.line());
        }
        let fill = d.fill(&parts);
        // `hug_wide_first` is always set: the fold's first item is the inline element, and when it
        // sits mid-line right after a parent element's `>` and is too wide for its own line, it must
        // hug-and-break-internally rather than drop (which would strand a spurious `>⏎<child` break —
        // the nested-`<span>` non-idempotency). `break_after_dropped_first` couples the *trailing*
        // text to the drop, and only applies when the element is sandwiched (a preceding sibling can
        // push it onto its own line); the two flags address opposite ends of the fold.
        d.with_context(
            fill,
            tsv_lang::doc::DocContext {
                hug_wide_first: true,
                break_after_dropped_first: sandwiched,
                // Terminal trailing text after a wide element hugs the dangled `>` (respecting the
                // author's space boundary); non-terminal text (`trailing_line`) keeps its own line.
                hug_terminal_after_break: !trailing_line,
                ..Default::default()
            },
        )
    }

    /// Build a doc for a text node
    ///
    /// Returns None for whitespace-only text that should be skipped.
    /// For text with content, normalizes internal whitespace to single spaces.
    ///
    /// # Parameters
    /// - `trim_completely`: If true, trim leading/trailing whitespace (block context).
    ///   If false, preserve single space at boundaries (inline context).
    fn build_text_doc(&self, text: &internal::Text, trim_completely: bool) -> Option<DocId> {
        let raw = text.raw(self.source);
        // ASCII (collapsible) whitespace only: a non-breaking space (U+00A0) is content,
        // so a node made only of NBSP is NOT empty here and flows to the fill path below
        // (preserved verbatim), never dropped or collapsed to a regular space.
        let trimmed = raw.trim_ascii();
        if trimmed.is_empty() {
            // Pure (ASCII) whitespace: collapse to single space only in inline context
            if !trim_completely && raw.bytes().any(|b| b.is_ascii_whitespace()) {
                Some(self.d().text(" "))
            } else {
                None
            }
        } else {
            // Has content: use fill() for word-level line breaking
            // This matches Prettier's splitTextToDocs behavior
            self.build_text_fill_doc(raw, trim_completely)
        }
    }

    /// Build a fill doc for text content, enabling word-level line breaking.
    ///
    /// Splits text on whitespace into words, joining with line() docs.
    /// This allows fill() to break at word boundaries when lines exceed width.
    fn build_text_fill_doc(&self, raw: &str, trim_completely: bool) -> Option<DocId> {
        self.build_text_fill_doc_trimmed(raw, trim_completely, trim_completely, false, false)
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
        d.concat(&[
            d.text("<!--"),
            d.source_span(comment.content_span, self.source),
            d.text("-->"),
        ])
    }

    //
    // Helper methods
    //
}
