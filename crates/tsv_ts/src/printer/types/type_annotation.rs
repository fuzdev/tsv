// Type annotation printing for TypeScript
//
// Handles printing of type annotations (`: Type`) with various contexts:
// - Simple type annotations
// - Width-aware wrapping for type arguments
// - Return type annotations

use super::helpers::{TypeParenRule, type_args_should_wrap_for_return_type, unwrap_parenthesized};
use super::{CommentSpacing, Printer, TrailingBlock, UnionValueDoc};
use crate::ast::internal::{self, TSType};
use crate::printer::layout::hang_after_operator;
use smallvec::smallvec;
use tsv_lang::doc::DocBuf;
use tsv_lang::doc::arena::DocId;

/// Which pair, if any, an annotation's **position** requires around its type — over and
/// above the parens the author wrote, which the type's own doc already prints.
///
/// The annotation emitters below build their value in three SHAPES, and the pair has to
/// reach all three or it is present at one arm and missing at another:
///
/// - an ordinary **type doc** — [`Printer::build_annotation_value_doc`], which every
///   value-building site asks instead of `build_type_doc` directly (the comment-free fast
///   path, the broke-after leading run, the plain fall-through, the `:`-gap hang);
/// - a **frozen** verbatim slice — [`frozen_annotation_parens`], the slicing rule, since
///   a source paren the freeze would drop as redundant is the very pair required here;
/// - a shell-**stripped** inner — [`Printer::wrap_annotation_required_pair`], the one
///   shape neither of the above covers, because the shell it would have kept is what was
///   stripped.
///
/// The hand-rolled `": (" + build_type_doc(inner) + ")"` the arrow callsite used instead
/// was none of them: an alternate-layout builder that reassembled the annotation from
/// parts and therefore ran no gap lookup at all, dropping every comment in the `:`→type
/// gap and in the authored shell ([`comments.md`](../../../../docs/comments.md)
/// hazard 4).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::printer) enum AnnotationParens {
    /// The type prints exactly the parens the author wrote — every annotation position
    /// but the one below.
    AsWritten,
    /// An **arrow's return type**, where a function type needs the disambiguating pair:
    /// `(x: T): ((y: T) => T) =>`, never `(x: T): (y: T) => T =>`, whose second `=>`
    /// would read as the arrow's own.
    ArrowReturn,
}

/// The [`AnnotationParens::ArrowReturn`] predicate, in the `fn` shape
/// [`Printer::build_required_paren_pair_operand_doc`] takes. Reads through the author's
/// shell: `(x: T): ((y: T) => T) =>` needs the pair just as the shell-less spelling does,
/// and the shell is redundant *as a type* — only this position requires it.
fn arrow_return_needs_parens(_: &Printer<'_>, ty: &TSType<'_>) -> bool {
    matches!(unwrap_parenthesized(ty), TSType::Function(_))
}

/// Never — the annotation `:` gap's ordinary answer to "does this frozen child need a
/// pair of its own?", the `member_parens` predicate [`Printer::build_frozen_head_doc`]
/// takes. A source paren after `:` is redundant *as a type* and drops under the freeze;
/// this is exactly what [`Printer::build_frozen_single_child_doc`] hardcodes, named here
/// so the position can select it beside the arrow-return rule.
fn no_frozen_parens(_: &Printer<'_>, _ty: &TSType<'_>) -> bool {
    false
}

/// The freeze's paren rule for an annotation position — the `member_parens` predicate
/// [`Printer::build_frozen_head_doc`] slices by, so a frozen arrow return type keeps (or
/// re-synthesizes) the pair the grammar requires instead of dropping it as redundant.
///
/// A freeze must not unmake a pair the grammar requires: with the ordinary rule,
/// `: // prettier-ignore⏎((y: T) => T)` froze to `(y: T) => T` and the arrow's own `=>`
/// bound the reparse the other way.
fn frozen_annotation_parens(parens: AnnotationParens) -> TypeParenRule {
    match parens {
        AnnotationParens::AsWritten => no_frozen_parens,
        AnnotationParens::ArrowReturn => arrow_return_needs_parens,
    }
}

impl<'a> Printer<'a> {
    /// Build an annotation's value doc, adding the pair the POSITION requires around it.
    ///
    /// The required pair routes through [`Self::build_required_paren_pair_operand_doc`],
    /// so an authored shell that prints its own pair — one retained for a trailing `//`,
    /// or opened over a leading one — is not double-wrapped, and the shell's leading and
    /// trailing gaps keep their own emitter. `build_type_doc` alone would strip the
    /// redundant shell and print no pair at all, which is why the arrow callsite had to
    /// synthesize one.
    pub(in crate::printer) fn build_annotation_value_doc(
        &self,
        ty: &TSType<'_>,
        parens: AnnotationParens,
    ) -> DocId {
        match parens {
            AnnotationParens::AsWritten => self.build_type_doc(ty),
            AnnotationParens::ArrowReturn => {
                self.build_required_paren_pair_operand_doc(ty, arrow_return_needs_parens)
            }
        }
    }

    /// Add the position's required pair around a **shell-stripped** value.
    ///
    /// The strip's counterpart to [`Self::build_annotation_value_doc`]: an in-shell
    /// directive routes its inner out of the shell that held it, and the resulting doc
    /// is not a type doc, so the shell-aware routing inside
    /// `build_required_paren_pair_operand_doc` has nothing to read. The *freeze* answers
    /// the same question through its slice instead ([`frozen_annotation_parens`]); this
    /// is the one arm no slice covers, because the shell it would have kept is exactly
    /// what was stripped.
    fn wrap_annotation_required_pair(&self, value: DocId, parens: AnnotationParens) -> DocId {
        let d = self.d();
        match parens {
            AnnotationParens::AsWritten => value,
            AnnotationParens::ArrowReturn => d.concat(&[d.text("("), value, d.text(")")]),
        }
    }

    /// Build a Doc for a type annotation (e.g., `: number`)
    ///
    /// Handles comments between the colon and the type. For a line comment
    /// between `:` and the type, the comment stays inline after `:` with a
    /// hardline before the type (`: // c\n T`) so the line comment doesn't
    /// swallow what follows. Union types additionally INDENT the type; other
    /// types are not indented.
    /// The crate-public seam for [`build_type_annotation_doc`]: `tsv_svelte` needs
    /// it for a **destructuring** block binding pattern, whose braces it builds on
    /// its own comment-preserving path and which therefore has to append the `: T`
    /// tail explicitly.
    ///
    /// [`build_type_annotation_doc`]: Self::build_type_annotation_doc
    pub(crate) fn build_type_annotation_doc_public(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        self.build_type_annotation_doc(annotation)
    }

    /// The `: Type` doc for every annotation position but an arrow's return type — the
    /// [`AnnotationParens::AsWritten`] entry to [`Self::build_type_annotation_doc_parens`].
    pub(in crate::printer) fn build_type_annotation_doc(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        self.build_type_annotation_doc_parens(annotation, AnnotationParens::AsWritten)
    }

    /// [`Self::build_type_annotation_doc`] with the position's required pair
    /// ([`AnnotationParens`]) — the arrow return type's entry, and the only caller that
    /// passes anything but [`AnnotationParens::AsWritten`].
    fn build_type_annotation_doc_parens(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
        parens: AnnotationParens,
    ) -> DocId {
        let d = self.d();
        // Check for comments between `:` and the type
        let colon_end = annotation.span.start + 1; // After the `:`
        // A redundant paren shell with a leading line-comment run (`: (// c\n T)`) strips
        // to the same continuation-indent hang as bare `: // c\n T`; substitute the
        // unwrapped inner (`ty`) and widen the gap window (`type_start`) to its start so
        // the line-comment branch below fires — the outer paren would otherwise hide the
        // comment and the inline path would relocate it at a differing indent (a
        // non-idempotency). A mixed (leading block) or trailing shell hoists losslessly
        // too — the leading run via the continuation indent, the trailing comment via
        // `with_stripped_paren_trailing`. A shell with no leading line comment is returned
        // unchanged, so the block-comment / no-comment paths below preserve it in place.
        // A shell one SUFFIX down (`: (⏎// c⏎A)[]`) widens the window too, but hands the
        // value back whole — there is nothing to substitute — and names its own leading
        // run as this gap's to emit (`hang.claimed_shell`).
        let hang = self.keyword_value_stripped_paren_hang(annotation.type_annotation);
        let (type_start, ty) = (hang.value_start, hang.value_type);

        // Zero-comment gate over the `:`→type gap, computed once and reused by every
        // arm below (the union arm and the simple fall-through ask for this exact
        // range; a type annotation is among the most frequent TS constructs and a
        // comment inside one is rare). It also subsumes the line-comment check that
        // follows: ownership only ever binds a *block* comment (`owned ⇒ is_block`),
        // so a line comment is always in the to-emit set and no gap without to-emit
        // comments can hold one. The wrapping sibling
        // (`build_type_annotation_doc_with_wrapping`) already hoists this way.
        let gap_has_comments = self.has_comments_to_emit_between(colon_end, type_start);

        // Check if there's a line comment between : and the type
        if gap_has_comments && self.has_line_comments_between(colon_end, type_start) {
            // An own-line format-ignore directive in the gap takes the directive builder,
            // which freezes the value. Its placement is the one the emitter below gives any
            // own-line comment: both keep the directive's line, since a head-trailing
            // directive is inert under the placement classification and the relocated form
            // would lose the freeze on the second pass. Routed for
            // composite children too: a union child freezes via its own leading-run
            // walk, but the annotation owns the directive's emission either way. An
            // in-shell directive (`: (⏎// prettier-ignore⏎ T)`) routes the same way —
            // the shell strips and the paren-stripped inner freezes
            // (`paren_interior_routed_inner`).
            if self.member_gap_frozen(colon_end, annotation.type_annotation.span().start)
                || self
                    .paren_interior_routed_inner(annotation.type_annotation)
                    .is_some()
            {
                return self.build_annotation_own_line_directive_doc(annotation, parens);
            }
            // The `:`→type gap is a value gap, so a line comment hangs the type through the
            // keyword→value emitter (`append_keyword_value_line_comments`): a comment on the
            // `:` line trails it, an own-line comment keeps its own line — it LEADS the type,
            // so own-line-ness is authorship — and the type drops one indent level, as at
            // `keyof` and the function type's `=>`. Each line comment terminates at
            // end-of-line — otherwise a following comment (or the type) is swallowed into its
            // text (`// a // b` reparses as one comment: a content loss). Uniform across
            // union, intersection, and simple types — see conformance_prettier.md §Uniform
            // forced-continuation indent. (Prettier pulls an own-line comment up and leaves
            // an intersection or simple type flush, so this diverges for those.)
            //
            // A *block* comment in this gap is handled by the else branch below, NOT
            // here: a newline-broken block compacts to the inline value-side position
            // (`a: /* c */ X`) rather than hanging — a deliberate, cataloged choice
            // (annotation_leading_block_prettier_divergence).
            // Type position: a trailing block lifted from the shell trails the type
            // inline before the terminator.
            let type_doc = self.with_claimed_shell_leading_run(hang.claimed_shell, || {
                self.build_hang_value_doc_parens(
                    annotation.type_annotation,
                    ty,
                    TrailingBlock::Inline,
                    parens,
                )
            });
            let mut parts: DocBuf = smallvec![d.text(":")];
            self.append_keyword_value_line_comments(&mut parts, colon_end, type_start, type_doc);
            d.concat(&parts)
        } else {
            // Handle unions/intersections with width-based breaking
            // Short: `param: Type1 | Type2`
            // Long: `param:\n\t| Type1\n\t| Type2`
            // Hugged: `param: {\n\t…\n} | null` (the union seam decides; see below)
            //
            // This pattern matches index signature type annotation handling.
            // For a non-hugging union and for intersections, wrap in group + indent + line
            // so they break after `:` and inherit breaking from this context's group.
            // Redundant comment-free parens are stripped first so `(A | B)` / `(A & B)`
            // get the bare layout (prettier strips them too); other parens keep the `_`
            // fall-through.
            let value_type = self.unwrap_redundant_parens(ty);
            match value_type {
                TSType::Union(u) => {
                    // The one union seam for every annotation position: a hugging
                    // union (`{ … } | null`) keeps `: ` glued — a parameter's annotation
                    // hugs exactly as a variable's or a property's does — and any other
                    // union hangs after the `:`.
                    self.build_annotation_union_doc(colon_end, type_start, u, gap_has_comments)
                }
                TSType::Intersection(i) => {
                    // Build intersection with proper indentation for type annotation context:
                    // `: FirstType &` stays on the same line, continuation types are indented
                    // Extract comments between `:` and the intersection first
                    self.build_intersection_type_annotation_doc(i, colon_end)
                }
                _ => self.build_simple_type_annotation_doc(
                    colon_end,
                    type_start,
                    ty,
                    gap_has_comments,
                    parens,
                ),
            }
        }
    }

    /// Emit `: <comments> <value>` for an annotation whose head→type gap holds an
    /// own-line format-ignore directive. The whole run keeps its own-line placement
    /// (`append_keyword_value_line_comments` — same-line comments trail `:`, own-line
    /// comments and the value drop into the indent), and a non-composite type is
    /// frozen verbatim (`single_child_frozen`); a union/intersection child builds
    /// normally and freezes via its own leading-run walk (Rule A). An IN-SHELL
    /// directive (`paren_interior_routed_inner`) strips the shell — the window widens to the
    /// paren-stripped inner's start, handing the whole leading run to the emitter —
    /// and freezes the inner, converging in one pass to the fixed point the bare
    /// authoring holds; trailing shell-gap comments are lifted after the value
    /// (lossless strip).
    ///
    /// Neither frozen arm is a type doc — one is a verbatim source slice, the other a
    /// shell-STRIPPED inner — so `build_annotation_value_doc`'s shell-aware routing has
    /// nothing to read and the position's required pair has to reach them another way:
    /// the freeze slices by [`frozen_annotation_parens`], and the strip re-adds the pair
    /// through [`Self::wrap_annotation_required_pair`]. Without both, an arrow return
    /// type printed `: // prettier-ignore⏎(y: T) => T =>`, whose second `=>` the reparse
    /// reads as the arrow's own — a freeze that unmakes the disambiguation it froze.
    fn build_annotation_own_line_directive_doc(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
        parens: AnnotationParens,
    ) -> DocId {
        let d = self.d();
        let colon_end = annotation.span.start + 1;
        let child = annotation.type_annotation;
        let (gap_end, value_doc) = if self.single_child_frozen(colon_end, child) {
            (
                child.span().start,
                self.build_frozen_head_doc(child, frozen_annotation_parens(parens)),
            )
        } else if let Some(inner) = self.paren_interior_routed_inner(child) {
            let inner_doc = self.build_routed_child_doc(inner);
            let stripped =
                self.with_stripped_paren_trailing(inner_doc, child, inner, TrailingBlock::Inline);
            (
                inner.span().start,
                self.wrap_annotation_required_pair(stripped, parens),
            )
        } else {
            (
                child.span().start,
                self.build_annotation_value_doc(child, parens),
            )
        };
        let mut parts: DocBuf = smallvec![d.text(":")];
        self.append_keyword_value_line_comments(&mut parts, colon_end, gap_end, value_doc);
        d.concat(&parts)
    }

    /// Emit `: <block-comments> <type>` for a simple annotation — the fall-through
    /// shared by `build_type_annotation_doc`'s `_` match arm and
    /// `build_type_annotation_doc_with_wrapping` (once its wrapping-TypeReference /
    /// Union / Intersection branches are ruled out). Block comments in the `:`→type
    /// gap stay inline (`: /* c */ Type`). Takes the caller's already-computed
    /// `colon_end` / `type_start` so neither re-derives them, and the raw `ty` (not an
    /// unwrapped form) so redundant parens like `: (string)` are preserved.
    ///
    /// `gap_has_comments` is the caller's answer for the `:`→type gap, so a caller that
    /// already knows the whole annotation is comment-free spends no search here at all.
    fn build_simple_type_annotation_doc(
        &self,
        colon_end: u32,
        type_start: u32,
        ty: &TSType<'_>,
        gap_has_comments: bool,
        parens: AnnotationParens,
    ) -> DocId {
        let d = self.d();
        // Skip the `empty()` comment child on the comment-free `: Type` gap — type
        // annotations are one of the most frequent TS constructs, so a wasted child here
        // (walked by render + every fits pass) is ubiquitous. Byte-identical: the gap is
        // comment-free, so the comment doc would be `empty()`. A pair, not a buffer.
        if !gap_has_comments {
            return d.concat(&[d.text(": "), self.build_annotation_value_doc(ty, parens)]);
        }
        let mut parts: DocBuf = smallvec![d.text(": ")];
        // A glued format-ignore directive in the gap freezes a non-composite type
        // verbatim (`let v: /* format-ignore */ {x:   1}` — the directive itself is
        // emitted inline; own-line directives took the own-line branch). The slice
        // keeps the position's required pair (`frozen_annotation_parens`), as at the
        // own-line arm.
        if self.single_child_frozen(colon_end, ty) {
            parts.push(self.build_comments_between(
                colon_end,
                type_start,
                CommentSpacing::Trailing,
            ));
            parts.push(self.build_frozen_head_doc(ty, frozen_annotation_parens(parens)));
            return d.concat(&parts);
        }
        // A block run the author broke AFTER, before a type that actually breaks
        // (`let x: /* c */⏎{ …multiline… }`) or holding a multi-line comment
        // (`let x: /* c⏎d */⏎B`): the run keeps the head line and the
        // type opens on the next, un-indented — prettier's `printLeadingComment`
        // newline-after `line`, materialized by that forced break (the
        // annotation adds no indent group, so the type lands at the statement's
        // level). A type that FITS behind single-line comments collapses that `line` to a
        // space in both formatters — the glued path below. The gate is the shared
        // [`Printer::breaking_value_leading_run`] — including its physical-next
        // decline, so an OWNED comment glued to the type keeps the glued path here
        // exactly as it does at the `=` seams.
        if let Some((run, type_doc)) =
            self.breaking_value_leading_run(colon_end, type_start, || {
                self.build_annotation_value_doc(ty, parens)
            })
        {
            self.push_leading_run_before_breaking_value(&mut parts, &run, type_start);
            parts.push(type_doc);
            return d.concat(&parts);
        }
        parts.push(self.build_comments_between(colon_end, type_start, CommentSpacing::Trailing));
        parts.push(self.build_annotation_value_doc(ty, parens));
        d.concat(&parts)
    }

    /// The `: <union>` doc for every annotation position — the one place the hug
    /// question is asked at the `:` seam, shared by the plain entry
    /// ([`Self::build_type_annotation_doc_parens`] — parameters, destructured and rest
    /// bindings, index-signature values), the wrapping entry
    /// ([`Self::build_type_annotation_doc_with_wrapping`] — variables, class properties,
    /// property signatures, return types) and the **mapped type**'s own `:`
    /// ([`Self::build_mapped_value_tail_doc`], whose node is not a `TSTypeAnnotation` but
    /// whose seam is this one). They share it so the positions cannot
    /// disagree: prettier's union printer decides the hug on the union alone, and an
    /// entry that hangs every union after the `:` breaks a parameter's `{ … } | null`
    /// after `a:` while the same annotation on a variable hugs.
    ///
    /// ⚠️ The mapped type reached this seam through a hand-rolled copy that pushed the
    /// gap run OUTSIDE the hang. Two fixed points one pass apart followed: with the run
    /// outside, the hang measures the union alone, fits, and collapses onto the `:` line
    /// — destroying the author's break; the collapsed output then GLUES the run to the
    /// union's head, so the next pass hands it in ([`Self::build_union_value_doc`]), its
    /// multi-line text defeats `arena_fits`, and the hang opens. The run belongs inside
    /// the group whose break it is meant to force.
    ///
    /// `type_start` is the gap's far end — the caller's own window, so the hang path is
    /// each entry's own. `gap_may_have_comments` is the caller's zero-comment gate over
    /// (at least) the `:`→type gap; a `true` only turns on the gap-run lookups below.
    ///
    /// The hug is [`Self::build_hugged_union_after_operator_doc`]'s; every other union
    /// **hangs** ([`Self::hang_annotation_union_doc`]) — it breaks after the `:` with the
    /// members indented, the gap run riding inside the hang group.
    pub(in crate::printer) fn build_annotation_union_doc(
        &self,
        colon_end: u32,
        type_start: u32,
        u: &internal::TSUnionType<'_>,
        gap_may_have_comments: bool,
    ) -> DocId {
        // A glued block run between `:` and a union with no authored leading `|` is
        // handed INTO the union — the same value seam as the alias RHS
        // (`build_union_value_doc`); the caller-side `comments_doc` then stays `None`
        // so exactly one of the two prints the run (docs/comments.md hazard 3). A
        // handed run also declines the hug (`hugged` is read off the same call, never
        // re-derived from the bare predicate): prettier binds that comment to the first
        // member, and a union with a commented member breaks after the `:`.
        let UnionValueDoc {
            doc: type_doc,
            run_handed,
            hugged,
        } = self.build_union_value_doc(colon_end, u);
        let gap_run = gap_may_have_comments && !run_handed;

        // Glued comments between `:` and the union (`: /* c */ A | B`), for the hug
        // arm; the hang path routes the run through `hang_annotation_union_doc` instead.
        let comments_doc = || {
            gap_run
                .then(|| {
                    self.build_inline_comments_between_doc_trailing_space_opt(colon_end, type_start)
                })
                .flatten()
        };

        if let Some(hugged) =
            self.build_hugged_union_after_operator_doc(": ", hugged, comments_doc, type_doc)
        {
            return hugged;
        }

        self.hang_annotation_union_doc(colon_end, type_start, type_doc, gap_run)
    }

    /// The `<op> <union>` doc for an operator→union seam whose union HUGS — `: { … } | null`,
    /// `: Map<…> | null`, `is { … } | null` — or `None` when the union hangs after the
    /// operator, which each caller emits itself (the annotation's
    /// [`Self::hang_annotation_union_doc`] carries the gap run inside the hang group; the
    /// predicate's plain `hang_after_operator` does not). Shared by
    /// [`Self::build_annotation_union_doc`] and the type predicate's `is` seam, and spelled
    /// through the function type's own `joined` at its `=>`, so the hug is one rule: a
    /// hugging union keeps `<op> ` glued with no break-after-operator fallback. A brace
    /// member (`{ … } | null` / `| void`) owns its own expansion and the void member trails
    /// the `}`; a reference member (`Map<…> | null`) lets its type arguments own the break
    /// (`: Map<⏎string,⏎number⏎> | null`, `union_hug_reference_long`). Prettier's union
    /// printer prints a hugging union with no group of its own, so nothing above the member
    /// can break — a break after the operator for a union that DOES hug was a tsv-only
    /// second state, pinned by no fixture and matching prettier nowhere.
    /// ⚠️ Not the sanctioned `return_type_generic_union_long` family: that one is a `null`
    /// member INSIDE a type argument (`Promise<A | null>`), where prettier hugs the `<…>` past
    /// the print width, and it lives in [`union_has_brace_member`]'s narrowing of the
    /// type-ARGUMENT hug, not at any operator seam.
    ///
    /// `hugged` is whether the union's doc prints hugged — the value seams read it off
    /// [`UnionValueDoc`] (a glued block run handed into the union declines the hug: prettier
    /// binds that comment to the first member, and the union then breaks after the operator,
    /// `union_hug_gap_block_comment`); the predicate's `is` seam, where prettier binds the
    /// same comment to the predicate's annotation and keeps hugging, asks the bare
    /// [`Self::union_prints_hugged`]. The caller passes the answer of the doc it built, never
    /// a re-derivation, so the seam and the union cannot disagree.
    ///
    /// `operator` is the seam's spaced text (`": "`) and `comments_doc` the gap's inline
    /// comment run, built lazily: only the hug emits it here — its layout never synthesizes
    /// the break the hang emission's soft separators key on — and the comment-free common
    /// path carries no empty child.
    ///
    /// [`union_has_brace_member`]: super::helpers::union_has_brace_member
    pub(in crate::printer) fn build_hugged_union_after_operator_doc(
        &self,
        operator: &'static str,
        hugged: bool,
        comments_doc: impl FnOnce() -> Option<DocId>,
        type_doc: DocId,
    ) -> Option<DocId> {
        let d = self.d();
        hugged.then(|| match comments_doc() {
            Some(c) => d.concat(&[d.text(operator), c, type_doc]),
            None => d.concat(&[d.text(operator), type_doc]),
        })
    }

    /// Emit `: <run?><union>` with the gap's unclaimed run riding INSIDE the hang
    /// group via the value-gap leading emitter, so each comment's separator (space /
    /// soft `line` / hardline — `push_leading_comment_run`'s three-way rule)
    /// materializes exactly when the `:` seam breaks: a glued run keeps its bytes
    /// and rides down glued, a broke-after run's soft `line` drops the union below
    /// it, and an own-line run's hardline holds its authored line and forces the
    /// hang open. Emitted outside the group (the former `Trailing` glue), the soft
    /// and hard separators were invisible to it — an own-line run got welded back
    /// onto the union's head, which is not a fixed point.
    ///
    /// The one emission for both annotation union arms (simple + wrapping);
    /// `gap_run` is the caller's "unclaimed to-emit run in the gap" answer
    /// (`has_comments && !run_handed` — hazard 3's exactly-one-printer split with
    /// `build_union_value_doc`), so the comment-free common path carries no empty
    /// child.
    fn hang_annotation_union_doc(
        &self,
        colon_end: u32,
        type_start: u32,
        type_doc: DocId,
        gap_run: bool,
    ) -> DocId {
        let d = self.d();
        let hung = if !gap_run {
            type_doc
        } else if let Some(run) = self.broke_after_value_leading_run(colon_end, type_start)
            && self.run_holds_multiline_block(&run)
        {
            // A multi-line comment carries its own hard break, so the run's separators are
            // forced and an author blank after it survives — the soft value-gap emitter below
            // would yield it (`: /* x⏎y */⏎⏎| A⏎| B`), as the non-union `:` arm no longer does
            // ([`Printer::breaking_value_leading_run`]).
            self.leading_run_before_breaking_value_doc(&run, type_start, type_doc)
        } else {
            self.prepend_rhs_comments(type_doc, colon_end, type_start)
        };
        d.concat(&[d.text(":"), hang_after_operator(d, hung)])
    }

    /// Build type annotation doc with width-aware type argument wrapping.
    ///
    /// For `TypeReference<Args>`, `build_type_arguments_doc` wraps the type
    /// arguments at the width boundary.
    ///
    /// For a non-hugging union, the break-after-colon layout:
    /// ```text
    /// property:
    ///     | string
    ///     | number;
    /// ```
    /// A hugging union (`{ … } | null`, `Map<…> | null`) keeps `: ` glued instead — the
    /// shared `:` seam, [`Self::build_annotation_union_doc`].
    ///
    /// For other types, delegates to `build_type_annotation_doc`.
    ///
    /// Returns doc starting with `: ` (the annotation prefix).
    pub(in crate::printer) fn build_type_annotation_doc_wrapping(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        self.build_type_annotation_doc_with_wrapping(annotation, true, AnnotationParens::AsWritten)
    }

    /// Build type annotation doc for function return types.
    ///
    /// For return types, we only use wrapping when type arguments would benefit from breaking:
    /// - Multiple type args (like `Result<A, B>`) - can break between args
    /// - Unions/intersections (like `Promise<A | B>`) - can break internally
    ///
    /// Simple cases like `Promise<void>` should NOT wrap - we want params to break first.
    pub(in crate::printer) fn build_type_annotation_doc_for_return_type(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        self.build_type_annotation_doc_with_wrapping(annotation, false, AnnotationParens::AsWritten)
    }

    /// [`Self::build_type_annotation_doc_for_return_type`] for an **arrow's** return
    /// type, whose function-type case takes the disambiguating pair
    /// ([`AnnotationParens::ArrowReturn`]). Every other return-type position — the
    /// function type's own, `function` declarations, methods — ends its annotation at a
    /// token that cannot be mistaken for a `=>`, so they keep the `AsWritten` entry.
    pub(in crate::printer) fn build_arrow_return_type_annotation_doc(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
    ) -> DocId {
        self.build_type_annotation_doc_with_wrapping(
            annotation,
            false,
            AnnotationParens::ArrowReturn,
        )
    }

    /// Inner implementation for type annotation with wrapping support.
    ///
    /// When `always_wrap` is true, wraps any TypeReference with type args.
    /// When false, only wraps if type args would benefit from breaking.
    fn build_type_annotation_doc_with_wrapping(
        &self,
        annotation: &internal::TSTypeAnnotation<'_>,
        always_wrap: bool,
        parens: AnnotationParens,
    ) -> DocId {
        let d = self.d();
        let colon_end = annotation.span.start + 1; // After the `:`
        // A transparent one-member composite is its member here, the `|` dropped and its
        // head gap folded into the `:`→type gap ([`Printer::transparent_value`]); every
        // read below is of the member.
        let value = self.transparent_value(annotation.type_annotation);
        let type_start = value.span().start;

        // One window search over the whole annotation gates every comment query below.
        // Each of them — the `:`→type gap, the type-name→type-args gap, and the member
        // gaps `union_prints_hugged` inspects — is bounded inside `annotation.span`
        // (`: Type`), and a comment only counts when it lies fully inside the queried
        // range. So a comment-free annotation provably has none in any of them: the
        // per-gap searches are skipped and the `empty()` children they would feed into
        // the concats below are never pushed. Byte-identical, and the false path is the
        // overwhelmingly common one — annotations are among the most frequent TS
        // constructs, and comments inside one are rare.
        let has_comments =
            self.has_comments_to_emit_between(annotation.span.start, annotation.span.end);

        // First check for line comments between `:` and the type.
        // If there are comments, fall back to build_type_annotation_doc which handles them
        // properly. A redundant paren shell with a leading line-comment run in the return
        // type (`(): (// c\n T)`) must delegate just like the bare `(): // c\n T` — widen the
        // probe to the unwrapped inner's start so the outer paren doesn't hide the comment
        // (build_type_annotation_doc strips the shell and hangs the type; without this the
        // wrapping logic below would relocate the comment non-idempotently).
        let line_comment_probe_end = self.keyword_value_stripped_paren_hang(value).value_start;
        if has_comments && self.has_line_comments_between(colon_end, line_comment_probe_end) {
            return self.build_type_annotation_doc_parens(annotation, parens);
        }

        // A glued format-ignore directive freezes a non-composite type verbatim —
        // checked before the TypeReference-with-args branch below would rebuild a
        // frozen `Foo<...>` from parts (own-line directives are line comments, already
        // delegated above; composites decline and freeze via their own walk).
        if has_comments && self.single_child_frozen(colon_end, value) {
            return self
                .build_simple_type_annotation_doc(colon_end, type_start, value, true, parens);
        }

        // Handle TypeReference with type arguments - use wrapping version when appropriate
        if let TSType::TypeReference(r) = value
            && let Some(type_args) = &r.type_arguments
            && (always_wrap || type_args_should_wrap_for_return_type(type_args))
        {
            let mut parts: DocBuf = smallvec![d.text(": ")];
            // Comments between `:` and the type (e.g., `: /* c */ Promise<string>`)
            if has_comments
                && let Some(comments_doc) =
                    self.build_inline_comments_between_doc_trailing_space_opt(colon_end, type_start)
            {
                parts.push(comments_doc);
            }
            parts.push(self.build_entity_name_doc(&r.type_name));
            // Preserve comments between type name and type args: `Promise/* c */ <string>`
            if has_comments
                && let Some(name_ta_comments) = self.build_name_to_type_params_comments_opt(
                    r.type_name.span().end,
                    type_args.span.start,
                    CommentSpacing::Trailing,
                )
            {
                parts.push(name_ta_comments);
            }
            parts.push(self.build_type_arguments_doc(type_args));
            return d.concat(&parts);
        }

        // Strip redundant comment-free parens around a union / intersection so a
        // `(A | B)` / `(A & B)` return type or member type gets the same break
        // layout as the bare form (prettier strips them too). Other parenthesized
        // types keep the existing fall-through below.
        let value_type = self.unwrap_redundant_parens(value);
        let value_type_start = value_type.span().start;

        // Union types: hug the `:` when the union prints hugged, else hang after it.
        if let TSType::Union(u) = value_type {
            return self.build_annotation_union_doc(colon_end, value_type_start, u, has_comments);
        }

        // Handle Intersection types - first member hugs `:`, continuations indented.
        if let TSType::Intersection(i) = value_type {
            return self.build_intersection_type_annotation_doc(i, colon_end);
        }

        // Fall-through: reached on every simple annotation (`: string`, `: Foo`,
        // `: Foo[]`, …), the common case. Emit the shared `: <comments> <type>` path
        // directly instead of delegating to `build_type_annotation_doc`, which would
        // re-derive what we already know here: no line comments (proven false above),
        // and `unwrap_redundant_parens` + the Union/Intersection match (ruled out above).
        // A comment-free annotation also already answers the gap query, so it costs no
        // search of its own.
        self.build_simple_type_annotation_doc(
            colon_end,
            type_start,
            value,
            has_comments && self.has_comments_to_emit_between(colon_end, type_start),
            parens,
        )
    }

    /// Build intersection type annotation with proper indentation.
    ///
    /// Structure for class properties:
    /// ```text
    /// property: FirstType &
    ///     SecondType &
    ///     ThirdType;
    /// ```
    ///
    /// The first type stays on the same line as `:`, continuation types are indented.
    /// This differs from `build_intersection_type_doc` (in union_intersection.rs) which
    /// doesn't add internal indentation (expecting the parent context to provide it).
    /// Both functions share the same grouping rule: 2-type with a huggable/expanding
    /// boundary (TypeLiteral/MappedType at first or last position) skips the group;
    /// all other cases need one.
    ///
    /// Line comments between members are delegated to `build_intersection_type_doc`
    /// (which owns the multiline-with-comments layout) — the continuation loop here
    /// has no line-comment handling and would otherwise drop them.
    fn build_intersection_type_annotation_doc(
        &self,
        intersection: &internal::TSIntersectionType<'_>,
        colon_end: u32,
    ) -> DocId {
        let d = self.d();
        if intersection.types.is_empty() {
            return d.text(": ");
        }

        // Single type - just use the normal intersection doc
        // Extract comments between `:` and the type (e.g., `: & /* c */ A`)
        if intersection.types.len() == 1 {
            let first_type_start = intersection.types[0].span().start;
            // A LINE comment in the `:`→member window (it sits in the intersection's
            // leading-`&` gap — a comment before the span would have routed to
            // `build_type_annotation_doc`'s line-comment branch instead): emit the
            // one-pass fixed point of the reparsed, `&`-less authoring through the same
            // keyword→value emitter — a comment on the `:` line trails it, an own-line one
            // keeps its line. An own-line directive also freezes a non-composite member
            // (Rule A — a head-trailing relocation is inert, losing the freeze on pass 2),
            // mirroring `build_annotation_own_line_directive_doc`; a union/intersection
            // sole member builds normally and freezes via its own leading-run walk. The
            // inline path below would relocate the comment to trail `:` with the member at
            // column 0 — a 2-pass transient, and a lost freeze for a directive.
            // TODO: believed unreachable — both callers' line-comment gates see through a
            // one-member composite before this arm is asked; delete once a planted panic
            // here also survives `ignore:audit` and `gaps:audit`.
            if self.has_line_comments_between(colon_end, first_type_start) {
                let child = &intersection.types[0];
                let value_doc = if self.single_child_frozen(colon_end, child) {
                    self.build_frozen_single_child_doc(child)
                } else {
                    self.build_type_doc(child)
                };
                let mut parts: DocBuf = smallvec![d.text(":")];
                self.append_keyword_value_line_comments(
                    &mut parts,
                    colon_end,
                    first_type_start,
                    value_doc,
                );
                return d.concat(&parts);
            }
            let mut parts: DocBuf = smallvec![d.text(": ")];
            if let Some(comments_doc) = self
                .build_inline_comments_between_doc_trailing_space_opt(colon_end, first_type_start)
            {
                parts.push(comments_doc);
            }
            parts.push(self.build_type_doc(&intersection.types[0]));
            return d.concat(&parts);
        }

        // Multi-member: `: ` + any colon→first-member comment, then delegate the whole
        // intersection body to the shared bare-intersection printer
        // (`intersection_hanging_with_indent`). The first member hugs `:` and
        // continuations indent, exactly like the type-alias RHS — huggable boundaries,
        // the expanding-first hug, and block + line comments are all handled uniformly
        // there (a single source of truth; the bare and annotation contexts can't drift).
        // Emit only the comment between `:` and the intersection's span start. A leading
        // block comment INSIDE the intersection (`: & /* c */ A`, where the span starts at
        // the leading `&`) is emitted by the bare printer, so bounding at `types[0]` here
        // would double-emit it.
        let mut parts: DocBuf = smallvec![d.text(": ")];
        if let Some(comments_doc) = self.build_inline_comments_between_doc_trailing_space_opt(
            colon_end,
            intersection.span.start,
        ) {
            parts.push(comments_doc);
        }
        parts.push(self.intersection_hanging_with_indent(intersection));
        d.concat(&parts)
    }
}
