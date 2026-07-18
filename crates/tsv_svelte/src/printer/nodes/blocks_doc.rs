// Doc builders for Svelte control-flow blocks
//
// {#if}/{:else if}/{:else}, {#each}, {#await}, {#key}, and {#snippet} —
// opening/closing tag layout, branch flattening, and section bodies.

// Allow Svelte block syntax like `{:else}`, `{:then}`, `{:catch}` which
// look like Rust format args but are valid Svelte template syntax.
#![allow(clippy::literal_string_with_formatting_args)]

use crate::ast::internal::{self, Fragment, FragmentNode};
use crate::printer::Printer;
use smallvec::smallvec;
use tsv_lang::comments_to_emit_in_range;
use tsv_lang::doc::arena::DocId;
use tsv_lang::doc::{DocBuf, GroupId};

use super::helpers::{each_expr_comment_end, indent_body};

// Opening-tag literals for control-flow blocks. Every offset that locates the
// embedded expression past the opening tag derives from `.len()` of these, so
// the emitted text and the scan offset cannot drift apart. Shared with the
// inline / whitespace-sensitive builders in `element_doc.rs`.
pub(crate) const IF_BLOCK_OPEN: &str = "{#if ";
pub(crate) const ELSE_IF_BLOCK_OPEN: &str = "{:else if ";
pub(crate) const EACH_BLOCK_OPEN: &str = "{#each ";
pub(crate) const AWAIT_BLOCK_OPEN: &str = "{#await ";
pub(crate) const KEY_BLOCK_OPEN: &str = "{#key ";

/// One built, **mode-agnostic** piece of an if-tail (consequent body, a
/// `{:else if}` head + body, or the `{:else}` body). The bodies and heads are
/// identical whether the tail renders inline or expanded — only the indent /
/// hardline wrapping differs — so they are built **once** and composed into both
/// forms by `compose_if_tail`. Building a full doc per form instead would rebuild
/// every nested body once per form, compounding to O(2^depth) on nested blocks (the
/// build-fanout audit guards against that).
enum IfPiece {
    Consequent(DocId),
    ElseIf { head: DocId, body: DocId },
    Else(DocId),
}

/// The pre-built, **mode-agnostic** pieces of an await tail (each present section's
/// body + the un-shorthanded `{:then}` / `{:catch}` keyword), built once and composed
/// into both expanding-construct tails by `compose_await_tail` — so a nested section
/// body is built once, not once per form (building a doc per form would rebuild each
/// section body twice, compounding to O(2^depth)).
struct AwaitPieces {
    pending: Option<DocId>,
    then_kw: Option<DocId>,
    then_body: Option<DocId>,
    catch_kw: Option<DocId>,
    catch_body: Option<DocId>,
}

/// Build one `{#await}` section body (`pending` / `then` / `catch`).
///
/// `expand` is the construct-wide boundary decision (see `Printer::body_boundaries_break`):
/// every section's boundaries break together, so a section authored inline still drops to
/// its own line once any sibling section went multiline. Keying it per-section on that
/// section's own authored whitespace would let a render-free character weld one section's
/// body to its keyword while the others break.
fn build_await_section_body(printer: &Printer<'_>, fragment: &Fragment<'_>, expand: bool) -> DocId {
    let body_doc = if expand {
        printer.build_nodes_doc_multiline(fragment.nodes)
    } else {
        printer.build_fragment_doc(fragment)
    };
    indent_body(printer, body_doc, expand)
}

/// Build `indent([line, body_doc])` for space-only await blocks.
///
/// In flat mode (fits): ` body_doc` (space + content)
/// In break mode (exceeds print width): newline + indent + body_doc
fn indent_body_soft(printer: &Printer<'_>, body_doc: DocId) -> DocId {
    let line = printer.d().line();
    let inner = printer.d().concat(&[line, body_doc]);
    printer.d().indent(inner)
}

/// Split a raw parameter string at top-level commas, returning trimmed param strings.
///
/// Handles nesting for `()`, `[]`, `{}`, `<>`, and string literals (`'...'`, `"..."`).
/// E.g., `"a: A | 'x', b: B<C, D>"` → `["a: A | 'x'", "b: B<C, D>"]`.
fn split_raw_params_at_commas(raw: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
            }
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth -= 1,
            b',' if depth == 0 => {
                result.push(raw[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    let last = raw[start..].trim();
    if !last.is_empty() {
        result.push(last);
    }
    result
}

/// Whether a wrapped block head's clause + `}` should hug the expression's last
/// line (`) as item}`, `)}`) rather than dropping to their own line.
///
/// True only for a single call/`new` whose callee contains no other call: when its
/// arguments wrap, the `)` dedents to the tag's base indent, so the rendered last
/// line starts with `)` and (per the layout rule) must not be broken — the clause +
/// `}` continue on it. A binary/logical chain (last line is an operand), a
/// multi-segment member chain (last line is a `.method(...)` segment, not a bare
/// `)`), or anything else breaks the clause + `}` to their own line at base.
fn clause_hugs_expr(expr: &tsv_ts::Expression<'_>) -> bool {
    use tsv_ts::Expression as E;
    // A callee with no nested call means the only `(` belongs to this call, so its
    // `)` lands at the tag base when the args wrap (vs. a chain, whose segments indent).
    fn callee_has_no_call(e: &E<'_>) -> bool {
        match e {
            E::Identifier(_) | E::ThisExpression(_) | E::Super(_) => true,
            E::MemberExpression(m) => callee_has_no_call(m.object),
            _ => false,
        }
    }
    match expr {
        E::CallExpression(c) => callee_has_no_call(c.callee),
        E::NewExpression(n) => callee_has_no_call(n.callee),
        _ => false,
    }
}

/// How an `{#await}` carries its first section in the head, keyed on the **absence of a
/// pending body** (the binding is optional). Single classification shared by the head-clause
/// builder and the tail builders (which skip the head-carried keyword) — so the two can't drift.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AwaitShorthand {
    /// `{#await x then v}` / bare `{#await x then}` — no pending body, a `then` section.
    Then,
    /// `{#await x catch e}` / bare `{#await x catch}` — no pending, no `then`, a `catch` section.
    Catch,
    /// Full form (`{#await x}…{:then}…{:catch}…`) — the head carries no section clause.
    None,
}

/// Whether an await block's `:then` section carries a printable body. An **empty-body** `:then`
/// (`{#await p} {:then v}{/await}`, or the shorthand `{#await p then v}{/await}`) is dropped
/// entirely — marker and binding — matching prettier, since the `value` binding is unused when
/// nothing renders. (A `:catch` is *not* dropped when empty: an empty `{:catch}` still handles a
/// rejection, so removing it would change semantics — see `conformance_prettier.md` §Svelte: Blocks.)
fn then_has_content(block: &internal::AwaitBlock<'_>) -> bool {
    block
        .then
        .as_ref()
        .is_some_and(|t| t.nodes.iter().any(|n| !n.is_whitespace_only_text()))
}

/// Classify an await block's head shorthand. See [`AwaitShorthand`].
///
/// A pending fragment that is empty **or space-only** (whitespace with no newline) carries no
/// body, so — exactly like an absent pending — the first **surviving** section folds into the
/// head shorthand (`then v` / `catch e`). This is what makes a space-only pending
/// (`{#await p} {:then v}{/await}`) converge to the same fixed point as a truly-empty one instead
/// of lingering as an un-folded full form. A **newline**-authored empty pending is left un-folded
/// (its `is_boundary_break` node counts as a body) — it keeps the full multiline form.
///
/// An empty-body `:then` is not a survivor (it is dropped, see [`then_has_content`]), so the fold
/// skips it to the `:catch`.
fn await_shorthand(block: &internal::AwaitBlock<'_>) -> AwaitShorthand {
    let has_pending_body = block.pending.as_ref().is_some_and(|p| {
        p.nodes
            .iter()
            .any(|n| !n.is_whitespace_only_text() || n.is_boundary_break())
    });
    if has_pending_body {
        AwaitShorthand::None
    } else if then_has_content(block) {
        AwaitShorthand::Then
    } else if block.error.is_some() || block.catch.is_some() {
        AwaitShorthand::Catch
    } else {
        AwaitShorthand::None
    }
}

impl<'a> Printer<'a> {
    /// Whether the trailing comments in `[start, end)` end with a line (`//`) comment.
    ///
    /// A trailing line comment is emitted (by `build_trailing_js_comment_doc`) with a
    /// closing `hardline` — a `//` runs to end of line, so the following clause + `}`
    /// already drop to the next line. The head dangle (and the flat hug) must then not
    /// add their own break, or a spurious blank line / leading space appears. A trailing
    /// *block* comment carries no `hardline`, so it does not suppress the dangle.
    ///
    /// **to emit**, deliberately: the question is whether the comment *this gap prints last*
    /// is a line comment, because that emission is what carries the closing `hardline`. A
    /// comment the gap does not print carries no hardline, so it must not answer this.
    pub(super) fn head_trailing_line_comment(&self, start: u32, end: u32) -> bool {
        comments_to_emit_in_range(self.comments, start, end)
            .last()
            .is_some_and(|c| !c.is_block)
    }

    /// Whether a wrapped block head may dangle its `}` here. The head expression is
    /// allowed to break (`allow_wrapping` or a multiline context) AND the context permits
    /// the dangle — false only inside a whitespace-significant element (`<pre>` /
    /// `<textarea>`), gated by [`Printer::block_dangle_allowed`]. Gating it off only hugs
    /// the `}`; the expression still wraps to respect printWidth either way.
    fn block_head_can_wrap(&self, allow_wrapping: bool, in_multiline_context: bool) -> bool {
        (allow_wrapping || in_multiline_context) && self.block_dangle_allowed()
    }

    /// Wrap a block-tag head so its closing `}` dangles on its own line when the head wraps.
    ///
    /// `open` is the opening literal (`{#if `, `{#each `, …) and `head_inner` is everything
    /// between it and the closing `}` — the breakable expression plus any clause (` as item`,
    /// ` then value`). When `can_wrap` is set (the same condition under which the expression is
    /// allowed to break), `[head_inner, softline]` is grouped so the trailing softline — and thus
    /// `}` — drops to its own line at the tag's base indent whenever the head exceeds print width
    /// (a deliberate `_prettier_divergence`, consistent with tsv's JS `if (⏎…⏎) {` and broken-element
    /// `>`). When the head fits, the softline collapses and `}` hugs the head, byte-identical to the
    /// inline form. When `can_wrap` is false (inline context, `remove_lines` applied) the head is
    /// emitted flat with `}` hugged, unchanged.
    ///
    /// `expr_ends_with_line_comment` (from `head_trailing_line_comment`) short-circuits both
    /// paths: the comment's own `hardline` already drops the clause + `}` to the next line, so
    /// the dangle/hug break is skipped to avoid a spurious blank line.
    pub(super) fn build_block_head_doc(
        &self,
        open: &'static str,
        expr_doc: DocId,
        clause: Option<DocId>,
        can_wrap: bool,
        hug: bool,
        expr_ends_with_line_comment: bool,
    ) -> DocId {
        let d = self.d();
        let open_doc = d.text(open);
        let close = d.text("}");
        if expr_ends_with_line_comment {
            // The trailing line comment already emitted a `hardline` that drops the
            // clause + `}` to the next line at base indent. Emit them directly with no
            // dangle/hug break. Still group the expression on the wrapping path so the
            // body-expand keyed to `BlockHead` sees the (comment-forced) break.
            let head = if can_wrap {
                d.group_with_id(expr_doc, GroupId::BlockHead)
            } else {
                expr_doc
            };
            return match clause {
                Some(c) => d.concat(&[open_doc, head, c, close]),
                None => d.concat(&[open_doc, head, close]),
            };
        }
        if can_wrap {
            // Key the breakable expression to `GroupId::BlockHead`. The head group's
            // `fits()` counts the trailing clause + `}` (they sit in rest-commands /
            // resolve flat during the fits test), so the head breaks at the right
            // boundary; reading anything keyed to `BlockHead` immediately after the
            // group resolves keeps the shared id nesting-safe.
            let grouped = d.group_with_id(expr_doc, GroupId::BlockHead);
            if hug {
                // The expression renders ending with `)` on its own line at base (a
                // single call whose args wrapped). Per the layout rule, don't break a
                // line that starts with `)` — the clause + `}` continue on it
                // (`) as item}`, `)}`), in both the flat and broken head.
                match clause {
                    Some(c) => {
                        let space = d.text(" ");
                        d.concat(&[open_doc, grouped, space, c, close])
                    }
                    None => d.concat(&[open_doc, grouped, close]),
                }
            } else {
                // Binary chain / member chain / etc.: the clause + `}` drop to their
                // own line at the tag's base indent when the head wraps (`expr⏎as item}`,
                // `expr⏎}`); when it fits they hug inline (`expr as item}`).
                let hardline = d.hardline();
                let (break_tail, flat_tail) = match clause {
                    Some(c) => {
                        let space = d.text(" ");
                        (d.concat(&[hardline, c]), d.concat(&[space, c]))
                    }
                    None => (hardline, d.empty()),
                };
                let dangle = d.if_break_with_id(break_tail, flat_tail, GroupId::BlockHead);
                d.concat(&[open_doc, grouped, dangle, close])
            }
        } else {
            match clause {
                Some(c) => {
                    let space = d.text(" ");
                    d.concat(&[open_doc, expr_doc, space, c, close])
                }
                None => d.concat(&[open_doc, expr_doc, close]),
            }
        }
    }

    /// The shared block-head tail every block builder ends with: detect whether the
    /// head expression's trailing comments (over `[expr.end, comment_end)`) end with a
    /// line comment, then build the head doc via [`Printer::build_block_head_doc`].
    ///
    /// `expr` is the head expression — used for its span and the `clause_hugs_expr`
    /// classification; `expr_doc` is the already-built expression doc (the `{#each}`
    /// degenerate index/key form passes a concat of the expression plus its tail here,
    /// so the two are distinct). `clause` is the optional ` as …` / ` then …` / ` catch …`
    /// tail (without leading space), and `comment_end` bounds the trailing-comment scan
    /// (the head end for `{#if}`/`{:else if}`/`{#key}`, or the pattern-start-narrowed
    /// end for `{#each}`/`{#await}`). `can_wrap` stays caller-computed — its sources
    /// differ across builders and several reuse it afterward for the body-drop.
    fn build_block_head(
        &self,
        open: &'static str,
        expr: &tsv_ts::Expression<'_>,
        expr_doc: DocId,
        clause: Option<DocId>,
        comment_end: u32,
        can_wrap: bool,
    ) -> DocId {
        let elc = self.head_trailing_line_comment(expr.span().end, comment_end);
        self.build_block_head_doc(
            open,
            expr_doc,
            clause,
            can_wrap,
            clause_hugs_expr(expr),
            elc,
        )
    }

    /// Build a **section-free** block (key, plain each/if/await, snippet) whose body
    /// is inline-authored, expanding the body + `{/tag}` onto their own lines when the
    /// head goes multiline.
    ///
    /// `head_doc` is the opening tag through its `}` (including the `BlockHead`
    /// head-wrap group + dangle); `body_doc` / `close` are the inline body and the
    /// `{/tag}` close.
    ///
    /// A `conditional_group` chooses in one pass among (1) fully inline, (2) flat head +
    /// expanded body (the construct overflows but the head fits alone — the expanded
    /// body's leading `hardline` ends the head's `fits()` lookahead, so the head
    /// measures *head-alone*), and (3) wrapped head + expanded body (the head alone
    /// overflows, so it wraps and its `}` dangles). Decoupling head-wrap (head-alone
    /// width) from body-expand (head+body width) keeps every layout a one-pass fixed
    /// point: a one-line input in the "middle zone" (head fits alone, head+body
    /// doesn't) converges directly to layout 2 instead of wrapping then un-wrapping
    /// across two passes. This holds for **every** body shape — text, expression,
    /// void, and element/component — which all drop to their own line on overflow.
    fn build_expanding_block(
        &self,
        head_doc: DocId,
        body_doc: DocId,
        close: DocId,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        let lead = d.hardline();
        let trail = d.hardline();
        let multiline_tail = d.concat(&[d.indent(d.concat(&[lead, body_doc])), trail, close]);
        // Inline tail keeps the body's own `indent()` wrapper (so a body that breaks
        // internally still indents); the close hugs the body.
        let inline_tail = d.concat(&[d.indent(body_doc), close]);
        self.build_expanding_construct(head_doc, inline_tail, multiline_tail, gt_prefix)
    }

    /// Prepend a split-off preceding sibling's closing `>` (`gt_prefix`) to a block
    /// candidate: hugged in the inline candidate (`>{#…}`) and dangled onto its own line
    /// in a multiline candidate (`⏎>{#…}`), so the `>` tracks the block's own
    /// inline-vs-multiline choice. `None` leaves the candidate untouched. See the axis-3
    /// sibling-`>` dangle in `build_inline_element_omit_close_gt`.
    fn fold_gt(&self, gt_prefix: Option<DocId>, dangle: bool, body: DocId) -> DocId {
        let d = self.d();
        match gt_prefix {
            Some(gt) if dangle => d.concat(&[d.hardline(), gt, body]),
            Some(gt) => d.concat(&[gt, body]),
            None => body,
        }
    }

    /// Post-build placement of a preceding sibling's split-off `>` (`gt_prefix`) on a block
    /// builder's **non-expanding** return paths — the tails that don't thread `gt_prefix`
    /// through `build_expanding_construct`/`build_expanding_block` (authored-multiline bodies,
    /// `{#await}`'s newline/empty tail). The `>` must track whether the block **renders**
    /// inline or multiline — hug when inline (`>{#…}`), dangle onto its own line when
    /// multiline (`⏎>{#…}`) — and must never be dropped. Three cases by how `doc` breaks:
    ///
    /// - **force-break** (`will_break`: a `hardline` / propagated `breakParent`) → the block is
    ///   unconditionally multiline, so dangle statically.
    /// - **can break at render but not forced** (`can_break`, e.g. a short empty block whose
    ///   only break point is a **long head** that width-wraps) → the inline-vs-multiline choice
    ///   is a *render-time* decision `will_break` can't see, so fold the `>` with `if_break` in
    ///   an enclosing `group`: hug when the group fits, dangle when it breaks. Placing it
    ///   statically here would hug a `>` whose block then wraps — reparse-safe but
    ///   **non-idempotent** (the wrap reflows on the next pass).
    /// - **cannot break** (no line at all, e.g. an empty `{#await p}{/await}` with a short head)
    ///   → always inline, so hug statically.
    ///
    /// `None` (every non-dangle caller) is a no-op. Distinct from the space-only tail, which
    /// needs its `group` **unconditionally** (its section separators break under it) and so
    /// folds the `>` at its own return rather than here.
    fn dangle_gt(&self, gt_prefix: Option<DocId>, doc: DocId) -> DocId {
        let d = self.d();
        match gt_prefix {
            None => doc,
            Some(gt) if d.will_break(doc) => d.concat(&[d.hardline(), gt, doc]),
            Some(gt) if d.can_break(doc) => {
                let folded = d.if_break(d.concat(&[d.hardline(), gt]), gt);
                d.group(d.concat(&[folded, doc]))
            }
            Some(gt) => d.concat(&[gt, doc]),
        }
    }

    /// Core of the expand-when-the-construct-overflows layout, over a precomputed
    /// `inline_tail` (everything after the head's `}` hugged onto one line) and
    /// `multiline_tail` (the same content with each body/section/branch on its own
    /// line). Shared by the section-free blocks (via `build_expanding_block`),
    /// `{#if}`/`{#each}` with `{:else}`/`{:else if}` alternates, and `{#await}`
    /// (multiple sections, via `compose_await_tail`).
    ///
    /// A `conditional_group` picks fully-inline / flat-head + expanded / wrapped-head +
    /// expanded in one pass, decoupling head-wrap (head-alone width) from body-expand
    /// (head+tail width) so every layout is a one-pass fixed point (no two-pass
    /// wrap-then-unwrap in the middle zone). The body **always drops to its own line**
    /// when the construct overflows — uniformly across text, expression, void, and
    /// element/component bodies (a deliberate divergence from prettier, which hugs the
    /// `}` and breaks an element body internally; see
    /// `conformance_prettier.md` §Svelte: Blocks). The `conditional_group` fits-tests
    /// each candidate in flat mode, so an element body's inline candidate does not
    /// "fit by breaking internally" — it falls through to the expanded (drop) state.
    ///
    /// **Head or body forced multiline** → expand directly, skipping the
    /// `conditional_group`. An unconditional break (a `hardline`) anywhere in a candidate
    /// short-circuits `fits()` to "fits" — it stops the lookahead at the newline — so
    /// offering the inline candidate would *wrongly hug*. For the head that means hugging
    /// the tail (a trailing line comment); for the body it means **welding a multiline body
    /// to the head and close tag** (`{#each xs as x}<tr>⏎…⏎</tr>{/each}`), the block
    /// analogue of a delimiter dangle — the body's own line structure, which is render-free
    /// at the boundary, would be picking the layout.
    ///
    /// This tests the *inline* tail, so it fires only on an **unconditional** break
    /// (`will_break` is a sound static superset — hardline / propagated `breakParent`), not
    /// on a body that merely *might* wrap by width. A body that can only break internally
    /// (wrapping attributes) still measures flat and falls through to the expanded state on
    /// overflow, exactly as before.
    fn build_expanding_construct(
        &self,
        head_doc: DocId,
        inline_tail: DocId,
        multiline_tail: DocId,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        if d.will_break(head_doc) || d.will_break(inline_tail) {
            return self.fold_gt(gt_prefix, true, d.concat(&[head_doc, multiline_tail]));
        }
        let inline = self.fold_gt(gt_prefix, false, d.concat(&[head_doc, inline_tail]));
        let expanded = self.fold_gt(gt_prefix, true, d.concat(&[head_doc, multiline_tail]));
        d.conditional_group(&[inline, expanded])
    }

    /// Whether a block's body boundaries — the run between its head tag and its first
    /// child, and between its last child and the close / next branch tag — must **break**
    /// rather than hug, given whether every branch is inline-authored.
    ///
    /// A block branch is a **fragment**, and every fragment boundary in Svelte is
    /// **render-free**: the compiler removes start/end-of-content whitespace (only
    /// *inter-sibling* whitespace and `<pre>`/`<textarea>` are significant). So the
    /// character at that boundary carries no authorship signal and must not select the
    /// layout. Two consequences, both encoded here:
    ///
    /// - **Hug is all-or-nothing, per construct.** Keying each side on its own authored
    ///   whitespace lets a render-free character weld the body to one tag while the other
    ///   breaks (`{#if c}<div>a</div>⏎<div>b</div>{/if}`, `{#if c}⏎<div>a</div>{/if}`) —
    ///   the block analogue of a delimiter dangle, and a *different stable form per
    ///   authoring* of one document.
    /// - **A branch that renders inline still breaks** once any sibling branch went
    ///   multiline, so `{:else}` never welds its body while `{#if}` holds its own.
    ///
    /// The same invariant `ElementLayout::WithContent(BoundaryMode)` encodes for an
    /// element's content boundary. See conformance_prettier.md §Svelte: Inline content
    /// block-style. `<pre>`/`<textarea>` never reach here — they are dispatched to the
    /// whitespace-sensitive builder, where the boundary is literal and the hug mandatory.
    fn body_boundaries_break(all_branches_inline: bool) -> bool {
        !all_branches_inline
    }

    /// Whether every if-block branch (consequent, each `{:else if}` consequent,
    /// `{:else}`) is inline-authored — the precondition for the body-expand fast path.
    fn if_branches_all_inline(&self, block: &internal::IfBlock<'_>, imc: bool) -> bool {
        let mut all_inline = self.fragment_inline_authored(&block.consequent, imc);
        let mut alt = block.alternate.as_ref();
        while let Some(a) = alt {
            if let Some(else_if) = Self::get_flattenable_else_if(a) {
                all_inline &= self.fragment_inline_authored(&else_if.consequent, imc);
                alt = else_if.alternate.as_ref();
            } else {
                all_inline &= self.fragment_inline_authored(a, imc);
                alt = None;
            }
        }
        all_inline
    }

    /// Indent a section body, dropping it to its own line (leading `hardline`) when
    /// `multiline`; otherwise indent it in place (so a body that breaks internally still
    /// indents). The shared body-expand primitive for every block tail's body / branch /
    /// section / fallback.
    fn indent_body_expand(&self, body: DocId, multiline: bool) -> DocId {
        let d = self.d();
        if multiline {
            d.indent(d.concat(&[d.hardline(), body]))
        } else {
            d.indent(body)
        }
    }

    /// Build the if-tail's bodies and `{:else if}` heads **once** (mode-agnostic),
    /// flattening the `{:else if}` chain into a linear piece list. The inline and
    /// expanded tails are both composed from these shared pieces by
    /// [`Self::compose_if_tail`], so a nested body is built once rather than once per
    /// form (a per-form build would rebuild it twice, compounding to O(2^depth)).
    fn build_if_pieces(&self, block: &internal::IfBlock<'_>) -> Vec<IfPiece> {
        let mut pieces = vec![IfPiece::Consequent(
            self.build_fragment_doc(&block.consequent),
        )];
        let mut alt = block.alternate.as_ref();
        while let Some(a) = alt {
            if let Some(else_if) = Self::get_flattenable_else_if(a) {
                // Build the else-if head with wrapping enabled so it can dangle within the
                // expanded form; in the inline form `BlockHead` resolves flat (no dangle).
                let expr_doc = self.build_else_if_expr_doc(else_if, true);
                let head = self.build_block_head(
                    ELSE_IF_BLOCK_OPEN,
                    &else_if.test,
                    expr_doc,
                    None,
                    else_if.opening_tag_span.end - 1,
                    self.block_dangle_allowed(),
                );
                let body = self.build_fragment_doc(&else_if.consequent);
                pieces.push(IfPiece::ElseIf { head, body });
                alt = else_if.alternate.as_ref();
            } else {
                pieces.push(IfPiece::Else(self.build_nodes_doc(a.nodes)));
                alt = None;
            }
        }
        pieces
    }

    /// Compose an if-tail (consequent body + alternate branches + `{/if}`) in inline
    /// (`multiline = false`) or expanded (`multiline = true`) form from pre-built
    /// [`IfPiece`]s, for `build_expanding_construct`. Cheap — only indent / hardline
    /// wrapping, no body rebuilds. The `{:else if}` chain is emitted as one flat
    /// `concat` (its nesting is render-transparent).
    fn compose_if_tail(&self, pieces: &[IfPiece], multiline: bool) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        for piece in pieces {
            match piece {
                IfPiece::Consequent(body) => parts.push(self.indent_body_expand(*body, multiline)),
                IfPiece::ElseIf { head, body } => {
                    if multiline {
                        parts.push(d.hardline());
                    }
                    parts.push(*head);
                    parts.push(self.indent_body_expand(*body, multiline));
                }
                IfPiece::Else(body) => {
                    if multiline {
                        parts.push(d.hardline());
                    }
                    parts.push(d.text("{:else}"));
                    parts.push(self.indent_body_expand(*body, multiline));
                }
            }
        }
        if multiline {
            parts.push(d.hardline());
        }
        parts.push(d.text("{/if}"));
        d.concat(&parts)
    }

    /// Build if block doc with full context (multiline + preceding content).
    ///
    /// `has_preceding_breakable`: If true, there's breakable content before this block,
    /// so use remove_lines() to ensure that content breaks first.
    ///
    /// `gt_prefix`: a preceding inline-element sibling's split-off closing `>` to fold
    /// into the block (axis-3 sibling-`>` dangle, set only by `build_block_node_doc_with_gt`).
    /// The expanding fast path folds it into the inline-vs-multiline `conditional_group`;
    /// the authored-multiline tail dangles it via `dangle_gt`.
    pub(super) fn build_if_block_doc_with_full_context(
        &self,
        block: &internal::IfBlock<'_>,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        // Use remove_lines only if there's preceding breakable content (so it breaks first).
        // Otherwise, allow natural wrapping to respect print_width.
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_block_head_expr(
            IF_BLOCK_OPEN,
            block.opening_tag_span,
            &block.test,
            block.opening_tag_span.end - 1,
            allow_wrapping || in_multiline_context,
        );

        // Check leading/trailing whitespace, considering multiline context.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        // E.g., `{#if a} content {/if}` or `{#if a} content{/if}` → expand to multiline.
        let (has_leading, has_trailing) =
            self.fragment_ws_status(&block.consequent, in_multiline_context);
        // Force non-inline when block elements among multiple children
        // (matches prettier's forceBreakContent + breakParent)
        let force_break = self.fragment_should_force_break_content(block.consequent.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            IF_BLOCK_OPEN,
            &block.test,
            expr_doc,
            None,
            block.opening_tag_span.end - 1,
            can_wrap,
        );

        // Inline-authored block (consequent + every alternate branch): expand the
        // whole block — bodies, `{:else if}`/`{:else}` sections, and `{/if}` — onto
        // their own lines when the head wraps (or the construct overflows).
        if self.if_branches_all_inline(block, in_multiline_context) {
            let pieces = self.build_if_pieces(block);
            let inline_tail = self.compose_if_tail(&pieces, false);
            let multiline_tail = self.compose_if_tail(&pieces, true);
            return self.build_expanding_construct(
                head_doc,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        // Any branch rendering multiline breaks *every* boundary — the consequent, each
        // `{:else if}` / `{:else}`, and `{/if}`.
        let expand =
            Self::body_boundaries_break(self.if_branches_all_inline(block, in_multiline_context));

        // Build the consequent body only on the non-fast path — the fast path above
        // builds its own (shared) pieces, so building it eagerly made that path build the
        // consequent twice, keeping the nested-block fanout exponential. For inline: the
        // regular fragment doc (preserves spaces); for multiline: the multiline doc (line
        // structure with hardlines). Always indent() for internal break indentation.
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.consequent)
        } else {
            self.build_nodes_doc_multiline(block.consequent.nodes)
        };
        let indented_body = indent_body(self, body_doc, expand);

        let mut parts: DocBuf = smallvec![head_doc, indented_body];

        if let Some(alt) = &block.alternate {
            if expand {
                parts.push(d.hardline());
            }
            parts.push(self.build_if_alternate_doc(alt, expand, in_multiline_context));
        }

        if expand {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/if}"));
        // Non-expanding tail (authored-multiline branch): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&parts))
    }

    /// Check if a fragment can be flattened to an else-if.
    ///
    /// Returns the inner IfBlock only when the fragment is exactly one IfBlock
    /// (plus optional whitespace) AND the user authored it as `{:else if}`
    /// (Svelte's `elseif: true` flag). Returns None for multiple IfBlocks, other
    /// content, or a block-form `{:else}{#if}{/if}` (`elseif: false`): that form is
    /// preserved verbatim rather than collapsed — matching prettier, which keeps the
    /// two distinct (collapsing would be information loss).
    pub(super) fn get_flattenable_else_if<'arena>(
        alt: &Fragment<'arena>,
    ) -> Option<&'arena internal::IfBlock<'arena>> {
        // The boxed `IfBlock` variant is a `&'arena` pointer, so the returned
        // reference is tied to the arena, not to `alt`.
        let mut if_block: Option<&'arena internal::IfBlock<'arena>> = None;

        for node in alt.nodes {
            match node {
                FragmentNode::IfBlock(b) => {
                    if if_block.is_some() {
                        // Multiple IfBlocks - can't flatten
                        return None;
                    }
                    if_block = Some(b);
                }
                FragmentNode::Text(t) if t.is_ascii_ws_only => {
                    // Collapsible (ASCII) whitespace-only text is OK; a non-breaking
                    // space is content and blocks the elseif flatten.
                }
                _ => {
                    // Non-whitespace content - can't flatten
                    return None;
                }
            }
        }

        // Block-form `{:else}{#if}{/if}` (elseif: false) does not flatten — see fn doc.
        if_block.filter(|b| b.elseif)
    }

    /// Build the condition-expression doc for a flattened `{:else if}` block.
    ///
    /// Shared by the normal and whitespace-sensitive alternate printers.
    /// `get_flattenable_else_if` only returns genuine `{:else if}` blocks, so the
    /// opening is always the literal `{:else if ` and the expression starts that many
    /// chars past the opening-tag span.
    pub(super) fn build_else_if_expr_doc(
        &self,
        else_if: &internal::IfBlock<'_>,
        in_multiline_context: bool,
    ) -> DocId {
        self.build_block_head_expr(
            ELSE_IF_BLOCK_OPEN,
            else_if.opening_tag_span,
            &else_if.test,
            else_if.opening_tag_span.end - 1,
            in_multiline_context,
        )
    }

    /// Build doc for if block alternate (else or else-if).
    ///
    /// `expand` is the construct-wide boundary decision (see `body_boundaries_break`) — it
    /// is *not* re-derived per branch, because hug is all-or-nothing: a branch that renders
    /// inline still drops to its own line once any sibling branch went multiline.
    fn build_if_alternate_doc(
        &self,
        alt: &Fragment<'_>,
        expand: bool,
        in_multiline_context: bool,
    ) -> DocId {
        let d = self.d();
        // Check if this can be flattened to {:else if ...}
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            // {:else if condition}
            let expr_doc = self.build_else_if_expr_doc(else_if, in_multiline_context);

            // Build the body inline only when the whole construct stays inline; once it
            // expands, every branch body carries multiline line structure.
            let body_doc = if expand {
                self.build_nodes_doc_multiline(else_if.consequent.nodes)
            } else {
                self.build_fragment_doc(&else_if.consequent)
            };

            let indented_body = indent_body(self, body_doc, expand);

            // `build_else_if_expr_doc` builds the condition with `in_multiline_context`
            // as its wrapping flag, so the dangle keys on the same condition.
            let head_doc = self.build_block_head(
                ELSE_IF_BLOCK_OPEN,
                &else_if.test,
                expr_doc,
                None,
                else_if.opening_tag_span.end - 1,
                in_multiline_context && self.block_dangle_allowed(),
            );
            let mut parts: DocBuf = smallvec![head_doc, indented_body];

            if let Some(nested_alt) = &else_if.alternate {
                if expand {
                    parts.push(d.hardline());
                }
                parts.push(self.build_if_alternate_doc(nested_alt, expand, in_multiline_context));
            }

            return d.concat(&parts);
        }

        // Plain {:else}
        let body_doc = if expand {
            self.build_nodes_doc_multiline(alt.nodes)
        } else {
            self.build_nodes_doc(alt.nodes)
        };

        let indented_body = indent_body(self, body_doc, expand);

        d.concat(&[d.text("{:else}"), indented_body])
    }

    /// Whether the each block's body and its optional `{:else}` fallback are both
    /// inline-authored — the precondition for the body-expand fast path.
    fn each_branches_all_inline(&self, block: &internal::EachBlock<'_>, imc: bool) -> bool {
        let mut all_inline = self.fragment_inline_authored(&block.body, imc);
        if let Some(fallback) = &block.fallback {
            all_inline &= self.fragment_inline_authored(fallback, imc);
        }
        all_inline
    }

    /// Build the each-block's body and optional `{:else}` fallback **once**
    /// (mode-agnostic), for composition into both expanding-construct tails by
    /// [`Self::compose_each_tail`] — so a nested body is built once rather than once
    /// per form (a per-form build would rebuild it twice, compounding to O(2^depth)).
    fn build_each_pieces(&self, block: &internal::EachBlock<'_>) -> (DocId, Option<DocId>) {
        let body = self.build_fragment_doc(&block.body);
        let fallback = block.fallback.as_ref().map(|f| self.build_fragment_doc(f));
        (body, fallback)
    }

    /// Compose an each-block tail (body + optional `{:else}` fallback + `{/each}`) in
    /// inline or expanded form from pre-built pieces, for `build_expanding_construct`.
    /// Cheap — only indent / hardline wrapping, no body rebuilds.
    fn compose_each_tail(&self, body: DocId, fallback: Option<DocId>, multiline: bool) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        parts.push(self.indent_body_expand(body, multiline));
        if let Some(fb) = fallback {
            if multiline {
                parts.push(d.hardline());
            }
            parts.push(d.text("{:else}"));
            parts.push(self.indent_body_expand(fb, multiline));
        }
        if multiline {
            parts.push(d.hardline());
        }
        parts.push(d.text("{/each}"));
        d.concat(&parts)
    }

    /// Build each block doc with full context (multiline + preceding content).
    ///
    /// `gt_prefix`: see `build_if_block_doc_with_full_context`.
    pub(super) fn build_each_block_doc_with_full_context(
        &self,
        block: &internal::EachBlock<'_>,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_comment_end = each_expr_comment_end(block);
        let expr_doc = self.build_block_head_expr(
            EACH_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            expr_comment_end,
            allow_wrapping || in_multiline_context,
        );

        // Build the optional key doc (shared between the clause and degenerate paths).
        let key_doc = block.key.as_ref().map(|key| {
            // The key expression is inside parens, so the offset accounts for that.
            if let Some(key_span) = block.key_span {
                self.build_expression_doc_for_block(
                    key,
                    key_span.start + 1, // after "("
                    key_span.end - 1,   // before ")"
                    1,                  // "(" = 1 char (key is inside parens)
                    allow_wrapping || in_multiline_context,
                )
            } else {
                // No key_span: build doc directly
                self.build_ts_expression_doc(key)
            }
        });

        // Separate the breakable expression from its clause so the clause + `}` can
        // dedent together onto their own line when the head wraps. `clause` is the
        // `as pattern[, index][ (key)]` tail WITHOUT its leading space (added by
        // `build_block_head_doc`); the degenerate index/key-without-`as` cases (not
        // valid Svelte) keep hugging the expression unchanged.
        let (head_expr, clause) = if let Some(context) = &block.context {
            let mut clause_parts: DocBuf = smallvec![d.text("as ")];
            clause_parts.push(self.build_pattern_doc(context));
            if let Some(index) = block.index {
                clause_parts.push(d.text(", "));
                clause_parts.push(d.text_pooled(index));
            }
            if let Some(kd) = key_doc {
                clause_parts.push(d.text(" ("));
                clause_parts.push(kd);
                clause_parts.push(d.text(")"));
            }
            (expr_doc, Some(d.concat(&clause_parts)))
        } else {
            // No `as`: any index/key is degenerate — keep it hugging the expression.
            let mut e: DocBuf = smallvec![expr_doc];
            if let Some(index) = block.index {
                e.push(d.text(", "));
                e.push(d.text_pooled(index));
            }
            if let Some(kd) = key_doc {
                e.push(d.text(" ("));
                e.push(kd);
                e.push(d.text(")"));
            }
            (d.concat(&e), None)
        };

        // Check leading/trailing whitespace, considering multiline context.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) =
            self.fragment_ws_status(&block.body, in_multiline_context);
        // Force non-inline when block elements among multiple children
        let force_break = self.fragment_should_force_break_content(block.body.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            EACH_BLOCK_OPEN,
            &block.expression,
            head_expr,
            clause,
            expr_comment_end,
            can_wrap,
        );

        // Inline-authored block (body + `{:else}` fallback): expand the body,
        // `{:else}` section, and `{/each}` onto their own lines when the head wraps
        // (or the construct overflows).
        if self.each_branches_all_inline(block, in_multiline_context) {
            let (body, fallback) = self.build_each_pieces(block);
            let inline_tail = self.compose_each_tail(body, fallback, false);
            let multiline_tail = self.compose_each_tail(body, fallback, true);
            return self.build_expanding_construct(
                head_doc,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        // Either branch rendering multiline breaks *every* boundary — the body, `{:else}`,
        // and `{/each}`.
        let expand =
            Self::body_boundaries_break(self.each_branches_all_inline(block, in_multiline_context));

        // Build the body only on the non-fast path (the fast path above builds its own
        // shared pieces). For inline: regular fragment doc (preserves spacing); for
        // multiline: the multiline doc (line structure with hardlines).
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(block.body.nodes)
        };
        let indented_body = indent_body(self, body_doc, expand);

        let mut parts: DocBuf = smallvec![head_doc, indented_body];

        if let Some(fallback) = &block.fallback {
            if expand {
                parts.push(d.hardline());
            }

            parts.push(d.text("{:else}"));

            let fallback_doc = if expand {
                self.build_nodes_doc_multiline(fallback.nodes)
            } else {
                self.build_fragment_doc(fallback)
            };

            parts.push(indent_body(self, fallback_doc, expand));
        }

        if expand {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/each}"));
        // Non-expanding tail (authored-multiline body): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&parts))
    }

    /// Whether a fragment is inline-authored (no leading/trailing whitespace — incl.
    /// space-only — and no forced break) — the precondition for the body-expand fast
    /// path on `{#if}` / `{#each}` / `{#await}` bodies, branches, and sections. Uses
    /// the same `fragment_ws_status` the non-fast-path `is_inline` uses, so space-only
    /// / newline-authored fragments fall through to the existing whitespace-respecting
    /// paths.
    fn fragment_inline_authored(&self, frag: &Fragment<'_>, in_multiline_context: bool) -> bool {
        let (has_leading, has_trailing) = self.fragment_ws_status(frag, in_multiline_context);
        !has_leading && !has_trailing && !self.fragment_should_force_break_content(frag.nodes)
    }

    /// The `{:then …}` keyword doc — `{:then value}` if a `then` value binds, else
    /// `{:then}` if the then-section has content, else `None`. Whether to emit it is the
    /// caller's decision: a `then`-shorthand carries it in the head instead.
    fn await_then_keyword(&self, block: &internal::AwaitBlock<'_>) -> Option<DocId> {
        // An empty-body `:then` is dropped entirely — no marker — matching prettier.
        if !then_has_content(block) {
            return None;
        }
        let d = self.d();
        if let Some(value) = &block.value {
            Some(d.concat(&[
                d.text("{:then "),
                self.build_pattern_doc(value),
                d.text("}"),
            ]))
        } else {
            Some(d.text("{:then}"))
        }
    }

    /// The `{:catch …}` keyword doc — `{:catch error}` if an error binds, else `{:catch}`
    /// if the catch-section has content, else `None`. A `catch`-shorthand carries it in the
    /// head instead.
    fn await_catch_keyword(&self, block: &internal::AwaitBlock<'_>) -> Option<DocId> {
        let d = self.d();
        if let Some(error) = &block.error {
            Some(d.concat(&[
                d.text("{:catch "),
                self.build_pattern_doc(error),
                d.text("}"),
            ]))
        } else if block.catch.as_ref().is_some_and(|c| !c.nodes.is_empty()) {
            Some(d.text("{:catch}"))
        } else {
            None
        }
    }

    /// Which shorthand carries its clause in the head, so the tail omits that keyword:
    /// `(then-shorthand, catch-shorthand)`. Derived from [`await_shorthand`] so it can't drift
    /// from the head-clause builder.
    fn await_shorthand_flags(block: &internal::AwaitBlock<'_>) -> (bool, bool) {
        match await_shorthand(block) {
            AwaitShorthand::Then => (true, false),
            AwaitShorthand::Catch => (false, true),
            AwaitShorthand::None => (false, false),
        }
    }

    /// Build each present await section's body + the un-shorthanded `{:then}` / `{:catch}`
    /// keyword **once** (mode-agnostic), for composition into both expanding-construct
    /// tails by [`Self::compose_await_tail`]. Bodies are the raw `build_fragment_doc`; the
    /// per-mode indent wrapping is applied at composition.
    fn build_await_pieces(&self, block: &internal::AwaitBlock<'_>) -> AwaitPieces {
        let (is_then_shorthand, is_catch_shorthand) = Self::await_shorthand_flags(block);
        AwaitPieces {
            pending: block.pending.as_ref().map(|p| self.build_fragment_doc(p)),
            then_kw: (!is_then_shorthand)
                .then(|| self.await_then_keyword(block))
                .flatten(),
            // An empty-body `:then` is dropped (marker + body); keep only a content body.
            then_body: block
                .then
                .as_ref()
                .filter(|_| then_has_content(block))
                .map(|t| self.build_fragment_doc(t)),
            catch_kw: (!is_catch_shorthand)
                .then(|| self.await_catch_keyword(block))
                .flatten(),
            catch_body: block.catch.as_ref().map(|c| self.build_fragment_doc(c)),
        }
    }

    /// Compose the await tail (section bodies + `{:then}` / `{:catch}` keywords +
    /// `{/await}`) in inline or expanded form from pre-built [`AwaitPieces`], for
    /// `build_expanding_construct`. Cheap — only indent / hardline wrapping, no rebuilds.
    fn compose_await_tail(&self, p: &AwaitPieces, multiline: bool) -> DocId {
        let d = self.d();
        let mut parts: DocBuf = DocBuf::new();
        if let Some(pending) = p.pending {
            parts.push(self.indent_body_expand(pending, multiline));
        }
        if let Some(kw) = p.then_kw {
            if multiline {
                parts.push(d.hardline());
            }
            parts.push(kw);
        }
        if let Some(then_body) = p.then_body {
            parts.push(self.indent_body_expand(then_body, multiline));
        }
        if let Some(kw) = p.catch_kw {
            if multiline {
                parts.push(d.hardline());
            }
            parts.push(kw);
        }
        if let Some(catch_body) = p.catch_body {
            parts.push(self.indent_body_expand(catch_body, multiline));
        }
        if multiline {
            parts.push(d.hardline());
        }
        parts.push(d.text("{/await}"));
        d.concat(&parts)
    }

    /// Push a space-only section's body to `parts` when it carries printable content, and
    /// **report whether it did**. A whitespace-only section (`build_nodes_doc_multiline`
    /// drops its whitespace-only nodes, so its body is empty) contributes nothing; the caller
    /// emits a separator `line` only after a section that returned `true`, so an empty section
    /// **collapses** — its surrounding markers glue (`{#await p}{:then v}{/await}`), exactly
    /// as every other empty construct does. A content section keeps its `indent_body_soft`,
    /// whose leading `line` is the space *before* the content; the returned `true` gates the
    /// space *after* it.
    fn push_await_space_only_body(&self, parts: &mut DocBuf, fragment: &Fragment<'_>) -> bool {
        if fragment.nodes.iter().any(|n| !n.is_whitespace_only_text()) {
            let body = self.build_nodes_doc_multiline(fragment.nodes);
            parts.push(indent_body_soft(self, body));
            true
        } else {
            false
        }
    }

    /// Build the await tail for the **space-only** layout. A content-bearing section body
    /// (`indent_body_soft`, via `push_await_space_only_body`) is wrapped in single spaces, but
    /// a **whitespace-only** section contributes no body and its markers **glue**
    /// (`{#await p}{:then v}{/await}`): a separator `line` is emitted only after a section that
    /// carried content, so an empty section collapses to `}{` exactly as every other empty
    /// construct does — while the `{:then}` / `{:catch}` markers are kept (prettier instead
    /// deletes the whole section; see `conformance_prettier.md` §Svelte: Blocks). The whole
    /// construct still breaks together as a unit under the caller's `group`. Mirrors
    /// `compose_await_tail`, but with **conditional** `line()` separators and soft-indented
    /// bodies (the head is prepended + grouped by the caller).
    fn build_await_tail_space_only(&self, block: &internal::AwaitBlock<'_>) -> DocId {
        let d = self.d();
        let (is_then_shorthand, is_catch_shorthand) = Self::await_shorthand_flags(block);
        let mut parts: DocBuf = DocBuf::new();
        // Whether the section immediately before the next marker carried content — gates that
        // marker's leading separator `line`, so an empty section's markers glue.
        let mut prev_had_content = false;
        if let Some(pending) = &block.pending {
            prev_had_content = self.push_await_space_only_body(&mut parts, pending);
        }
        if !is_then_shorthand && let Some(kw) = self.await_then_keyword(block) {
            if prev_had_content {
                parts.push(d.line());
            }
            parts.push(kw);
            prev_had_content = false;
        }
        if let Some(then_block) = &block.then {
            prev_had_content = self.push_await_space_only_body(&mut parts, then_block);
        }
        if !is_catch_shorthand && let Some(kw) = self.await_catch_keyword(block) {
            if prev_had_content {
                parts.push(d.line());
            }
            parts.push(kw);
            prev_had_content = false;
        }
        if let Some(catch_block) = &block.catch {
            prev_had_content = self.push_await_space_only_body(&mut parts, catch_block);
        }
        if prev_had_content {
            parts.push(d.line());
        }
        parts.push(d.text("{/await}"));
        d.concat(&parts)
    }

    /// Build the await tail for the **newline-authored** layout: section bodies via
    /// `build_await_section_body`, with a `hardline` before each keyword and before
    /// `{/await}` when the construct expands. Mirrors `compose_await_tail`; `expand` is the
    /// construct-wide boundary decision (see `Self::body_boundaries_break`), so every
    /// section boundary breaks together rather than keying on its own authored whitespace
    /// (the head is prepended by the caller).
    fn build_await_tail_newline(&self, block: &internal::AwaitBlock<'_>, expand: bool) -> DocId {
        let d = self.d();
        let (is_then_shorthand, is_catch_shorthand) = Self::await_shorthand_flags(block);
        let mut parts: DocBuf = DocBuf::new();
        if let Some(pending) = &block.pending {
            parts.push(build_await_section_body(self, pending, expand));
        }
        if !is_then_shorthand && let Some(kw) = self.await_then_keyword(block) {
            if expand {
                parts.push(d.hardline());
            }
            parts.push(kw);
        }
        // An empty-body `:then` is dropped (marker via `await_then_keyword` above, body here).
        if then_has_content(block)
            && let Some(then_block) = &block.then
        {
            parts.push(build_await_section_body(self, then_block, expand));
        }
        if !is_catch_shorthand && let Some(kw) = self.await_catch_keyword(block) {
            if expand {
                parts.push(d.hardline());
            }
            parts.push(kw);
        }
        if let Some(catch_block) = &block.catch {
            parts.push(build_await_section_body(self, catch_block, expand));
        }
        if expand {
            parts.push(d.hardline());
        }
        parts.push(d.text("{/await}"));
        d.concat(&parts)
    }

    /// Build a doc for an await block (no preceding context / sibling `>`).
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_await_block_doc(&self, block: &internal::AwaitBlock<'_>) -> DocId {
        self.build_await_block_doc_with_full_context(block, false, false, None)
    }

    /// Build await block doc with full context (multiline + preceding content).
    ///
    /// `gt_prefix`: a preceding inline-element sibling's split-off closing `>` to fold into
    /// the block (axis-3 sibling-`>` dangle, set only by `build_block_node_doc_with_gt`). The
    /// expanding fast path folds it into the inline-vs-multiline `conditional_group`; the
    /// space-only / newline tails dangle it via `dangle_gt`.
    pub(super) fn build_await_block_doc_with_full_context(
        &self,
        block: &internal::AwaitBlock<'_>,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        // When a `then`/`catch` shorthand carries its binding pattern in the head, bound
        // the awaited expression's trailing-comment range at the pattern start (mirroring
        // `{#each}`'s `context.span().start`) so a comment *inside* the pattern isn't
        // relocated out to trail the expression (`{#await p /* c */ then …}`); the
        // comment-aware `build_pattern_doc` preserves it in place instead. The full form
        // carries its patterns in `{:then}`/`{:catch}` outside the head, so it keeps the
        // head end.
        let head_end = block.opening_tag_span.end - 1;
        let expr_comment_end = match await_shorthand(block) {
            AwaitShorthand::Then => block.value.as_ref().map_or(head_end, |v| v.span().start),
            AwaitShorthand::Catch => block.error.as_ref().map_or(head_end, |e| e.span().start),
            AwaitShorthand::None => head_end,
        };
        let expr_doc = self.build_block_head_expr(
            AWAIT_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            expr_comment_end,
            allow_wrapping || in_multiline_context,
        );

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);

        // Fast path: every present section is inline-authored → body-expand like the
        // other blocks. The head carries the `then v` / `catch e` clause; the section
        // bodies + `{:then}`/`{:catch}` keywords + `{/await}` all drop to their own
        // lines when the head wraps, chosen in one pass by `build_expanding_construct`.
        let sections = [&block.pending, &block.then, &block.catch];
        let has_section = sections
            .iter()
            .any(|f| f.as_ref().is_some_and(|f| !f.nodes.is_empty()));
        // `fragment_inline_authored` (via `fragment_ws_status`) already treats a
        // space-only section as non-inline, so space-only await blocks fall through to
        // the `has_space_only` group path below.
        let all_sections_inline = sections
            .iter()
            .filter_map(|f| f.as_ref())
            .all(|f| self.fragment_inline_authored(f, in_multiline_context));
        // Uniform body-drop: when every present section is inline-authored, the body +
        // `{:then}`/`{:catch}` keywords + `{/await}` drop to their own lines on overflow.
        // Keyed on `can_wrap` — the same gate `{#if}`/`{#each}` use — so the body hugs in the
        // inline-content/hug-both path (`can_wrap` false) but drops in the multiline-fragment
        // path. A block-parent sibling routes await through the multiline path via
        // `has_control_flow_after_sibling` (so `can_wrap` is true there); an inline parent
        // keeps `can_wrap` false and hugs, matching `{#if}`/`{#each}`.
        // Shorthand clause lives in the head: `then v` / bare `then`, or `catch e` / bare
        // `catch`; the full form has none. Built once, shared by the fast path, the
        // space-only tail, and the newline-authored tail. Classified by `await_shorthand`,
        // the same source `await_shorthand_flags` uses to skip the head-carried keyword.
        let clause = match await_shorthand(block) {
            AwaitShorthand::Then => Some(match &block.value {
                Some(value) => d.concat(&[d.text("then "), self.build_pattern_doc(value)]),
                None => d.text("then"),
            }),
            AwaitShorthand::Catch => Some(match &block.error {
                Some(error) => d.concat(&[d.text("catch "), self.build_pattern_doc(error)]),
                None => d.text("catch"),
            }),
            AwaitShorthand::None => None,
        };
        // `comment_end` is bound at `expr_comment_end` (not the head end) so a line
        // comment *inside* a shorthand pattern isn't mistaken for a trailing line comment
        // on the awaited expression — that would drop the space before the `then`/`catch`
        // clause.
        let head_doc = self.build_block_head(
            AWAIT_BLOCK_OPEN,
            &block.expression,
            expr_doc,
            clause,
            expr_comment_end,
            can_wrap,
        );

        // Fast path: every present section is inline-authored → body-expand like the other
        // blocks. The section bodies + `{:then}`/`{:catch}` keywords + `{/await}` all drop to
        // their own lines when the head wraps, chosen in one pass by `build_expanding_construct`.
        if has_section && all_sections_inline {
            let pieces = self.build_await_pieces(block);
            let inline_tail = self.compose_await_tail(&pieces, false);
            let multiline_tail = self.compose_await_tail(&pieces, true);
            return self.build_expanding_construct(
                head_doc,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        // Space-only sections break together as one unit under a single `group` (the tail
        // uses `line()` separators); newline-authored sections key hardlines on authored
        // trailing whitespace. Both reuse the hoisted head + the shared tail builders.
        let has_space_only = [&block.pending, &block.then, &block.catch].iter().any(|f| {
            f.as_ref()
                .is_some_and(|f| self.fragment_has_space_only_ws(f))
        });
        if has_space_only {
            let tail = self.build_await_tail_space_only(block);
            // This `group` can fit inline, so its break is a *render-time* decision that
            // `will_break` (and thus `dangle_gt`) can't see. Fold a preceding sibling's
            // split-off `>` (`gt_prefix`) *inside* the group with `if_break`, keyed on the
            // group's own flat/break state: hug when it fits (`>{#await…}`), dangle onto its
            // own line when it breaks (`⏎>{#await…}`). The `None` arm is a bare group,
            // byte-identical to a block with no preceding sibling.
            let group_body = d.concat(&[head_doc, tail]);
            return match gt_prefix {
                Some(gt) => {
                    let folded_gt = d.if_break(d.concat(&[d.hardline(), gt]), gt);
                    d.group(d.concat(&[folded_gt, group_body]))
                }
                None => d.group(group_body),
            };
        }

        // Any section rendering multiline breaks *every* boundary — each section body, the
        // `{:then}` / `{:catch}` keywords, and `{/await}`.
        let expand = Self::body_boundaries_break(all_sections_inline);
        let tail = self.build_await_tail_newline(block, expand);
        // Non-expanding tail (newline-authored sections): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&[head_doc, tail]))
    }

    /// Build a doc for a key block (no preceding context / sibling `>`).
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(crate) fn build_key_block_doc(&self, block: &internal::KeyBlock<'_>) -> DocId {
        self.build_key_block_doc_with_full_context(block, false, false, None)
    }

    /// Build key block doc with full context (multiline + preceding content).
    ///
    /// `gt_prefix`: see `build_if_block_doc_with_full_context`.
    pub(super) fn build_key_block_doc_with_full_context(
        &self,
        block: &internal::KeyBlock<'_>,
        in_multiline_context: bool,
        has_preceding_breakable: bool,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Build expression doc with context-dependent behavior
        let allow_wrapping = !has_preceding_breakable;
        let expr_doc = self.build_block_head_expr(
            KEY_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            block.opening_tag_span.end - 1,
            allow_wrapping || in_multiline_context,
        );

        // Check leading/trailing whitespace, considering space-only patterns.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) = self.fragment_ws_status(&block.fragment, false);
        // Force non-inline when block elements among multiple children
        let force_break = self.fragment_should_force_break_content(block.fragment.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // A multiline body breaks both boundaries — the body and `{/key}`.
        let expand = Self::body_boundaries_break(is_inline);

        // For inline: use regular fragment doc (preserves inline spacing)
        // For multiline: use multiline doc (preserves line structure with hardlines)
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.fragment)
        } else {
            self.build_nodes_doc_multiline(block.fragment.nodes)
        };

        let indented_body = indent_body(self, body_doc, expand);

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            KEY_BLOCK_OPEN,
            &block.expression,
            expr_doc,
            None,
            block.opening_tag_span.end - 1,
            can_wrap,
        );
        let close = d.text("{/key}");
        if is_inline {
            // Inline-authored body: expand it + `{/key}` when the head wraps.
            return self.build_expanding_block(head_doc, body_doc, close, gt_prefix);
        }

        let mut parts: DocBuf = smallvec![head_doc, indented_body];

        if expand {
            parts.push(d.hardline());
        }

        parts.push(close);
        // Non-expanding tail (authored-multiline body): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&parts))
    }

    /// Build a doc for a snippet block (no sibling `>` to fold).
    pub(crate) fn build_snippet_block_doc(&self, block: &internal::SnippetBlock<'_>) -> DocId {
        self.build_snippet_block_doc_with_full_context(block, None)
    }

    /// Build a doc for a snippet block, optionally folding a preceding sibling's `>`.
    ///
    /// Uses same inline/multiline pattern as if blocks. Opening tag uses group() for
    /// parameter wrapping when they exceed print width. Takes no context: the head wraps by
    /// its own width (its `BlockHead` group), and the body-drop is likewise decided by
    /// **width** (the `conditional_group` in `build_expanding_block`) — never by whether the
    /// head may wrap, which would let a render-free boundary select the layout (see
    /// `body_boundaries_break`).
    pub(crate) fn build_snippet_block_doc_with_full_context(
        &self,
        block: &internal::SnippetBlock<'_>,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Check leading/trailing whitespace, considering space-only patterns.
        // Space-only whitespace (no newlines) also triggers expansion to match prettier.
        let (has_leading, has_trailing) = self.fragment_ws_status(&block.body, false);
        let force_break = self.fragment_should_force_break_content(block.body.nodes);
        let is_inline = !has_leading && !has_trailing && !force_break;

        // Type parameters (generics). Parsed nodes route through tsv_ts's type-parameter
        // printer (constraints, defaults, modifiers, interior comments, width-based
        // wrapping of a long generic list — its own group, so it breaks independently of
        // the parameter list). The raw-text fallback (parse failure) emits the source
        // verbatim between `<` and `>`.
        let type_params_part = if let Some(decl) = &block.type_parameters {
            tsv_ts::build_type_parameters_doc_with_comments(d, decl, &self.ts_inputs(), &self.embed)
        } else if let Some(raw) = block.type_params_raw {
            d.concat(&[d.text("<"), d.text_pooled(raw), d.text(">")])
        } else {
            d.empty()
        };

        // Parameter list `(…)`. The parens fold so that when they wrap, `)` dedents to
        // base and `}` hugs it (`)}`) — no dangle (no trailing comma; trailingComma:
        // 'none').
        let params_inner = if let Some(raw) = &block.raw_parameters {
            // Parse-failure fallback: emit the raw parameter source (comments preserved
            // verbatim), split at top-level commas so a long list still wraps one-per-line.
            let params_docs: DocBuf = split_raw_params_at_commas(raw)
                .iter()
                .map(|s| d.text_pooled(s))
                .collect();
            let mut parts: DocBuf = DocBuf::with_capacity(params_docs.len() * 2);
            for (i, param_doc) in params_docs.into_iter().enumerate() {
                if i > 0 {
                    parts.push(d.text(","));
                    parts.push(d.line());
                }
                parts.push(param_doc);
            }
            d.concat(&[
                d.text("("),
                d.indent_softline(d.concat(&parts)),
                d.softline(),
                d.text(")"),
            ])
        } else {
            // Parsed parameters route through the same comment-aware,
            // `FunctionParameter`-context printer a real function signature uses, so
            // interior comments (`{ a = /* c */ 1 }`), boundary comments (`a /* c */, b`),
            // the single-pattern hug, and nesting-depth expansion all match a standalone
            // parameter list. `build_function_params_doc_with_comments` emits the `(…)`
            // with no group of its own — the `group` below drives the wrap.
            match block.params_paren {
                Some(paren) => tsv_ts::build_function_params_doc_with_comments(
                    d,
                    block.parameters,
                    Some(paren.start),
                    Some(paren.end),
                    &self.ts_inputs(),
                    &self.embed,
                ),
                None => d.text("()"),
            }
        };
        // The parameter list gets its OWN group so it breaks independently of the
        // type-parameter group (mirroring a real function signature, where `<…>` and
        // `(…)` are sibling groups): a long generic list can wrap while short params stay
        // inline on the closing `>(…)}` line, and vice-versa. The outer `BlockHead` group
        // still governs the head as a whole.
        let params_doc = d.group(params_inner);

        // Opening tag `{#snippet name<T>(params)}`. Key the group to `BlockHead` so the
        // body can expand when the params wrap (below).
        //   When fits: {#snippet name(a, b, c)}
        //   When wraps: {#snippet name(\n\ta,\n\tb,\n\tc\n)}
        let opening_doc = d.group_with_id(
            d.concat(&[
                d.text("{#snippet "),
                // The snippet name, verbatim from the identifier expression's span.
                d.source_span(block.expression.span(), self.source),
                type_params_part,
                params_doc,
                d.text("}"),
            ]),
            GroupId::BlockHead,
        );

        // Body: inline hugs directly, multiline uses hardlines
        let body_doc = if is_inline {
            self.build_fragment_doc(&block.body)
        } else {
            self.build_nodes_doc_multiline(block.body.nodes)
        };

        // Inline-authored body: expand the body + `{/snippet}` onto their own lines
        // when the construct overflows (params wrap, or head + body exceeds width) —
        // uniformly, including paramless snippets. Keyed to the opening group above.
        if is_inline {
            let close = d.text("{/snippet}");
            return self.build_expanding_block(opening_doc, body_doc, close, gt_prefix);
        }

        // A multiline body breaks both boundaries — the body and `{/snippet}`.
        let expand = Self::body_boundaries_break(is_inline);

        let mut parts: DocBuf = smallvec![opening_doc];
        parts.push(indent_body(self, body_doc, expand));

        if expand {
            parts.push(d.hardline());
        }

        parts.push(d.text("{/snippet}"));
        // Non-expanding tail (authored-multiline body): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&parts))
    }
}
