// Text-child handling and word-fill construction for fragment content
//
// `handle_text_child` mirrors prettier-plugin-svelte's handleTextChild: each
// text node's boundary whitespace is resolved against its siblings — with
// `TextChildContext` carrying the caller-side facts — and content words pack
// into fill docs that break greedily at collapsible whitespace. The sibling
// walk in `fragment_doc.rs` constructs the context and dispatches here.

use super::element_doc::MultilineCause;
use super::fragment_doc::text_starts_with_linebreak;
use super::helpers::is_inline_content;
use crate::ast::internal::{
    self, FragmentNode, is_collapsible_ws, is_collapsible_ws_char, split_collapsible_ws,
};
use crate::printer::Printer;
use smallvec::SmallVec;
use tsv_lang::doc::{DocBuf, arena::DocId};

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
pub(super) struct TextChildContext {
    /// Whether the fragment is built on the convergence path (the multiline element arm, the only
    /// caller that routes blocks and control-flow blocks through their own dispatch) — and, when
    /// it is, *why* the layout went multiline. The cause is read by the sibling-newline flow rule
    /// alone; every other site asks only [`MultilineCause::is_multiline`].
    pub(super) cause: MultilineCause,
    /// Whether this node's inline run holds prose — one of the two gates on the sibling-newline
    /// flow rule at its standalone-separator site (`cause` is the other). Computed once per run by
    /// the caller ([`Printer::scan_inline_run`]) and only on the `multiline` path; `false`
    /// everywhere else, which is inert because that site returns before reading it in the
    /// non-multiline arm. Note "inert per arm" is NOT "the arms agree" — the inline arm reaches
    /// the same separator through its own `next_is_tag` case, and the two emitting a different doc
    /// for one logical separator is exactly the period-2 cycle `cause` exists to close.
    pub(super) run_has_prose: bool,
    /// The first and last index in `trimmed_nodes` that the whitespace rules see — the fragment's
    /// content bounds once every HOISTED node is skipped
    /// ([`FragmentNode::content_bounds`]). `handle_text_child`'s `is_first` / `is_last` are
    /// `i <= .0` / `i >= .1` rather than `i == 0` / `i + 1 == len`, so a text with only hoisted
    /// nodes between it and the edge trims its run — the compiler deletes that run, since it lifts
    /// those nodes out before it trims.
    ///
    /// Carried on the context rather than recomputed per child: the question is per-FRAGMENT, and
    /// asking it per node would rescan the sibling list at every text (O(n²) on a long fragment).
    pub(super) content_bounds: (usize, usize),
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
    pub(super) glued_prefix: Option<(DocId, usize)>,
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
    pub(super) prev_sibling_head: usize,
}

impl<'a> Printer<'a> {
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

    /// Handle a text child node - matches prettier-plugin-svelte's handleTextChild.
    ///
    /// Takes `trimmed_nodes` + the node index `i` (the same shape as `handle_block_child`)
    /// and derives every sibling-kind fact internally, rather than receiving them as a long
    /// list of positional bools. `trimmed_nodes[i]` must be a `FragmentNode::Text`.
    pub(super) fn handle_text_child(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
        ctx: TextChildContext,
        child_docs: &mut DocBuf,
        handle_whitespace_of_prev_text: &mut bool,
    ) {
        let TextChildContext {
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
        // The two flow-follower kinds answer every boundary question below identically — the
        // trailing-`line` decision and both halves of `break_before_wide_flow` — so the union is
        // named once. Which member of a welded unit crosses the width cannot matter, and how far
        // the measured unit extends past the follower is the RENDER walk's question alone
        // (`flow_lookahead`), so a build-side split between the two would be a distinction the
        // walk cannot see.
        let next_is_flow_or_tag = next_is_flow || next_is_tag;
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
            // [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
            // Both spellings of a *flowing* separator — an authored space and an authored single
            // newline — must land on the same doc, or the pair diverges (and, once the formatter
            // emits one of them, flip-flops). So the flow test is spelling-independent and the
            // newline arm below simply re-spells itself as the space.
            //
            // The enclosing element need only be multiline AT ALL — `run_has_prose` is this site's
            // whole boundary, and the cause is not a second one.
            //
            // ⚠️ This conjunct read `cause == MultilineCause::Structural` and that was too narrow,
            // in a way only a whole run could show. The flow rule has three call sites and the
            // OTHER two — a content text's leading and trailing runs — carry no cause gate at all,
            // so a `SourceBreaks` container half-applied it: within one run the boundaries touching
            // a text node flowed while the one between two adjacent siblings did not, and
            // `text1 <span>a</span>⏎<span>b</span> text2` came out broken in a line that fits —
            // a form neither formatter produces. A rule keyed on the CONTAINER cannot be right at
            // one of its boundaries and wrong at the next.
            //
            // The narrow gate was written against a period-2 cycle: collapsing the separator was
            // held to delete the very break that chose the multiline arm, so the next pass would
            // take the inline arm, whose `next_is_tag` case emits a bare `line` (all-or-nothing
            // with the already-broken parent group) and splits the run apart again. That does not
            // occur, and the reason is structural rather than lucky — this separator is *interior*
            // to the content, so collapsing it cannot touch the element's own boundary newlines,
            // which are what the multiline decision reads. Both spellings converge, for element
            // and tag siblings alike, and `authoring:audit` is the standing guard.
            // `elements/inline_content_spaced_tags_tail_long` is unaffected: its content is a
            // reflowable fill, so the source-breaks signal is suppressed before it reaches here
            // ([`Self::content_is_reflowable_fill`]) and the gate never opens for it.
            //
            // Widening it makes tsv converge a tag pair onto one line where prettier splits it —
            // a deliberate divergence in the same family as the rest of this rule, pinned by
            // `elements/inline_adjacent_sibling_newline_flow_prettier_divergence`.
            let separator_flows = run_has_prose
                && cause.is_multiline()
                && self.neighbour_newline_flows(prev_node)
                && self.neighbour_newline_flows(next_node);
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
        let leading_newline_flows = self.boundary_newline_flows(
            leading_run.matches('\n').count(),
            separator_like_text,
            prev_node,
        );
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
                // Text after expression/html/render tag: use leading_line in fill instead of
                // wrapping the tag with group([tag, line()]). The group approach forces line()
                // to break after multiline tags, pushing text to a new line. leading_line lets
                // fill continue on the tag's continuation line (line() → space in flat, newline
                // in break).
                //
                // Unconditional — a run holding OTHER break-capable expression tags used to take
                // a plain leading space here instead (the `breakable_exprs` hard-width carve-out),
                // on the theory that a fill `line` renders in fits()-Break mode and short-circuits
                // an earlier expression group's lookahead. That carve-out removed every boundary
                // break point from the run, so the earlier group's measurement ran across the
                // plain spaces into the NEXT breakable tag's head and tore a welded unit that fit
                // at its own position (`fill_multi_expr_travel_long`). The stop at the boundary
                // `line` is the correct answer under the travel doctrine: the boundary itself is
                // measured pairwise against the whole flat unit that follows
                // ([`tsv_lang::doc::DocContext::break_before_wide_flow`]), so a unit that does
                // not fit travels there and nothing is stranded flat past printWidth.
                trim_left = true;
                add_leading_space = false;
                leading_line = true;
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
            } else if !prev_will_break
                && !is_last
                && (child_docs
                    .last()
                    .is_none_or(|&doc| d.strip_leading_line_group(doc).is_none())
                    || prev_node.is_some_and(|n| self.tail_boundary_regenerates_static_break(n))
                    // The wrap keeps the joint join below only when its OTHER side pins its
                    // line. A wrapped element whose far sibling FLOWS (an element or tag — the
                    // ws-only separator's spellings converge, so neither is an outside-in
                    // anchor) takes the same per-width boundary as the unwrapped shapes: the
                    // §129 argument is that the own-line spelling is a fixed point the hugged
                    // form must converge TO, and only a non-flowing neighbour (a comment, a
                    // control-flow block) makes it one.
                    || trimmed_nodes[..prev_sibling_head]
                        .iter()
                        .rev()
                        .find(|n| !n.is_whitespace_only_text())
                        .is_some_and(|n| self.sibling_newline_flows(n)))
            {
                // Non-last text after a NON-breaking element: the same treatment a non-breaking
                // TAG gets — trim and lead the fill with its own `line`, a measured boundary the
                // previous unit's fit walk stops at (the travel doctrine, see the tag arm above),
                // so the tail hugs the closing tag when it fits and breaks per width when it does
                // not. Leaving the raw whitespace instead lets that fit walk run past the closing
                // tag into the following words and tear the element open block-style at a width
                // where the folded form fits (`inline_component_wide_multi_long`'s exactly-100
                // case). Pinned by `inline_wide_element_content_tail_long` (element-child
                // content, incl. the one-pass width-transition claim of its
                // `unformatted_ours_compact`) and `inline_wide_content_text_sibling_long` (prose
                // content); cataloged in conformance_prettier_svelte.md §Svelte: Inline content
                // block-style.
                //
                // The ONE shape that does not take this arm is an element still inside its
                // inline-sibling wrap (`group([line, el])` — a spaced comment or a whitespace-only
                // separator put it there) whose width-broken form would NOT regenerate the static
                // Tier-2 trigger: two boundaries then meet on one element, and the §129 outside-in
                // doctrine resolves the LEADING one first (`inline_sibling_drop_tail_flow_long`) —
                // a greedy per-width tail there keeps the comment hug and re-answers the leading
                // boundary against a column the converged output no longer contains, leaving the
                // document a second stable fixed point. That shape keeps the joint join below.
                //
                // A REGENERATING previous element (a component, or an inline element with
                // non-reflowable-fill content) takes this arm even when wrapped: its width-broken
                // form re-parses as a statically-broken one (the emitted boundary newlines re-read
                // as the Tier-2 signal), and the static path's tail hugs per width — so the
                // width-broken rendering must answer the same or the document converges only on
                // pass 2 (`authoring:audit` catches the mutants). That invariant outranks the
                // outside-in preference, which for these kinds has no stable joint answer to keep.
                trim_left = true;
                add_leading_space = false;
                leading_line = true;
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
                    // Non-last text after an element still inside its inline-sibling wrap, whose
                    // width break does not regenerate (see the arm above for both halves of that
                    // gate): keep the trailing boundary grouped WITH the element, so the two
                    // boundaries meeting on it resolve outside-in — the leading wrap breaks first,
                    // and the joint measurement is what pushes a too-wide-to-sit-flat element's
                    // tail down (`inline_sibling_drop_tail_flow_long`, §129).
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
            // Non-last text after a BREAKING inline element falls through without action,
            // exactly like the breaking-tag case above: the element already carries its hard
            // break, so the text's own fill leading `line` owns the boundary, measured at render
            // from the element's actual end column — it hugs the closing tag when it fits and
            // breaks per width when it does not, converging both authorings (the flow rule
            // respells a single authored newline as the space). This used to be the NON-TERMINAL
            // guard (`group([element, line()])` for every non-tag prev), which forced the
            // boundary to break whenever the element broke; the regenerating-kinds argument for
            // retiring it is on the `leading_line` arm above.
        } else if has_leading_ws && !is_first {
            // ┌─ THE UNCLAIMED-BOUNDARY RULE (this arm is its leading half; the trailing half is
            // │  the last arm of the `trailing_line` chain below, and the two are exact mirrors).
            //
            // A same-line space boundary next to a sibling none of the arms above claims — a
            // comment, or a control-flow block. (An inline element, a tag, a block element and an
            // own-line declaration each have their own arm; the linebreak-authored boundary is
            // arm 2.) `position.next_is_inline()` / `prev_is_inline` are false for these:
            // `is_inline_content` excludes a comment, and the glued-comment-run hop only reports a
            // run ending in an inline ELEMENT.
            //
            // The boundary is inter-node whitespace, so it collapses to one rendered space and a
            // break there is render-equivalent. It must become the fill's OWN `line`, never a space
            // baked into the adjacent word. Baked, it is not a break point at all, and both halves
            // go wrong in their own way:
            // - LEADING: the fresh-line drop carries the space to the head of the continuation line
            //   (`-->⏎\t text1`), which the next pass reads as indentation and drops — no fixed
            //   point.
            // - TRAILING: the space and the following comment ride one item, so the fill can never
            //   break in front of the comment AND the preceding word's fit check is charged the
            //   comment's width — a greedy column lost far under printWidth, and an over-width line
            //   no break can reach when a word and a comment each fit alone but not together
            //   (`fill_break_before_comment_spaced_long`,
            //   `fill_after_comment_glued_midline_long`'s terminal case).
            //
            // ⚠️ NEITHER half is `multiline`-gated, and both were. `multiline` is the CONTAINER's
            // `MultilineCause`, which is `None` for an inline container whose content is
            // collapsible (`<span>`, a table cell) even when width forces the break — the exact
            // confusion `inline_multi_element_pack_long` was about. Gated, each half fired for a
            // block container and silently skipped the inline one, which kept the damage above. The
            // question is per-BOUNDARY, not per-container-class, and a run that fits is a no-op
            // either way: the flag renders Flat as the space it replaced.
            //
            // ⚠️ Nothing else in the gate sees the trailing half. Both layouts are idempotent, keep
            // every comment, and reparse, so F1, the fuzzer, the ledger, the census and the
            // round-trip are all blind; on the widest shape prettier's own output IS the over-width
            // line, so the oracle cannot grade it either. Only a width measurement separates them.
            // The leading half's damage does reach F1 — but only from a shape no fixture had.
            //
            // `leading_line` is the same parity-shifted mechanism the after-a-tag boundary uses.
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
        let trailing_newline_flows =
            self.boundary_newline_flows(trailing_ws_newlines, separator_like_text, next_node);
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
            if is_first || next_is_flow_or_tag {
                // One boundary, one answer: a first child, and a middle child before a tag or
                // before a flowing inline element / component, all end the fill with a trailing
                // `line`, so the boundary breaks per width INSIDE the fill — which is what keeps
                // the run idempotent. A `group([line, node])` here breaks all-or-nothing and
                // flip-flops across passes (the Fill-idempotency bug class).
                //
                // Two conditions that used to split these cases apart are deliberately gone, and
                // both removals are load-bearing:
                // - the `breakable_exprs` hard-width carve-out (a plain trailing space when the
                //   run held another break-capable tag) — see the leading branch;
                // - the `multiline` gate on the flow follower. `multiline` is the CONTAINER's
                //   `MultilineCause`, so a width-broken inline container (span/td — content
                //   collapsible, hence `None`) routed this boundary to the `group([line, element])`
                //   wrap below, and when the element then folded with its terminal trailing text
                //   the group measured the ENTIRE fold flat — element plus every trailing word —
                //   so the run broke before the element far under printWidth
                //   (`inline_multi_element_pack_long` / `…_boundary_long_prettier_divergence`; a
                //   block container at the same width packed pairwise, the
                //   same-source-different-position tell).
                //
                // The boundary is the same pairwise fill question however the container's content
                // came to lay out multiline, and whatever flow node follows it.
                add_trailing_space = false;
                trailing_line = true;
                // A first child's leading boundary is the parent's, already trimmed.
                trim_right = !is_first;
            } else if !is_first {
                // Remaining inline callers: the follower is `is_inline_content` but neither
                // `is_inline_el_or_comp` nor a tag, which leaves exactly a BLOCK element. Wrap it
                // with `group([line, element])`. (Not a comment — see the arm below, which is
                // where a comment follower actually lands.)
                trim_right = true;
                add_trailing_space = false;
                *handle_whitespace_of_prev_text = true;
            }
        } else if has_trailing_ws && !is_last {
            // The trailing half of THE UNCLAIMED-BOUNDARY RULE — see the leading half's comment on
            // the `leading_line` arm above, which states the rule, both failure modes, and why
            // neither half is `multiline`-gated. This arm is its exact mirror.
            trim_right = true;
            add_trailing_space = false;
            trailing_line = true;
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
            // prettier keeps the opening tag on the text line — see conformance_prettier_svelte.md §Svelte:
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
            // A TAG joins on both shapes, unconditionally. Glued, it is the smallest welded unit
            // (`… glued{x}`, the word and its tag travel together) or mid-run glue
            // (`… glued{x}<a…>`): which member of the welded unit crosses the width cannot
            // matter, so the unit is measured through the tag and travels whole. There is no
            // run-ending carve-out: prettier instead keeps a run-ending tag outside the fill and
            // lets it ride past printWidth after the word it is welded to — tsv breaks at the
            // whitespace boundary in front of the word, holding the hard limit (a cataloged
            // divergence — conformance_prettier.md §Print Width Philosophy,
            // fill_glued_tag_travel_long). Spaced, the tag travels alone past the separator: a
            // tag whose expression cannot fit FLAT after the text starts on the fresh line —
            // collapsing flat there when it fits, breaking internally when not — rather than
            // opening mid-line at the end of the text line (the wide-element drop's tag analog;
            // fill_spaced_tag_travel_long, and the fill_expr_travel_* fixtures for what flows
            // after the traveled tag). Prettier's boundary measurement stops at the expression's
            // first internal break, so it opens the tag mid-line — the untruncated Break-mode
            // measurement this flag exists to replace, one follower kind at a time until none
            // was left conditional.
            //
            // Either way an inline element preceded by same-line content that must wrap starts on a
            // fresh line rather than dangling its opening tag at the text line's end (the
            // `inline_break_before_*` divergences) — tsv converges every authoring to that form where
            // prettier keeps the opening tag on the text line (conformance_prettier_svelte.md §Svelte:
            // Inline content block-style). Not `multiline`-gated: a single-line-authored run that
            // must wrap by width still converges to the fresh-line form (a short run that fits is a
            // no-op).
            //
            // The boundary's two shapes, split as the flag's own contract describes them
            // ([`tsv_lang::doc::DocContext::break_before_wide_flow`] carries the render-side
            // mechanics — the whole-flat pairwise measurement and the welded walk):
            let break_before_wide_flow = if has_trailing_ws {
                // SPACED half: the trailing `line` is the separator, and any flow follower —
                // element, component, or tag — couples it to the whole-flat pairwise
                // measurement. Deliberately no follower condition: how far the measured unit
                // extends past the follower is the RENDER walk's question alone
                // (`flow_lookahead` reading `DocArena::welded_entry` off the built docs), so a
                // build-side classification cannot disagree with what the walk sees. A
                // source-keyed follower set here was a two-pass hazard: a glued BLOCK follower
                // detaches by its own layout, so its weld survives only in the source, and
                // pass 2 re-classified the boundary pass 1 had measured.
                trailing_line && next_is_flow_or_tag
            } else {
                // GLUED half: no separator — the boundary in front of the last word is the
                // break point, and ANY tag joins: the welded word+tag pair is the smallest
                // welded unit (conformance_prettier.md §Print Width Philosophy,
                // fill_glued_tag_travel_long), and the render walk extends the measurement
                // through whatever glue actually SURVIVES in the output, stopping at the
                // first non-glued entry.
                next_is_flow_or_tag
            };
            // Both flags are read only off a `Fill` at render, and a run of a SINGLE word is a bare
            // `Text` (`build_text_fill_doc_trimmed`'s early return) — so the run has to be spelled
            // as the one-item fill it is, or the flag reaches no reader (`DocArena::as_fill`). The
            // glued half lands there by construction: it requires `!has_trailing_ws`, which is
            // exactly the arm that returns bare text. A lone `(` heading a run glued to a following
            // element then never got its boundary measured, and the run stood and paid the overflow
            // out of the element's own tag.
            let fill_doc = if break_before_wide_flow || glued_lead {
                d.with_context(
                    d.as_fill(fill_doc),
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
    /// The non-terminal join keeps the trailing boundary grouped *with* the element only for the
    /// shape that still reaches it — an element inside its leading wrap whose width break does
    /// not regenerate (the gate is on `handle_text_child`'s `leading_line` arm): there an element
    /// too wide to sit flat must push its tail to the next line, only measuring the two together
    /// sees that, and the leading wrap stays the boundary that breaks first. Every other
    /// non-terminal tail boundary is the text fill's own `leading_line`, decided per width from
    /// the element's actual end column.
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
    pub(super) fn build_text_doc(&self, text: &internal::Text) -> Option<DocId> {
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
    pub(super) fn build_text_fill_doc_trimmed(
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
}
