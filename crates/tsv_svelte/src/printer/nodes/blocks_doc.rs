// Doc builders for Svelte control-flow blocks
//
// {#if}/{:else if}/{:else}, {#each}, {#await}, {#key}, and {#snippet} —
// opening/closing tag layout, branch flattening, and section bodies.

// Allow Svelte block syntax like `{:else}`, `{:then}`, `{:catch}` which
// look like Rust format args but are valid Svelte template syntax.
#![allow(clippy::literal_string_with_formatting_args)]

use crate::ast::internal::{self, Fragment, FragmentNode};
use crate::printer::{HeadExpr, Printer};
use smallvec::smallvec;
use tsv_lang::doc::arena::DocId;
use tsv_lang::doc::{DocBuf, GroupId};

use super::element_doc::MultilineCause;
use super::helpers::each_expr_comment_end;

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

/// Build one `{#await}` section body (`pending` / `then` / `catch`) for the
/// newline-authored tail.
///
/// `expand` is construct-wide (hug is all-or-nothing — see
/// `Printer::fragment_inline_authored`): every section's boundaries break together, so
/// a section authored inline still drops to its own line once any sibling section went
/// multiline. Keying it per-section on that section's own authored whitespace would
/// let a render-free character weld one section's body to its keyword while the others
/// break. (`expand` is false only for a section-less await, where every fragment is
/// empty — the non-expand arm builds nothing.)
fn build_await_section_body(printer: &Printer<'_>, fragment: &Fragment<'_>, expand: bool) -> DocId {
    let body_doc = if expand {
        printer.build_nodes_doc_multiline(fragment.nodes)
    } else {
        // `expand` is false only for a section-less await, whose every fragment is empty (see the
        // doc comment above) — so there is nothing to lay out and the arm is spelled as the empty
        // doc it always produced. It used to route through a whole general-purpose fragment
        // builder to reach that same `empty()`, which made the builder look live: it was in fact
        // this call, always with a zero-length slice, and its loop body never executed once across
        // the fixture tree and ten real repos.
        printer.d().empty()
    };
    printer.indent_body_expand(body_doc, expand)
}

/// Whether this expression's broken form **ends on its own closing delimiter, dedented to
/// the tag's base indent**.
///
/// That is the whole question behind the head-closer hug: when the answer is yes the head's
/// last rendered line already starts with a closer, the layout rule says don't break such a
/// line, and the clause + the tag's own closer continue on it (`) as item}`, `)}`, `}}`,
/// `] as x}`, an `{#each}` key's `))}`). When it is no they take their own line at base.
/// Named for the question rather than the effect, because two callers ask it of two
/// different closers.
///
/// ⚠️ Answering it for **calls only** was a long-lived under-read of that very argument:
/// every bracket-delimited literal dedents its closer exactly the same way, so an object,
/// array, function, class or block-bodied arrow head rendered a SECOND closer line under
/// the first (`{#if {⏎…⏎}⏎}`), at every block tag and at an `{#each}` key alike.
///
/// The **transparent** arms carry the same reasoning through a wrapper that cannot move the
/// last line: a prefix operator (`!fn(⏎…⏎)`, `await fn(⏎…⏎)`) leaves the operand's `)`
/// leading it, an angle-bracket cast likewise (`<T>{⏎…⏎}`), and a non-null `!` glues to that
/// closer (`)!`), which is still a closer-led line.
///
/// ⚠️ **Under-approximating here is safe; over-approximating is not.** A false negative
/// costs one extra closer line — verbose, idempotent, reparses. A false positive glues the
/// tag's closer onto a line of *content*, which is a layout the reparse does not reproduce.
/// So a wrapper recurses on its operand even where it sometimes synthesizes a paren shell of
/// its own (`<T>(a &&⏎\tb)` really does end at `)`, and is left reading `false`) — add a kind
/// here only when its broken form **always** ends on a dedented delimiter.
///
/// ⚠️ The verdict is **static, but the question it stands in for is dynamic**: it assumes the
/// expression is what broke the head. When something ELSE breaks the head and the expression
/// stays flat — a multi-line comment ahead of it, whose interior newlines are verbatim — the
/// last line ends `*/ fn(aa)` and starts with no closer at all, so a yes here hugs a line of
/// content. Every yes arm has that hole and it long predates the list; it is left alone
/// rather than papered over, since `hug` cannot tell "my delimiter broke" from "my group was
/// broken for me" (the head group is the same group either way).
/// TODO: a per-kind static verdict cannot separate those; distinguishing them needs the
/// expression's own delimiter break to carry its own group, at which point the arms below
/// become an `if_break` on it. No fixture pins the hole on purpose — `input.svelte` must be
/// idempotent, so one could only bake the hugged form in as canonical.
/// `JsdocCast` is deliberately **absent** for that reason and no other: its parens are
/// unconditional, but the flat-cast-under-a-multi-line-comment shape is the one this
/// codebase already pins (`svelte/blocks/head_jsdoc_cast_multiline_comment_svelte_prettier_divergence`),
/// so adding it would move a committed shape to fix a rarer one.
///
/// False for a binary/logical chain (ends on an operand), a multi-segment member chain (a
/// `.method(...)` segment, not a bare `)`), a sequence (its `)` is glued to the last operand
/// one indent in, not at base), and an `as` / `satisfies` cast (ends on the type).
fn ends_at_base_closer(expr: &tsv_ts::Expression<'_>) -> bool {
    use tsv_ts::Expression as E;
    use tsv_ts::ast::internal::ArrowFunctionBody;
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
        // `import(…)` owns its parens outright, so it always ends at `)` — the plain
        // call arm one syntax over. (A JSDoc cast does too; see the doc comment for why
        // it is nonetheless off this list.)
        E::ImportExpression(_) => true,
        // Bracket-delimited literals: a broken one puts its own `}` / `]` on a line at
        // base. (An empty one never breaks, so the hug is unreachable, not wrong.)
        E::ObjectExpression(_)
        | E::ArrayExpression(_)
        | E::FunctionExpression(_)
        | E::ClassExpression(_) => true,
        // Only a BLOCK body ends on a delimiter; an expression body ends wherever that
        // expression ends (`() => a &&⏎\tb`), one indent in.
        E::ArrowFunctionExpression(a) => matches!(a.body, ArrowFunctionBody::BlockStatement(_)),
        // Transparent wrappers — see the doc comment above.
        E::UnaryExpression(u) => ends_at_base_closer(u.argument),
        E::AwaitExpression(a) => ends_at_base_closer(a.argument),
        E::TSTypeAssertion(t) => ends_at_base_closer(t.expression),
        E::TSNonNullExpression(t) => ends_at_base_closer(t.expression),
        _ => false,
    }
}

/// Which head [`Printer::build_block_head`] is assembling — the token it dangles, the group
/// its `if_break` keys on, and (for a block tag) the clause that rides ahead of the token.
///
/// Two heads take this exact shape, and they are one function because the closer rule is one
/// rule: the head wraps ⇒ the closer drops to the tag's base indent, unless the content's own
/// last line already ends there on its own closer ([`ends_at_base_closer`]). Spelling it twice
/// is how the key came to answer it differently from the head it sits in.
#[derive(Clone, Copy)]
enum HeadCloser {
    /// A block tag (`{#if …}`, `{#each …}`, …): closes `}`, with the optional
    /// ` as item` / ` then value` / ` catch e` clause dropping ahead of it.
    BlockTag(Option<DocId>),
    /// An `{#each}` key's parens (`… as item (key)}`): closes `)`, and a key never has a
    /// clause of its own. The `}` that follows is the enclosing tag's, emitted by the block
    /// tag around this — so a dropped `)` lands at base and the `}` continues on its line.
    EachKey,
}

impl HeadCloser {
    /// Everything this head contributes, as one row per variant: the **token** it dangles,
    /// the **group id** the dangle's `if_break` reads (distinct per head — see
    /// [`GroupId::BlockKey`] for why sharing one is unsafe), and the **clause** that rides
    /// between the content and that token.
    ///
    /// One table rather than three parallel `match self` accessors: the caller wants all
    /// three at once, and a third head is then a single line instead of three edits that
    /// could disagree about which head is which.
    fn parts(self) -> (&'static str, GroupId, Option<DocId>) {
        match self {
            Self::BlockTag(clause) => ("}", GroupId::BlockHead, clause),
            Self::EachKey => (")", GroupId::BlockKey, None),
        }
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
/// rejection, so removing it would change semantics — see `conformance_prettier_svelte.md` §Svelte: Blocks.)
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

/// Where the awaited expression's leading/trailing-comment scan stops — the `{#await}` twin of
/// [`each_expr_comment_end`], and the same question: **how far into the head does this
/// expression's comment range reach?**
///
/// When a `then`/`catch` shorthand carries its binding pattern in the head, the range stops at
/// the pattern start (mirroring `{#each}`'s `context.span().start`) so a comment *inside* the
/// pattern isn't relocated out to trail the expression (`{#await p /* c */ then …}`) — the
/// comment-aware `build_pattern_doc` preserves it in place instead. The full form carries its
/// patterns in `{:then}`/`{:catch}` outside the head, so it keeps the head end.
///
/// ⚠️ **The head end is a CEILING on that narrowing, not merely its fallback.** A pattern the
/// head *folds in* need not be written there: the shorthand a `catch`-first authoring folds to
/// is `then`, whose binding sits in a `{:then}` clause AFTER the catch body
/// (`{#await p catch e}BODY{:then v}…`). A range reaching that pattern spans the body's own
/// emitted text, so every comment in the body printed twice — once relocated into the head —
/// which is the seam's standing hazard: a comment range must never span text another emitter
/// prints. `each_expr_comment_end` needs no such clamp only because an `{#each}` context is
/// always written in its own head; a third narrowing here owes the same argument.
/// Pinned by `svelte/blocks/await/catch_shorthand_body_comment_prettier_divergence`.
fn await_expr_comment_end(block: &internal::AwaitBlock<'_>) -> u32 {
    let head_end = block.opening_tag_span.end - 1;
    match await_shorthand(block) {
        AwaitShorthand::Then => block.value.as_ref().map_or(head_end, |v| v.span().start),
        AwaitShorthand::Catch => block.error.as_ref().map_or(head_end, |e| e.span().start),
        AwaitShorthand::None => head_end,
    }
    .min(head_end)
}

impl<'a> Printer<'a> {
    /// Whether a wrapped block head may dangle its `}` here. The head expression is
    /// allowed to break (`allow_wrapping` or a multiline context) AND the context permits
    /// the dangle — false only inside a whitespace-significant element (`<pre>` /
    /// `<textarea>`), gated by [`Printer::block_dangle_allowed`]. Gating it off only hugs
    /// the `}`; the expression still wraps to respect printWidth either way.
    fn block_head_can_wrap(&self, allow_wrapping: bool, in_multiline_context: bool) -> bool {
        (allow_wrapping || in_multiline_context) && self.block_dangle_allowed()
    }

    /// The shared head tail: wrap a head so its closing token dangles on its own line at the
    /// tag's base indent when the head wraps.
    ///
    /// `closer` names which head this is ([`HeadCloser`]) — a block tag closing `}` after its
    /// optional ` as …` / ` then …` / ` catch …` clause, or an `{#each}` **key** closing `)`.
    /// The two share this function because they share the rule: the head wraps ⇒ the closer
    /// drops to base, unless the content's last line already ends there with a `)`. A key that
    /// spelled the rule itself is exactly how it came to hug where the head it sits in dangles.
    ///
    /// `open` is the opening literal (`{#if `, `{#each `, `(`, …); `head` carries the
    /// already-built expression doc (the `{#each}` degenerate index/key form passes a concat of
    /// the expression plus its tail, so `head.doc` and `expr` are distinct) plus the freeze and
    /// trailing-line-comment verdicts; `expr` is the head expression, used only for
    /// the [`ends_at_base_closer`] classification. `can_wrap` stays caller-computed — its sources
    /// differ across builders and several reuse it afterward for the body-drop.
    ///
    /// When `can_wrap` is set (the same condition under which the expression is allowed to
    /// break), the head is grouped so the trailing break — and thus `}` — drops to its own line
    /// at the tag's base indent whenever the head exceeds print width (a deliberate
    /// `_prettier_divergence`, consistent with tsv's JS `if (⏎…⏎) {` and broken-element `>`).
    /// When the head fits, the break collapses and `}` hugs the head, byte-identical to the
    /// inline form. When `can_wrap` is false (inline context, `remove_lines` applied) the head is
    /// emitted flat with `}` hugged, unchanged.
    ///
    /// `head.ends_with_line_comment` short-circuits both paths: the comment's own `hardline`
    /// already drops the clause + `}` to the next line, so the dangle/hug break is skipped to
    /// avoid a spurious blank line. It comes from `head`, not from a second scan here: the
    /// content builder emitted that run and already knows. The two answers were always over the
    /// same range (every caller passes one `comment_end` to both), so this is the same verdict
    /// asked once.
    ///
    /// `head.frozen` drops the opening literal's trailing space (`Printer::head_open_doc`):
    /// the head's content begins with its own hardline, so the space would be trailing
    /// whitespace on the keyword's line. Everything below is unchanged — the frozen
    /// content's hardline breaks the head group, so the clause + `}` take the same dangle
    /// a width-wrapped head takes. A frozen head also never hugs: the hug rule reads the
    /// *printed* shape of the expression (a call whose args wrapped ends with `)` on its own
    /// line, so the clause continues that line), and a verbatim slice has no such shape to
    /// read — its last line is whatever the author left there.
    fn build_block_head(
        &self,
        open: &'static str,
        expr: &tsv_ts::Expression<'_>,
        head: HeadExpr,
        closer: HeadCloser,
        can_wrap: bool,
    ) -> DocId {
        let d = self.d();
        let HeadExpr {
            doc: expr_doc,
            frozen,
            ends_with_line_comment,
            // A block head builds its own final shape (`build_expression_doc_for_block`
            // applies the continuation indent in place), so there is never a debt here.
            owes_continuation_indent: _,
        } = head;
        let hug = ends_at_base_closer(expr) && !frozen;
        let open_doc = self.head_open_doc(open, frozen);
        let (close_text, group_id, clause) = closer.parts();
        let close = d.text(close_text);
        if ends_with_line_comment {
            // The trailing line comment leaves the break to whatever follows it, so the
            // clause + closer drop themselves to the next line at base indent — no dangle/hug
            // break beyond that. Still group the expression on the wrapping path so the
            // body-expand keyed to this group sees the (comment-forced) break.
            let head = if can_wrap {
                d.group_with_id(expr_doc, group_id)
            } else {
                expr_doc
            };
            return match clause {
                Some(c) => d.concat(&[open_doc, head, c, close]),
                None => d.concat(&[open_doc, head, close]),
            };
        }
        if can_wrap {
            // Key the breakable expression to this head's group. The group's `fits()`
            // counts the trailing clause + closer (they sit in rest-commands / resolve
            // flat during the fits test), so the head breaks at the right boundary;
            // reading anything keyed to `BlockHead` immediately after the group resolves
            // keeps that id nesting-safe (the key takes its own id — see
            // [`GroupId::BlockKey`] — because it does NOT get that read order).
            let grouped = d.group_with_id(expr_doc, group_id);
            if hug {
                // The expression renders ending with `)` on its own line at base (a
                // single call whose args wrapped). Per the layout rule, don't break a
                // line that starts with `)` — the clause + closer continue on it
                // (`) as item}`, `)}`, and a key's `))}`), in both the flat and broken head.
                match clause {
                    Some(c) => {
                        let space = d.text(" ");
                        d.concat(&[open_doc, grouped, space, c, close])
                    }
                    None => d.concat(&[open_doc, grouped, close]),
                }
            } else {
                // Binary chain / member chain / etc.: the clause + closer drop to their
                // own line at the tag's base indent when the head wraps (`expr⏎as item}`,
                // `expr⏎}`, a key's `expr⏎)`); when it fits they hug inline
                // (`expr as item}`).
                let hardline = d.hardline();
                let (break_tail, flat_tail) = match clause {
                    Some(c) => {
                        let space = d.text(" ");
                        (d.concat(&[hardline, c]), d.concat(&[space, c]))
                    }
                    None => (hardline, d.empty()),
                };
                let dangle = d.if_break_with_id(break_tail, flat_tail, group_id);
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
        // No clause-borne sub-head on this path: a `{#key}` / plain each / if / await /
        // snippet head holds its whole breakable part inside the head group, where
        // `will_break(head_doc)` sees it.
        self.build_expanding_construct(head_doc, false, inline_tail, multiline_tail, gt_prefix)
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
    /// `None` (every non-dangle caller) is a no-op.
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
    /// `conformance_prettier_svelte.md` §Svelte: Blocks). The `conditional_group` fits-tests
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
    ///
    /// ⚠️ `head_forced_break` is the part of that question `will_break(head_doc)` **cannot
    /// answer**, and it is not optional bookkeeping. A head whose breakable part rides in the
    /// *clause* — an `{#each}` key — has that clause duplicated into both arms of the
    /// dangle's `if_break`, and an `IfBreak` is opaque to `will_break` (its content is
    /// mode-dependent, so the query must say "no"). A key forced open by a leading `//` or
    /// by prettier's 3+-group chain rule therefore reaches here invisible, and the inline
    /// candidate wins by the same `fits()` short-circuit this arm exists to avoid — welding
    /// `{item}{/each}` onto the key's `)}` line. Every caller whose head has no clause-borne
    /// sub-head passes `false`.
    fn build_expanding_construct(
        &self,
        head_doc: DocId,
        head_forced_break: bool,
        inline_tail: DocId,
        multiline_tail: DocId,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        if head_forced_break || d.will_break(head_doc) || d.will_break(inline_tail) {
            return self.fold_gt(gt_prefix, true, d.concat(&[head_doc, multiline_tail]));
        }
        let inline = self.fold_gt(gt_prefix, false, d.concat(&[head_doc, inline_tail]));
        let expanded = self.fold_gt(gt_prefix, true, d.concat(&[head_doc, multiline_tail]));
        d.conditional_group(&[inline, expanded])
    }

    /// Whether every if-block branch (consequent, each `{:else if}` consequent,
    /// `{:else}`) is inline-authored — the precondition for the body-expand fast path.
    fn if_branches_all_inline(&self, block: &internal::IfBlock<'_>) -> bool {
        let mut all_inline = self.fragment_inline_authored(&block.consequent);
        let mut alt = block.alternate.as_ref();
        while let Some(a) = alt {
            if let Some(else_if) = Self::get_flattenable_else_if(a) {
                all_inline &= self.fragment_inline_authored(&else_if.consequent);
                alt = else_if.alternate.as_ref();
            } else {
                all_inline &= self.fragment_inline_authored(a);
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
            d.indent_hardline(body)
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
            self.build_section_body_doc(&block.consequent),
        )];
        let mut alt = block.alternate.as_ref();
        while let Some(a) = alt {
            if let Some(else_if) = Self::get_flattenable_else_if(a) {
                // Build the else-if head with wrapping enabled so it can dangle within the
                // expanded form; in the inline form `BlockHead` resolves flat (no dangle).
                let head_expr = self.build_else_if_expr_doc(else_if, true);
                let head = self.build_block_head(
                    ELSE_IF_BLOCK_OPEN,
                    &else_if.test,
                    head_expr,
                    HeadCloser::BlockTag(None),
                    self.block_dangle_allowed(),
                );
                let body = self.build_section_body_doc(&else_if.consequent);
                pieces.push(IfPiece::ElseIf { head, body });
                alt = else_if.alternate.as_ref();
            } else {
                pieces.push(IfPiece::Else(self.build_section_body_doc(a)));
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
        let head = self.build_block_head_expr(
            IF_BLOCK_OPEN,
            block.opening_tag_span,
            &block.test,
            block.opening_tag_span.end - 1,
            allow_wrapping || in_multiline_context,
        );

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            IF_BLOCK_OPEN,
            &block.test,
            head,
            HeadCloser::BlockTag(None),
            can_wrap,
        );

        // Inline-authored block (consequent + every alternate branch): expand the
        // whole block — bodies, `{:else if}`/`{:else}` sections, and `{/if}` — onto
        // their own lines when the head wraps (or the construct overflows).
        if self.if_branches_all_inline(block) {
            let pieces = self.build_if_pieces(block);
            let inline_tail = self.compose_if_tail(&pieces, false);
            let multiline_tail = self.compose_if_tail(&pieces, true);
            return self.build_expanding_construct(
                head_doc,
                // An if head has no clause, so nothing rides outside the head group.
                false,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        // A newline-authored branch breaks *every* boundary — the consequent, each
        // `{:else if}` / `{:else}`, and `{/if}` (hug is all-or-nothing; this path is
        // only reached when some branch is not inline-authored, so the construct always
        // expands). The consequent body is built only here — the fast path above builds
        // its own (shared) pieces, so building it eagerly made that path build the
        // consequent twice, keeping the nested-block fanout exponential.
        let body_doc = self.build_nodes_doc_multiline(block.consequent.nodes);
        let indented_body = self.indent_body_expand(body_doc, true);

        let mut parts: DocBuf = smallvec![head_doc, indented_body];

        if let Some(alt) = &block.alternate {
            parts.push(d.hardline());
            parts.push(self.build_if_alternate_doc(alt, in_multiline_context));
        }

        parts.push(d.hardline());
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
                FragmentNode::Text(t) if t.is_collapsible_ws_only => {
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
    ) -> HeadExpr {
        self.build_block_head_expr(
            ELSE_IF_BLOCK_OPEN,
            else_if.opening_tag_span,
            &else_if.test,
            else_if.opening_tag_span.end - 1,
            in_multiline_context,
        )
    }

    /// Build doc for if block alternate (else or else-if) on the expanded (non-fast)
    /// path — every branch body carries multiline line structure and drops to its own
    /// line. The expansion is construct-wide, *not* re-derived per branch, because hug
    /// is all-or-nothing: a branch that renders inline still drops to its own line once
    /// any sibling branch went multiline (see `fragment_inline_authored`).
    fn build_if_alternate_doc(&self, alt: &Fragment<'_>, in_multiline_context: bool) -> DocId {
        let d = self.d();
        // Check if this can be flattened to {:else if ...}
        if let Some(else_if) = Self::get_flattenable_else_if(alt) {
            // {:else if condition}
            let head_expr = self.build_else_if_expr_doc(else_if, in_multiline_context);

            let body_doc = self.build_nodes_doc_multiline(else_if.consequent.nodes);
            let indented_body = self.indent_body_expand(body_doc, true);

            // `build_else_if_expr_doc` builds the condition with `in_multiline_context`
            // as its wrapping flag, so the dangle keys on the same condition.
            let head_doc = self.build_block_head(
                ELSE_IF_BLOCK_OPEN,
                &else_if.test,
                head_expr,
                HeadCloser::BlockTag(None),
                in_multiline_context && self.block_dangle_allowed(),
            );
            let mut parts: DocBuf = smallvec![head_doc, indented_body];

            if let Some(nested_alt) = &else_if.alternate {
                parts.push(d.hardline());
                parts.push(self.build_if_alternate_doc(nested_alt, in_multiline_context));
            }

            return d.concat(&parts);
        }

        // Plain {:else}
        let body_doc = self.build_nodes_doc_multiline(alt.nodes);
        let indented_body = self.indent_body_expand(body_doc, true);

        d.concat(&[d.text("{:else}"), indented_body])
    }

    /// Whether the each block's body and its optional `{:else}` fallback are both
    /// inline-authored — the precondition for the body-expand fast path.
    fn each_branches_all_inline(&self, block: &internal::EachBlock<'_>) -> bool {
        let mut all_inline = self.fragment_inline_authored(&block.body);
        if let Some(fallback) = &block.fallback {
            all_inline &= self.fragment_inline_authored(fallback);
        }
        all_inline
    }

    /// Build the each-block's body and optional `{:else}` fallback **once**
    /// (mode-agnostic), for composition into both expanding-construct tails by
    /// [`Self::compose_each_tail`] — so a nested body is built once rather than once
    /// per form (a per-form build would rebuild it twice, compounding to O(2^depth)).
    fn build_each_pieces(&self, block: &internal::EachBlock<'_>) -> (DocId, Option<DocId>) {
        let body = self.build_section_body_doc(&block.body);
        let fallback = block
            .fallback
            .as_ref()
            .map(|f| self.build_section_body_doc(f));
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

    /// Build an `{#each}` key's whole ` (…)` doc — `None` when the block has no key.
    ///
    /// A key is a **head of its own**: its `(` opens the gap and its `)` is a dangling
    /// closer, so it routes through the same assembler the block tag does
    /// ([`HeadCloser::EachKey`]) and answers the closer rule the same way — the key wraps ⇒
    /// `)` drops to the tag's base indent and the tag's own `}` continues on that line
    /// (`⏎)}`), unless the key's expression already ends on a dedented closer
    /// ([`ends_at_base_closer`]), which puts both on its line (`))}`).
    ///
    /// Three shapes decline that and take the plain prefixed-head assembly instead:
    ///
    /// - **No `key_span`** — nothing locates the parens, so there is no head to assemble.
    /// - **A frozen key** — its content is a verbatim slice that already begins with its own
    ///   hardline, so the `)` break is unconditional rather than a width verdict. It is also
    ///   the one shape that can meet `can_wrap == false` (an `{#each}` behind a breakable
    ///   sibling) and still need the break, which the dangling assembler has no group to
    ///   hang on.
    /// - **The degenerate no-`as` form** (`{#each xs, i (key)}` — Svelte's parser accepts it,
    ///   its compiler rejects it) — with no clause to sit behind, the key is not a sibling of
    ///   the head expression but part of it, concatenated into the head group by the caller.
    ///   A key that breaks there already breaks that group, so the tag's `}` is the closer
    ///   drop the rule asks for and a second one would stack two closer lines (`…cccc⏎)⏎}`).
    ///
    /// `can_wrap` is the **block's** own verdict rather than the head-expression build flag:
    /// the two differ inside a whitespace-sensitive element, where the expression still wraps
    /// for width but no closer may dangle. (That element has its own builder, so the gate is
    /// inert here — it is spelled to keep the key and its tag one answer.)
    fn build_each_key_doc(
        &self,
        block: &internal::EachBlock<'_>,
        allow_wrapping: bool,
        in_multiline_context: bool,
    ) -> Option<DocId> {
        let key = block.key.as_ref()?;
        // The key expression is inside parens, so the offset accounts for that.
        let Some(key_span) = block.key_span else {
            // No `key_span`: build the doc directly (still a braced head — the key parens
            // hug, so a leading cast reflows).
            return Some(self.build_ts_expression_doc_cannot_hang(key));
        };
        let key_head = self.build_expression_doc_for_block(
            key,
            key_span.start + 1, // after "("
            key_span.end - 1,   // before ")"
            1,                  // "(" = 1 char (key is inside parens)
            allow_wrapping || in_multiline_context,
        );
        if key_head.frozen || block.context.is_none() {
            return Some(self.build_prefixed_head_doc("(", key_head, ")"));
        }
        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        Some(self.build_block_head("(", key, key_head, HeadCloser::EachKey, can_wrap))
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
        let head = self.build_block_head_expr(
            EACH_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            expr_comment_end,
            allow_wrapping || in_multiline_context,
        );

        let key_doc = self.build_each_key_doc(block, allow_wrapping, in_multiline_context);

        // Whether the key is forced open regardless of width — a leading `//`, or prettier's
        // 3+-group member-chain rule. The body-expand cannot read this off `head_doc`: the key
        // rides in the `as` clause, which the dangle duplicates into both arms of an
        // `if_break`, and `will_break` is opaque to those (see `build_expanding_construct`).
        // Asked HERE, where the key doc is still in hand and still outside that wrapper.
        //
        // The degenerate no-`as` form needs no such verdict: with no clause to ride, its key
        // is concatenated into the head expression's own doc below, inside the head group,
        // where `will_break(head_doc)` already reaches it.
        let key_forced_break =
            block.context.is_some() && key_doc.is_some_and(|kd| d.will_break(kd));

        // Separate the breakable expression from its clause so the clause + `}` can
        // dedent together onto their own line when the head wraps. `clause` is the
        // `as pattern[, index][ (key)]` tail WITHOUT its leading space (added by
        // `build_block_head`); the degenerate index/key-without-`as` cases (not
        // valid Svelte) keep hugging the expression unchanged.
        let (head_expr, clause) = if let Some(context) = &block.context {
            let mut clause_parts: DocBuf = smallvec![d.text("as ")];
            clause_parts.push(self.build_pattern_doc(context));
            if let Some(index) = block.index {
                clause_parts.push(d.text(", "));
                clause_parts.push(d.text_pooled(index));
            }
            if let Some(kd) = key_doc {
                clause_parts.push(d.text(" "));
                clause_parts.push(kd);
            }
            (head, Some(d.concat(&clause_parts)))
        } else {
            // No `as`: any index/key is degenerate — keep it hugging the expression.
            let mut e: DocBuf = smallvec![head.doc];
            if let Some(index) = block.index {
                e.push(d.text(", "));
                e.push(d.text_pooled(index));
            }
            if let Some(kd) = key_doc {
                e.push(d.text(" "));
                e.push(kd);
            }
            (
                HeadExpr {
                    doc: d.concat(&e),
                    ..head
                },
                None,
            )
        };

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            EACH_BLOCK_OPEN,
            &block.expression,
            head_expr,
            HeadCloser::BlockTag(clause),
            can_wrap,
        );

        // Inline-authored block (body + `{:else}` fallback): expand the body,
        // `{:else}` section, and `{/each}` onto their own lines when the head wraps
        // (or the construct overflows).
        if self.each_branches_all_inline(block) {
            let (body, fallback) = self.build_each_pieces(block);
            let inline_tail = self.compose_each_tail(body, fallback, false);
            let multiline_tail = self.compose_each_tail(body, fallback, true);
            return self.build_expanding_construct(
                head_doc,
                key_forced_break,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        // A newline-authored branch breaks *every* boundary — the body, `{:else}`, and
        // `{/each}` (hug is all-or-nothing; this path is only reached when some branch
        // is not inline-authored, so the construct always expands). Bodies are built
        // only here — the fast path above builds its own shared pieces, so building
        // them eagerly would build each body twice (the nested-block fanout).
        let body_doc = self.build_nodes_doc_multiline(block.body.nodes);
        let indented_body = self.indent_body_expand(body_doc, true);

        let mut parts: DocBuf = smallvec![head_doc, indented_body];

        if let Some(fallback) = &block.fallback {
            parts.push(d.hardline());
            parts.push(d.text("{:else}"));
            let fallback_doc = self.build_nodes_doc_multiline(fallback.nodes);
            parts.push(self.indent_body_expand(fallback_doc, true));
        }

        parts.push(d.hardline());
        parts.push(d.text("{/each}"));
        // Non-expanding tail (authored-multiline body): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&parts))
    }

    /// Whether a fragment is inline-authored — no **newline-authored** boundary and no
    /// forced break — the precondition for the body-expand fast path on `{#if}` /
    /// `{#each}` / `{#await}` / `{#key}` / `{#snippet}` bodies, branches, and sections.
    ///
    /// A **space-only** boundary does NOT disqualify: it is render-free (the compiler
    /// trims every fragment edge at compile; only *inter-sibling* whitespace and
    /// `<pre>`/`<textarea>` are significant), so it neither survives inline nor selects
    /// the layout — the fast path trims it away (`build_section_body_doc`). Only a
    /// boundary whose whitespace run contains a newline keeps its meaning (the
    /// construct stays multiline). See conformance_prettier_svelte.md §Svelte: Blocks.
    ///
    /// The verdict drives the whole construct, and the expansion it gates is
    /// **all-or-nothing**: on the non-fast paths every branch body + marker + close
    /// drops to its own line — keying each side on its own authored whitespace would
    /// let a render-free character weld the body to one tag while the other breaks
    /// (`{#if c}⏎<div>a</div>{/if}`), the block analogue of a delimiter dangle, and a
    /// branch that renders inline still breaks once any sibling branch went multiline
    /// (`{:else}` never welds its body while `{#if}` holds its own). The same
    /// invariant `ElementLayout::WithContent(BoundaryMode)` encodes for an element's
    /// content boundary — see conformance_prettier_svelte.md §Svelte: Inline content
    /// block-style. `<pre>`/`<textarea>` never reach here — they are dispatched to
    /// the whitespace-sensitive builder, where the boundary is literal and the hug
    /// mandatory.
    fn fragment_inline_authored(&self, frag: &Fragment<'_>) -> bool {
        !self.fragment_boundary_newline(frag, true)
            && !self.fragment_boundary_newline(frag, false)
            && !self.fragment_should_force_break_content(frag.nodes)
    }

    /// Build a block-section body for the inline/expanding fast path: boundary
    /// whitespace **trimmed** (a space-only section boundary is render-free, so it never
    /// survives inline — see conformance_prettier_svelte.md §Svelte: Blocks), content otherwise
    /// the shared prettier-shaped builder. One builder for a glued and a space-only
    /// authoring of the same body, so both reach one fixed point by construction.
    ///
    fn build_section_body_doc(&self, fragment: &Fragment<'_>) -> DocId {
        self.build_nodes_doc_trimmed(fragment.nodes, MultilineCause::None)
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
    /// tails by [`Self::compose_await_tail`]. Bodies are the boundary-trimmed
    /// [`Self::build_section_body_doc`]; the per-mode indent wrapping is applied at
    /// composition.
    fn build_await_pieces(&self, block: &internal::AwaitBlock<'_>) -> AwaitPieces {
        let (is_then_shorthand, is_catch_shorthand) = Self::await_shorthand_flags(block);
        AwaitPieces {
            pending: block
                .pending
                .as_ref()
                .map(|p| self.build_section_body_doc(p)),
            then_kw: (!is_then_shorthand)
                .then(|| self.await_then_keyword(block))
                .flatten(),
            // An empty-body `:then` is dropped (marker + body); keep only a content body.
            then_body: block
                .then
                .as_ref()
                .filter(|_| then_has_content(block))
                .map(|t| self.build_section_body_doc(t)),
            catch_kw: (!is_catch_shorthand)
                .then(|| self.await_catch_keyword(block))
                .flatten(),
            catch_body: block.catch.as_ref().map(|c| self.build_section_body_doc(c)),
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

    /// Build the await tail for the **newline-authored** layout: section bodies via
    /// `build_await_section_body`, with a `hardline` before each keyword and before
    /// `{/await}` when the construct expands. Mirrors `compose_await_tail`; `expand` is
    /// construct-wide (hug is all-or-nothing — see `fragment_inline_authored`), so every
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
    pub(super) fn build_await_block_doc(&self, block: &internal::AwaitBlock<'_>) -> DocId {
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
        let head = self.build_block_head_expr(
            AWAIT_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            await_expr_comment_end(block),
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
        // A space-only section is inline-authored (its boundary is render-free and gets
        // trimmed by `build_section_body_doc`); only a newline-authored section falls
        // through to the newline tail below.
        let all_sections_inline = sections
            .iter()
            .filter_map(|f| f.as_ref())
            .all(|f| self.fragment_inline_authored(f));
        // Uniform body-drop: when every present section is inline-authored, the body +
        // `{:then}`/`{:catch}` keywords + `{/await}` drop to their own lines on overflow.
        // Keyed on `can_wrap` — the same gate `{#if}`/`{#each}` use — so the body hugs in the
        // inline-content path (`can_wrap` false) but drops in the multiline-fragment
        // path. A block-parent sibling routes await through the multiline path via
        // `has_control_flow_after_sibling` (so `can_wrap` is true there); an inline parent
        // keeps `can_wrap` false and hugs, matching `{#if}`/`{#each}`.
        // Shorthand clause lives in the head: `then v` / bare `then`, or `catch e` / bare
        // `catch`; the full form has none. Built once, shared by the fast path and the
        // newline-authored tail. Classified by `await_shorthand`, the same source
        // `await_shorthand_flags` uses to skip the head-carried keyword.
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
            head,
            HeadCloser::BlockTag(clause),
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
                // An await clause is a binding pattern — it cannot break on its own.
                false,
                inline_tail,
                multiline_tail,
                gt_prefix,
            );
        }

        // A newline-authored section breaks *every* boundary — each section body, the
        // `{:then}` / `{:catch}` keywords, and `{/await}` (hug is all-or-nothing, see
        // `fragment_inline_authored`). Here `!all_sections_inline` is false only for a
        // section-less await (`{#await p}{/await}` — every fragment empty), which stays
        // inline: nothing forces expansion.
        let expand = !all_sections_inline;
        let tail = self.build_await_tail_newline(block, expand);
        // Non-expanding tail (newline-authored sections): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&[head_doc, tail]))
    }

    /// Build a doc for a key block (no preceding context / sibling `>`).
    ///
    /// Uses same inline/multiline pattern as if blocks.
    pub(super) fn build_key_block_doc(&self, block: &internal::KeyBlock<'_>) -> DocId {
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
        let head = self.build_block_head_expr(
            KEY_BLOCK_OPEN,
            block.opening_tag_span,
            &block.expression,
            block.opening_tag_span.end - 1,
            allow_wrapping || in_multiline_context,
        );

        let can_wrap = self.block_head_can_wrap(allow_wrapping, in_multiline_context);
        let head_doc = self.build_block_head(
            KEY_BLOCK_OPEN,
            &block.expression,
            head,
            HeadCloser::BlockTag(None),
            can_wrap,
        );
        let close = d.text("{/key}");

        // Inline-authored (no newline-authored boundary, no forced break; a space-only
        // boundary is render-free and gets trimmed by the body builder): expand the
        // body + `{/key}` when the head wraps (or the construct overflows).
        if self.fragment_inline_authored(&block.fragment) {
            let body_doc = self.build_section_body_doc(&block.fragment);
            return self.build_expanding_block(head_doc, body_doc, close, gt_prefix);
        }

        // Newline-authored body: it and `{/key}` each keep their own line.
        let body_doc = self.build_nodes_doc_multiline(block.fragment.nodes);
        let parts: DocBuf = smallvec![
            head_doc,
            self.indent_body_expand(body_doc, true),
            d.hardline(),
            close,
        ];
        // Non-expanding tail (authored-multiline body): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&parts))
    }

    /// Build a doc for a snippet block (no sibling `>` to fold).
    pub(super) fn build_snippet_block_doc(&self, block: &internal::SnippetBlock<'_>) -> DocId {
        self.build_snippet_block_doc_with_full_context(block, None)
    }

    /// Build a doc for a snippet block, optionally folding a preceding sibling's `>`.
    ///
    /// Uses same inline/multiline pattern as if blocks. Opening tag uses group() for
    /// parameter wrapping when they exceed print width. Takes no context: the head wraps by
    /// its own width (its `BlockHead` group), and the body-drop is likewise decided by
    /// **width** (the `conditional_group` in `build_expanding_block`) — never by whether the
    /// head may wrap, which would let a render-free boundary select the layout (see
    /// `fragment_inline_authored`).
    pub(super) fn build_snippet_block_doc_with_full_context(
        &self,
        block: &internal::SnippetBlock<'_>,
        gt_prefix: Option<DocId>,
    ) -> DocId {
        let d = self.d();
        // Inline-authored = no newline-authored boundary and no forced break; a
        // space-only boundary is render-free and gets trimmed by the body builder.
        let is_inline = self.fragment_inline_authored(&block.body);

        // Type parameters (generics). They route through tsv_ts's type-parameter printer
        // (constraints, defaults, modifiers, interior comments, width-based wrapping of a
        // long generic list — its own group, so it breaks independently of the parameter
        // list). A head the parser could not read never gets here: it is a parse error, so
        // `type_parameters` is `Some` whenever the source wrote a `<…>`.
        let type_params_part = if let Some(decl) = &block.type_parameters {
            tsv_ts::build_type_parameters_doc_with_comments(d, decl, &self.ts_inputs(), &self.embed)
        } else {
            d.empty()
        };

        // Parameter list `(…)`. The parens fold so that when they wrap, `)` dedents to
        // base and `}` hugs it (`)}`) — no dangle (no trailing comma; trailingComma:
        // 'none').
        //
        // Parameters route through the same comment-aware, `FunctionParameter`-context
        // printer a real function signature uses, so interior comments (`{ a = /* c */ 1 }`),
        // boundary comments (`a /* c */, b`), the single-pattern hug, and nesting-depth
        // expansion all match a standalone parameter list.
        // `build_function_params_doc_with_comments` emits the `(…)` with no group of its
        // own — the `group` below drives the wrap.
        let params_inner = match block.params_paren {
            Some(paren) => tsv_ts::build_function_params_doc_with_comments(
                d,
                block.parameters,
                Some(paren.start),
                Some(paren.end),
                &self.ts_inputs(),
                &self.embed,
            ),
            None => d.text("()"),
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

        // Inline-authored body (boundary-trimmed): expand the body + `{/snippet}` onto
        // their own lines when the construct overflows (params wrap, or head + body
        // exceeds width) — uniformly, including paramless snippets. Keyed to the
        // opening group above.
        if is_inline {
            let body_doc = self.build_section_body_doc(&block.body);
            let close = d.text("{/snippet}");
            return self.build_expanding_block(opening_doc, body_doc, close, gt_prefix);
        }

        // Newline-authored body: it and `{/snippet}` each keep their own line.
        let body_doc = self.build_nodes_doc_multiline(block.body.nodes);
        let parts: DocBuf = smallvec![
            opening_doc,
            self.indent_body_expand(body_doc, true),
            d.hardline(),
            d.text("{/snippet}"),
        ];
        // Non-expanding tail (authored-multiline body): fold a preceding sibling's `>`.
        self.dangle_gt(gt_prefix, d.concat(&parts))
    }
}
