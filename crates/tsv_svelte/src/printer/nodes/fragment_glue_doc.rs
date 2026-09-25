// Byte-glue predicates and glued-run construction for fragment content
//
// A glued boundary — no source bytes between siblings — is render-significant:
// breaking it would inject a rendered space, so glued nodes travel as one
// unbreakable unit. This file holds the adjacency predicates and the builders
// that assemble those units (the sibling-`>` dangle, glued element/text/comment
// runs, comment-prefixed units). The sibling walk in `fragment_doc.rs`
// dispatches into these at each glued boundary it meets.

use super::element_doc::PreparedElement;
use super::helpers::is_control_flow_block;
use crate::ast::internal::{self, FragmentNode, is_collapsible_ws_char};
use crate::printer::Printer;
use smallvec::SmallVec;
use tsv_lang::doc::arena::DocId;

/// An inline unit the fragment walk pushes as ONE child doc: a lone element, a glued element run,
/// or either behind a glued comment prefix.
pub(super) struct InlineUnit {
    pub(super) doc: DocId,
    /// The last index the unit covers (its head is where it was built).
    pub(super) end: usize,
    /// Whether the unit's last element was built without its closing `>`, which the glued
    /// control-flow block after it takes ([`Printer::block_sibling_takes_gt`]). The walk must then
    /// emit that `>` with the block, the next node it visits.
    pub(super) sheds_gt: bool,
}

impl<'a> Printer<'a> {
    /// Whether two fragment nodes are **byte-glued** — no source between them (`a`'s end is `b`'s
    /// start). The adjacency test behind the "glued run" *layout* questions (the
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
    pub(super) fn leading_boundary_glued(
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

    /// Axis-3 sibling-`>` dangle: whether the control-flow block right after `nodes[last]` takes
    /// that element's closing `>` — a block directly following (no whitespace) an inline-element
    /// sibling, which the fragment walk will serve through its dangle arm. The element is then
    /// built without its `>` ([`Self::build_inline_unit`]) and the block with it threaded in
    /// ([`Self::build_block_node_doc_with_gt`]), so the `>` dangles onto the block-head line when
    /// the block renders multiline — or when the hugged block-head line would overflow, where the
    /// `>` dangles and the block stays inline on the fresh line. The `>` only moves *into* the
    /// closing tag (`</tag⏎>{#…}`), injecting no render-significant whitespace.
    ///
    /// Asked BEFORE the element is built, at the push of the unit it ends, so the element is
    /// built once, already in the form the block needs. Building it as an ordinary child and
    /// rebuilding it once the block turns up doubled the work at every level of a nest whose
    /// shedders each hold the next (`<b><i>…{#if}…{/if}</i></b>{#if}…`), O(2^depth).
    ///
    /// The dangle keys on the rendered layout, not on how the block's body is authored — so
    /// it is a fixed point on its own output (the dangled form's own-line body would
    /// otherwise read as authored-multiline on a second pass):
    /// - a conditional block (an inline-authored body that may stay inline or expand on
    ///   width) folds the `>` into its own three-way `conditional_group` — hug, dangle +
    ///   inline, dangle + expand (`build_expanding_construct`/`build_expanding_block`);
    /// - a block that unconditionally breaks (authored-multiline / forced) dangles the `>`
    ///   onto its own line (`⏎>` prefix), applied on the non-expanding tails by `dangle_gt`.
    ///
    /// Both happen inside the single `build_block_node_doc_with_gt` build — the block is
    /// built **once**, with the `>` threaded in, so a nested chain of dangles stays linear
    /// (an earlier two-build probe-then-rebuild was O(2^depth) in nesting).
    ///
    /// Applies to the four rendering block heads (`{#if}` / `{#each}` / `{#key}` /
    /// `{#await}`) — and to the one `{#snippet}` shape that still reaches the control-flow
    /// arm, a snippet glued to content on BOTH sides: an own-line snippet takes its line
    /// via [`Self::is_own_line_declaration`] in the arm ahead of the dangle's, so it is excluded
    /// here too (and forces its parent multiline). One caller serves both arms of
    /// `build_nodes_doc_trimmed`: the multiline arm — which a block parent takes once a
    /// control-flow block follows a sibling (`has_control_flow_after_sibling` →
    /// `compute_multiline_cause`) — and the inline arm, which an inline parent (a component, an
    /// inline element) takes for an `{#await}` or a both-sides-glued `{#snippet}`, neither of
    /// which forces it multiline. Either way the block builds in multiline context, and whether
    /// its body drops is width's decision in `build_expanding_construct`, so the dangle is a
    /// one-pass fixed point in both.
    ///
    /// `multiline` is the fragment walk's own arm, which decides whether the own-line
    /// declaration arm comes first. A `true` here is a promise the walk keeps: the block is the
    /// next node it visits, and it reaches the dangle arm, which emits the `>`.
    fn block_sibling_takes_gt(
        &self,
        nodes: &[FragmentNode<'_>],
        last: usize,
        multiline: bool,
    ) -> bool {
        let (Some(prev), Some(block)) = (nodes.get(last), nodes.get(last + 1)) else {
            return false;
        };
        is_control_flow_block(block)
            && matches!(prev, FragmentNode::Element(_))
            // Inline element, directly adjacent (no whitespace between it and the block).
            && !self.is_block_fragment_node(prev)
            && Self::byte_glued(prev, block)
            && !(multiline && self.is_own_line_declaration(nodes, last + 1))
    }

    /// Build the inline child at `nodes[i]` as the fragment walk pushes it: a lone element with
    /// its closing `>` handed to a glued following block when that block takes it
    /// ([`Self::block_sibling_takes_gt`], [`Printer::build_inline_element_omit_close_gt`]),
    /// else the node's ordinary doc. `None` when the node builds nothing.
    pub(super) fn build_inline_unit(
        &self,
        nodes: &[FragmentNode<'_>],
        i: usize,
        multiline: bool,
    ) -> Option<InlineUnit> {
        let node = nodes.get(i)?;
        if let FragmentNode::Element(element) = node
            && self.block_sibling_takes_gt(nodes, i, multiline)
            && let Some(doc) = self.build_inline_element_omit_close_gt(element)
        {
            return Some(InlineUnit {
                doc,
                end: i,
                sheds_gt: true,
            });
        }
        Some(InlineUnit {
            doc: self.build_fragment_node_doc(node)?,
            end: i,
            sheds_gt: false,
        })
    }

    /// The closing-`>` dangle onto glued following TEXT: when the inline element at `i` is
    /// byte-glued to content text on **both** sides — no whitespace either side, so the
    /// break-before rule cannot fire — build it as
    /// [`Printer::build_inline_element_close_gt_dangle`], the three-state group that dangles the
    /// closing `>` onto the following text's line when that fits and block-styles otherwise. The
    /// text-follower analog of the element→element run ([`Self::try_build_glued_element_run`]) and
    /// the element→block dangle ([`Self::block_sibling_takes_gt`]). `None` unless the
    /// glued-both-text shape holds and the element is the flat hug-both (`Soft`) form.
    pub(super) fn try_build_glued_both_text_dangle(
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

    /// The element→element analog of [`Self::block_sibling_takes_gt`] ("G2"), generalized from
    /// a pair to a maximal glued RUN: when `nodes[i]` HEADS a run of 2+ byte-glued inline elements,
    /// build the whole run as one concat (see [`Self::build_glued_element_run`]) — the unit's
    /// `end` is the last index the run covers, so the caller can skip the tail. When a glued
    /// following block takes the run's last `>` ([`Self::block_sibling_takes_gt`]), the run is
    /// built with it split off (`sheds_gt`), once. `None` when `nodes[i]` is not an inline element
    /// or has no glued inline-element follower (the caller handles it as an ordinary inline
    /// child). Detecting at the head and skipping the tail keeps the build O(run length); a
    /// walk-back-and-rebuild at each element would be O(length²).
    pub(super) fn try_build_glued_element_run(
        &self,
        trimmed_nodes: &[FragmentNode<'_>],
        i: usize,
        multiline: bool,
    ) -> Option<InlineUnit> {
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
        // A tail that cannot shed keeps its `>`: `build_glued_element_run` refuses the shed before
        // building any member's children, so the plain build below is the only one that does.
        if self.block_sibling_takes_gt(trimmed_nodes, end, multiline)
            && let Some(doc) = self.build_glued_element_run(trimmed_nodes, i, end, true)
        {
            return Some(InlineUnit {
                doc,
                end,
                sheds_gt: true,
            });
        }
        Some(InlineUnit {
            doc: self.build_glued_element_run(trimmed_nodes, i, end, false)?,
            end,
            sheds_gt: false,
        })
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
    pub(super) fn glued_comment_run_element(
        &self,
        nodes: &[FragmentNode<'_>],
        i: usize,
    ) -> Option<usize> {
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
    /// ([`Self::glued_comment_run_element`]), build the comments + the element as ONE concat —
    /// the unit's `end` is the last index it covers, so the caller skips the tail via
    /// `glued_run_consumed_until`. The comment prefix travels with the element: because the unit is
    /// a plain concat, the preceding text's break-before-flow measurement sees the whole thing flat
    /// (`welded_atom` → `None`), so a wide element pulls its comment prefix to the fresh
    /// line together rather than dangling the opening tag after a space. The element may itself head
    /// a glued-element run (G2) — reuse [`Self::try_build_glued_element_run`] there — else it is an
    /// ordinary inline child ([`Self::build_inline_unit`]); either way its last `>` goes to a glued
    /// following block that takes it (`sheds_gt`). `None` when `nodes[i]` is not a glued-comment
    /// prefix.
    pub(super) fn try_build_glued_comment_prefixed_element(
        &self,
        nodes: &[FragmentNode<'_>],
        i: usize,
        multiline: bool,
    ) -> Option<InlineUnit> {
        let elem_idx = self.glued_comment_run_element(nodes, i)?;
        // Build the element (or the glued-element run it heads), then prepend the comment docs.
        let unit = match self.try_build_glued_element_run(nodes, elem_idx, multiline) {
            Some(unit) => unit,
            None => self.build_inline_unit(nodes, elem_idx, multiline)?,
        };
        let prefix = self.build_glued_comment_run_doc(nodes, i, elem_idx)?;
        Some(InlineUnit {
            doc: self.d().concat(&[prefix, unit.doc]),
            ..unit
        })
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
    pub(super) fn try_build_glued_comment_prefix_for_text(
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
    /// - **per-pair sibling-`>` dangle (G2)**: each adjacent pair of eligible elements hands the
    ///   first's closing `>` to the second (`</span⏎><a⏎…`); the receiver renders it as a leading
    ///   `if_break` inside its attrs group, so it hugs when the attrs fit and dangles when they
    ///   wrap. A mid-run element can both receive (from its left) and shed (to its right).
    ///
    /// Eligibility is per element and the same for both roles
    /// ([`PreparedElement::gt_dangle_eligible`]: `Soft`, or multiline only because of the
    /// element's own authored line breaks). It is computed up front for every element because a
    /// pair's shed decision needs BOTH neighbours — a shed whose receiver turned out ineligible
    /// would strand the `>`. The dangle must not read a signal its own output rewrites: either
    /// role's element prints its content block-style once it does not fit, which the next pass
    /// reads as exactly that authored-newline layout, so a test that turned it away would undo
    /// the pair it just dangled. A structurally multiline element (a block child) stays out.
    ///
    /// Each element is prepared ONCE ([`Printer::prepare_sibling_element`]: tag, attrs, layout),
    /// eligibility is read off that layout, and only the arm actually taken builds the children.
    ///
    /// Against an ineligible neighbour the boundary stays an intact `>` (the element renders its
    /// ordinary doc), so nothing is ever lost. The `>` moves only *inside* a closing tag, so every
    /// reparse is byte-identical — render-safe.
    ///
    /// `shed_last`: the run's LAST element sheds its `>` too — to a glued following control-flow
    /// block, which always receives (the element→block dangle, [`Self::block_sibling_takes_gt`]).
    /// `None` then if that element is not eligible, before any member's children are built: a run
    /// whose tail keeps its `>` is not the unit the caller asked for, and it builds the plain run
    /// instead.
    fn build_glued_element_run(
        &self,
        nodes: &[FragmentNode<'_>],
        start: usize,
        end: usize,
        shed_last: bool,
    ) -> Option<DocId> {
        let d = self.d();
        let mut els: SmallVec<[&internal::Element<'_>; 8]> = SmallVec::new();
        let mut prepared: SmallVec<[Option<PreparedElement<'_>>; 8]> = SmallVec::new();
        for node in &nodes[start..=end] {
            let FragmentNode::Element(el) = node else {
                return None;
            };
            if self.is_block_fragment_node(node) {
                return None;
            }
            prepared.push(self.prepare_sibling_element(el));
            els.push(el);
        }
        let eligible = |i: usize| {
            prepared[i]
                .as_ref()
                .is_some_and(PreparedElement::gt_dangle_eligible)
        };
        let n = els.len();
        if shed_last && !eligible(n - 1) {
            return None;
        }
        let mut parts: SmallVec<[DocId; 8]> = SmallVec::new();
        for idx in 0..n {
            let sheds = if idx + 1 < n {
                eligible(idx) && eligible(idx + 1)
            } else {
                shed_last
            };
            let receives = idx > 0 && eligible(idx - 1) && eligible(idx);
            let doc = match &prepared[idx] {
                // `sheds || receives` implies the element is prepared and eligible: a mid-run
                // shed or a receive reads `eligible(idx)`, and the tail's `shed_last` is guarded
                // above.
                Some(p) if sheds || receives => {
                    let gt = if receives { Some(d.text(">")) } else { None };
                    self.build_gt_dangle_element_doc(els[idx], p, sheds, gt)
                }
                Some(p) => self.build_prepared_element_doc(els[idx], p),
                None => self.build_fragment_node_doc(&nodes[start + idx])?,
            };
            parts.push(doc);
        }
        Some(d.concat(&parts))
    }
}
