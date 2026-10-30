// Helper functions for type printing
//
// Standalone functions that don't require Printer state:
// - Type parenthesization predicates
// - Type unwrapping utilities
// - Source scanning helpers

use super::Printer;
use crate::ast::{
    self, TSIntersectionType, TSKeywordKind, TSLiteralType, TSParenthesizedType, TSType,
    TSUnionType,
};
use tsv_lang::Span;
use tsv_lang::source_scan::find_char_skipping_comments;

//
// Type argument analysis
//

/// Check if type arguments warrant wrapping in return types.
///
/// Returns true when type args can benefit from breaking:
/// - Multiple type args (like `Result<A, B>`) - can break between args
/// - Unions or intersections (like `Promise<A | B>`) - can break internally
/// - Nested TypeReferences with multiple type args (like `Promise<Result<A, B>>`) - inner can break
///
/// Returns false for single simple type args (like `Promise<void>`) - these should
/// let function params break first rather than breaking the return type.
pub(super) fn type_args_should_wrap_for_return_type(
    args: &ast::TSTypeParameterInstantiation<'_>,
) -> bool {
    // Multiple type args can break between them
    if args.params.len() > 1 {
        return true;
    }
    // Single type arg cases
    args.params.iter().any(|param| {
        match param {
            // Unions/intersections can break internally
            TSType::Union(_) | TSType::Intersection(_) => true,
            // Function/constructor types can break params internally
            TSType::Function(_) | TSType::Constructor(_) => true,
            // Nested TypeReference with multiple type args can break
            TSType::TypeReference(r) => r
                .type_arguments
                .as_ref()
                .is_some_and(|inner_args| inner_args.params.len() > 1),
            _ => false,
        }
    })
}

//
// Source scanning helpers
//

/// Find the position of a separator character in the source between start and end,
/// skipping over comments. Returns Some(position) if found, None otherwise.
pub(super) fn find_separator_position(
    source: &str,
    start: u32,
    end: u32,
    separator: u8,
) -> Option<u32> {
    find_char_skipping_comments(source.as_bytes(), start as usize, end as usize, separator)
        .map(|pos| pos as u32)
}

//
// Type unwrapping
//

/// Recursively unwrap TSParenthesizedType to get the inner type.
pub fn unwrap_parenthesized<'a>(ts_type: &'a TSType<'a>) -> &'a TSType<'a> {
    match ts_type {
        TSType::Parenthesized(p) => unwrap_parenthesized(p.type_annotation),
        _ => ts_type,
    }
}

/// Prettier's **`isSimpleType`** (`utilities/is-simple-type.js`): an atomic type that
/// never benefits from breaking — a primitive keyword, a literal (string / number /
/// bigint / negative / template), `this`, or a bare type reference with no type arguments
/// of its own (a nested `Array<X>` is *not* simple). Booleans are `TSType::Keyword` here,
/// so they fall under the keyword arm rather than `Literal`.
///
/// Parenthesized wrappers are unwrapped first: prettier's TS AST carries no
/// `TSParenthesizedType` node, so `(T)` reaches its check already as the bare reference.
///
/// Two callers ask it for different reasons — the type-argument hug via
/// [`is_simple_type_arg`], and the cast-seed shortness rule in
/// [`is_hopefully_short_arg`](crate::printer::calls::arg_predicates) — so it lives here as
/// one body rather than two spellings that could drift.
pub fn is_simple_type(ty: &TSType<'_>) -> bool {
    match unwrap_parenthesized(ty) {
        TSType::Keyword(_) | TSType::ThisType(_) | TSType::Literal(_) => true,
        TSType::TypeReference(reference) => reference.type_arguments.is_none(),
        _ => false,
    }
}

/// [`is_simple_type`] as the inline/hug criterion for a single **type argument** — the
/// single source of truth shared by the call/`new`/instantiation type-argument builder and
/// the type-position builder so the two agree by construction.
///
/// Template-literal types are **excluded** even though Prettier's `isSimpleType` accepts
/// them: tsv's template-literal-type printer carries an internal `${…}` break point, so
/// inlining the `<…>` would let the template break *there* (defeating the atomicity the
/// caller relies on). They stay in the breakable group path — a residual divergence
/// pending an atomic template-literal-type printer. That narrowing is this caller's alone;
/// don't push it down into [`is_simple_type`].
pub fn is_simple_type_arg(ty: &TSType<'_>) -> bool {
    let unwrapped = unwrap_parenthesized(ty);
    is_simple_type(unwrapped)
        && !matches!(
            unwrapped,
            TSType::Literal(TSLiteralType::TemplateLiteral(_))
        )
}

/// Whether any union member is **brace-delimited** (`TypeLiteral`/`Mapped`) — what splits
/// prettier's hugging type-argument union (`isObjectLikeType`, which also accepts a bare
/// `TSTypeReference`) into its two tsv layouts: a brace member carries its own group and
/// breaks block-style inside the inlined `<…>`, so the `<…>` itself never needs a break
/// point ([`Printer::type_arg_union_prints_hugged`](super::super::Printer::type_arg_union_prints_hugged));
/// a reference member (`Promise<… | null>`) has none of its own to offer, so its `<…>` is
/// unbreakable first and broken inside only past print width — the nullable union type
/// argument print-width divergence
/// ([`Printer::type_arg_is_nullable_reference_union`](super::super::Printer::type_arg_is_nullable_reference_union)).
///
/// One *clause* of those gates, never a gate itself. It is safe to ask bare (unlike
/// [`union_hug_shape`], it makes no claim to answer "does this hug?").
///
/// ⚠️ **Reads through the paren shell, via [`is_huggable_type`]** — the same
/// `isObjectType` this narrows, so a `({ … })` member answers the same here as it does at
/// [`union_hug_shape`]'s [`is_object_like_type`]. The two are clauses of one gate and must
/// agree about a parenthesized member, or the gate reads two ways about one union.
pub(super) fn union_has_brace_member(union: &TSUnionType<'_>) -> bool {
    union.types.iter().copied().any(is_huggable_type)
}

/// The retained-paren shell around `ts_type` — its **outermost** `TSParenthesizedType`
/// node. Returns `None` when `ts_type` is not parenthesized in source (the parens are
/// synthetic, added by the printer for precedence — there is no author gap, so no
/// comments to preserve).
///
/// Used to recover the paren span so the paren-retaining member printers can emit
/// comments the user wrote inside retained parens — `build_parenthesized_union_doc`
/// (`(/* c */ a | b)`, `(a | b /* c */)`) and
/// `build_parenthesized_intersection_trailing_object_doc` (`(// c⏎a & { … })`). Both are
/// handed their already-unwrapped inner type, so the paren's own gap is invisible to them
/// otherwise, and a comment in it would be silently dropped.
///
/// **Outermost, never an inner layer.** Redundant nested parens collapse into the single
/// pair those builders emit, so the whole `(`…`)` region is one author gap. Bounding a
/// gap scan by an inner layer's span stops it short and drops everything between the two
/// closers (`((A | B) /* c */)[]`, `[((A | B) /* c */)?]`).
pub(in crate::printer) fn outermost_paren<'a>(
    ts_type: &'a TSType<'a>,
) -> Option<&'a TSParenthesizedType<'a>> {
    match ts_type {
        TSType::Parenthesized(p) => Some(p),
        _ => None,
    }
}

/// The **deep** interior gaps of a redundant paren shell, as `(leading, trailing)` — from
/// the outermost `(` to the fully-unwrapped inner type, and from that type back out to the
/// outermost `)`. Pair it with [`outermost_paren`] for a type that may carry no shell:
/// `outermost_paren(ty).map(paren_shell_gaps)`.
///
/// **Deep, not per-paren**, and that is the whole reason this is one function rather than a
/// span pair each caller derives: redundant layers strip as a UNIT and print a single pair
/// between them, so a comment in a doubly-nested shell (`((/* c */ T))`) falls *between* the
/// two `(`s, where either layer's own window is blind to it. Both halves have been widened
/// to this window one at a time, each after a bug — the leading half at
/// [`Printer::stripped_paren_hang_has_breaking_leading_run`](super::super::Printer::stripped_paren_hang_has_breaking_leading_run),
/// the trailing half at
/// [`Printer::paren_shell_retains_for_trailing_run`](super::super::Printer::paren_shell_retains_for_trailing_run),
/// where asking only the outer layer's own trailing gap called a shell stripped while the
/// inner pair retained and printed the leading run itself, beside the enclosing gap's copy
/// (a DOUBLE-PRINT). Every caller that reads a shell's own comments asks here — the type
/// builders, the required-paren emitter, and `ignore.rs`'s directive-freeze scans alike — so
/// the two halves cannot drift apart again.
///
/// The paren bytes themselves are excluded, which changes no comment query — a comment can
/// neither begin at the `(` nor end past the `)` — but keeps the spans meaning exactly "the
/// author's gap". That is why a caller whose hand-rolled window included them
/// (`t.span().start` rather than `+ 1`) converts to this pair unchanged.
pub(in crate::printer) fn paren_shell_gaps(shell: &TSParenthesizedType<'_>) -> (Span, Span) {
    let inner = unwrap_parenthesized(shell.type_annotation).span();
    (
        Span::new(shell.span.start + 1, inner.start),
        Span::new(inner.end, shell.span.end - 1),
    )
}

/// Check if a type is "huggable" - brace-delimited types that expand internally.
///
/// Prettier's **`isObjectType`** (`utilities/node-types.js`), minus its Flow-only
/// `ObjectTypeAnnotation` arm. TypeLiteral (`{ a: T }`) and Mapped (`{ [K in T]: V }`)
/// types are huggable: they handle their own expansion and should keep `{` hugged to
/// the context.
///
/// ⚠️ **Reads through [`unwrap_parenthesized`], and every caller depends on that.**
/// Prettier's TS AST carries no `TSParenthesizedType` — its parser drops a redundant type
/// paren before any printing runs — so `isObjectType` always sees the inner node. tsv
/// KEEPS the paren node (it decides the output parens), so a raw read answers `false` for
/// `({ … })` and silently changes a layout the redundant paren is supposed to leave alone.
/// That bug has been live at four separate gates — the intersection separator, the
/// sole-parameter hug, the parameter grouping and the union hug. **Matching prettier's
/// node check means unwrapping**; do not "read the raw member to match Prettier" here or
/// at any caller.
#[inline]
pub(in crate::printer) fn is_huggable_type(ts_type: &TSType<'_>) -> bool {
    matches!(
        unwrap_parenthesized(ts_type),
        TSType::TypeLiteral(_) | TSType::Mapped(_)
    )
}

/// The **syntactic shape** a hugging union must have — exactly one object-like
/// member (`TSTypeLiteral` or `TSTypeReference`) with only void siblings (`void`,
/// `null`), e.g. `{ name: string; value: number } | null`.
///
/// ⚠️ **Necessary, never sufficient — do not use this as a layout gate.** This is
/// Prettier's `shouldHugUnionType` (`utilities/union-type-print.js`) **minus its
/// first clause**, `types.some((n) => hasComment(n))`. That clause needs the
/// comment table, which a free function has no access to; it lives in
/// [`Printer::union_prints_hugged`](super::super::Printer::union_prints_hugged),
/// which pairs this shape with the comment checks and is the single source of
/// truth for *whether the hug actually happens*.
///
/// A layout gate that asks this predicate alone re-derives the hug with a subset
/// of the rule, so it answers "hug" for a union the printer then expands: the
/// keyword keeps its operand glued while the members explode below it
/// (`type A = Foo<| {…} /* c */⏎| null>`). That is a bug class, not a hypothetical
/// — five gates asked it bare. Every caller must pair it with `union_prints_hugged`
/// (see [`Printer::type_arg_union_prints_hugged`](super::super::Printer::type_arg_union_prints_hugged)
/// for the one shape that does).
pub(super) fn union_hug_shape(union: &TSUnionType<'_>) -> bool {
    // Find exactly one object-like type
    let mut object_idx = None;
    for (i, t) in union.types.iter().enumerate() {
        if is_object_like_type(t) {
            if object_idx.is_some() {
                // More than one object-like type — don't hug
                return false;
            }
            object_idx = Some(i);
        }
    }
    let Some(obj_idx) = object_idx else {
        return false;
    };

    // All non-object members must be void types
    union
        .types
        .iter()
        .enumerate()
        .all(|(i, t)| i == obj_idx || is_void_type(t))
}

/// Check if a type is "object-like" for union hugging purposes.
/// Matches Prettier's `isObjectLikeType`: TSTypeLiteral and TSTypeReference.
///
/// ⚠️ **Reads through [`unwrap_parenthesized`], for the reason [`is_huggable_type`]
/// states**: prettier's TS AST carries no paren node, so matching its `isObjectLikeType`
/// means matching the node the paren wraps. A raw read is why `G<({ … }) | null>` used to
/// decline the hug its paren-free spelling takes.
///
/// The unwrap widens what reaches the hug, and the comment gate is what makes that safe:
/// prettier bails `shouldHugUnionType` on `types.some((t) => hasComment(t))`, and a comment
/// the author wrote inside a member's shell attaches to that member — so a paren'd member
/// carrying one must decline the hug exactly as the bare spelling does.
/// `Printer::union_member_shell_holds_comment` is that clause — without it a one-member
/// union whose member is a paren shell reaches the hug, where the hug's own leading-run
/// emitter and the enclosing gap that claimed the shell's region both fill one position and
/// an authored comment prints TWICE
/// (`single_member_intersection_leading_gap_shell_run_prettier_divergence`, the
/// `q<| /* c */ (// d⏎ P)>` case).
#[inline]
fn is_object_like_type(ts_type: &TSType<'_>) -> bool {
    matches!(
        unwrap_parenthesized(ts_type),
        TSType::TypeLiteral(_) | TSType::TypeReference(_)
    )
}

/// Check if a type is a "void type" for union hugging purposes.
/// Matches Prettier's `isVoidType`: void and null keywords.
///
/// Unwrapped for the same reason as [`is_object_like_type`] — the two are the two slots of
/// one gate, so they move together or not at all.
#[inline]
fn is_void_type(ts_type: &TSType<'_>) -> bool {
    matches!(
        unwrap_parenthesized(ts_type),
        TSType::Keyword(kw) if matches!(kw.kind, TSKeywordKind::Void | TSKeywordKind::Null)
    )
}

/// Check if the last type in an intersection is "huggable" (like TypeLiteral or MappedType).
///
/// Huggable types expand independently and should not have breaks/indent applied
/// around them in the parent context. This keeps patterns like `& {` hugged together.
#[inline]
pub(in crate::printer) fn intersection_has_huggable_last_type(
    intersection: &TSIntersectionType<'_>,
) -> bool {
    intersection
        .types
        .last()
        .copied()
        .is_some_and(is_huggable_type)
}

/// Check if the first type in an intersection is "expanding" (like TypeLiteral or MappedType).
///
/// When the first type expands (contains hardlines from multiline object body),
/// the continuation should use space instead of line to keep `} & Type` together.
/// This is the mirror of `intersection_has_huggable_last_type` for the first position.
#[inline]
pub(in crate::printer) fn intersection_has_expanding_first_type(
    intersection: &TSIntersectionType<'_>,
) -> bool {
    intersection
        .types
        .first()
        .copied()
        .is_some_and(is_huggable_type)
}

//
// Type parenthesization predicates
//

/// A "does this type need a pair at my position?" rule — one of the predicates below, in
/// the shape every paren seam takes it (a member-parens rule, a required-pair operand, a
/// frozen head's slice rule). It is asked THROUGH the printer, never of the bare node: what
/// needs a pair is the type **as printed**, and only the printer knows that
/// ([`Printer::printed_operand`]) — a sole-member composite prints as its member, except
/// a frozen one, which prints as the composite, `|` included. A rule that peeled on its
/// own (a free `fn(&TSType) -> bool`) read the frozen shape as its member and stripped a
/// pair the printed composite needs: `keyof (⏎// prettier-ignore⏎| {a:1})` came out
/// `keyof⏎// prettier-ignore⏎| {a:1}`, which does not reparse
/// (`union_prettier_ignore_single_member`). Taking the printer is what makes the peel
/// impossible to leave out.
pub(in crate::printer) type TypeParenRule = fn(&Printer<'_>, &TSType<'_>) -> bool;

impl Printer<'_> {
    /// The node `ts_type` PRINTS as at a paren seam — through the author's parens and every
    /// transparent one-member union / intersection layer (`(& { x: X })` → `{ x: X }`, the
    /// leading-operator authoring), recursively.
    ///
    /// Such a composite prints as that member: prettier drops single-element
    /// union/intersection nodes in postprocess, and tsv reaches the same collapse via
    /// `union_member_parens`. So every "does the inner need parens here?" predicate asks
    /// about what will be printed rather than about the wrapper — asking the wrapper says
    /// "an intersection, so parens" and keeps a pair that nothing needs (`(& { x: X })[]`
    /// → `({ x: X })[]`, where prettier emits `{ x: X }[]`). The peel keeps asking the same
    /// question of the member, so a member that genuinely needs parens (`(& (A | B))[]`)
    /// still gets them.
    ///
    /// The one layer that does NOT peel is a **frozen** sole-member composite
    /// ([`Self::transparent_sole_member`] stops at [`Self::sole_member_peels`]'s freeze):
    /// its printed form is the composite's own verbatim slice, operator included, so it
    /// needs a pair wherever a two-member composite does — the same answer the printer's
    /// own layout peel gives, which is the whole point of asking here rather than of the
    /// node.
    pub(in crate::printer) fn printed_operand<'t>(
        &self,
        ts_type: &'t TSType<'t>,
    ) -> &'t TSType<'t> {
        let inner = unwrap_parenthesized(ts_type);
        match self.transparent_sole_member(inner) {
            Some(member) => self.printed_operand(member),
            None => inner,
        }
    }

    /// Whether `ts_type`, a bare negative literal indexed-access object, was recorded by an
    /// enclosing array type as leading a run that holds a `[]`
    /// ([`Printer::negative_literal_pair_targets`]).
    pub(in crate::printer) fn negative_literal_array_run_pairs(
        &self,
        ts_type: &TSType<'_>,
    ) -> bool {
        self.negative_literal_pair_targets.is_marked(ts_type.span())
    }
}

/// Check if a type needs parentheses when used as the object in indexed access (`T[K]`).
/// Without parens: `A | B[K]` parses as `A | (B[K])`, not `(A | B)[K]`
///
/// A negative literal object (`(-1)[K]`) keeps an AUTHORED pair — and so does a pair around a
/// postfix run such a literal leads (`(-1[K])[J]`) — but a bare one prints as written, unless
/// an array suffix further out needs the pair ([`array_run_negative_literal`]).
pub(in crate::printer) fn type_needs_parens_for_indexed_access_object(
    p: &Printer<'_>,
    ts_type: &TSType<'_>,
) -> bool {
    let inner = p.printed_operand(ts_type);
    // TypeOperator included: `(keyof T)[K]` is valid and different from `keyof T[K]`
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeQuery(_)
            | TSType::TypeOperator(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    ) || authored_pair_leads_with_bare_negative_literal(p, ts_type)
        || (is_negative_literal(ts_type) && p.negative_literal_array_run_pairs(ts_type))
}

/// Check if a type needs parentheses when used as the element type in an array (`T[]`).
/// Without parens: `A | B[]` parses as `A | (B[])`, not `(A | B)[]`
///
/// A negative literal element always takes a pair (`-1[]` → `(-1)[]`): acorn-typescript
/// reads the bare literal with its expression parser, whose member read meets the `]` and
/// does not parse, while tsc reads the paired and the bare spelling as one type
/// ([`is_negative_literal`]). An authored pair around a run such a literal leads
/// (`(-1[K])[]`) is kept as written.
pub(in crate::printer) fn type_needs_parens_for_array_element(
    p: &Printer<'_>,
    ts_type: &TSType<'_>,
) -> bool {
    let inner = p.printed_operand(ts_type);
    // TypeOperator included: `(keyof T)[]` differs from `keyof T[]`, and
    // `(readonly string[])[]` differs from `readonly string[][]`.
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeQuery(_)
            | TSType::TypeOperator(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    ) || is_negative_literal(inner)
        || authored_pair_leads_with_bare_negative_literal(p, ts_type)
}

/// Whether `ts_type` is a negative literal type (`-1`, `-1n`).
///
/// Such a literal is the one type tsc and acorn-typescript read differently ahead of
/// postfix brackets: tsc's postfix loop takes `[]` / `[K]` after it, but acorn-typescript
/// reads the literal with its EXPRESSION parser (`parseMaybeUnary`), which takes every
/// `[…]` that follows as a computed member. So a bare `-1[]` does not parse there, and a
/// bare `-1[K]` is the literal type `-(1[K])` — a different tree. A pair settles both
/// parsers on tsc's reading.
pub(in crate::printer) fn is_negative_literal(ts_type: &TSType<'_>) -> bool {
    matches!(ts_type, TSType::Literal(TSLiteralType::UnaryExpression(_)))
}

/// Whether `ts_type` PRINTS starting with a negative literal's `-` — the literal itself,
/// or a run of indexed accesses whose leftmost object is one printed without a pair.
///
/// Read from the printed form, not the source: an object behind a paren shell never leads
/// with a bare literal, whichever way the shell resolves. Kept, the shell prints its `(`
/// first; stripped, it prints its inner, which does not lead with the literal (or the shell
/// would have been kept). An ARRAY run never qualifies either — its leftmost literal always
/// gains a pair ([`array_run_negative_literal`]).
fn leads_with_bare_negative_literal(ts_type: &TSType<'_>) -> bool {
    match ts_type {
        TSType::Literal(TSLiteralType::UnaryExpression(_)) => true,
        TSType::IndexedAccess(i) => leads_with_bare_negative_literal(i.object_type),
        _ => false,
    }
}

/// Whether `ts_type` is an AUTHORED paren shell (redundant layers included) whose content
/// prints starting with a negative literal — `(-1)`, `((-1))`, `(-1[K])` — ahead of postfix
/// brackets. The pair is kept: each parser reads the output as it read the input, where
/// stripping it hands acorn-typescript a member read (`-1[]` rejected, `-1[K]` the literal
/// `-(1[K])`) or, around a bare run, tsc's tree in place of the one acorn read.
fn authored_pair_leads_with_bare_negative_literal(p: &Printer<'_>, ts_type: &TSType<'_>) -> bool {
    matches!(ts_type, TSType::Parenthesized(_))
        && leads_with_bare_negative_literal(p.printed_operand(ts_type))
}

/// The bare negative literal at the leftmost end of an array element's run of indexed
/// accesses (`-1[K][]` → the `-1`), which needs a pair because the run holds a `[]`:
/// acorn-typescript's member read of the literal meets the `[]` and does not parse, so
/// tsv REPAIRS it to `(-1)[K][]`, the tree tsc read, in the spelling both parsers read. A
/// run of `[K]` alone is spelled as written instead — both parsers accept it, each as its
/// own program. Descends only through BARE objects: a paren shell ends the run.
pub(in crate::printer) fn array_run_negative_literal<'t>(
    element: &'t TSType<'t>,
) -> Option<&'t TSType<'t>> {
    let TSType::IndexedAccess(_) = element else {
        return None;
    };
    let mut ty = element;
    while let TSType::IndexedAccess(i) = ty {
        ty = i.object_type;
    }
    is_negative_literal(ty).then_some(ty)
}

/// Check if a type needs parentheses when used as an optional tuple element (`[T?]`).
/// Matches Prettier's `TSOptionalType` parent rule in `needs-parentheses.js`
/// (union/intersection plus the `TSTypeOperator`-case fall-through). Without
/// parens the `?` rebinds: `[() => void?]` / `[A | B?]` are invalid or change
/// meaning.
pub(super) fn type_needs_parens_for_optional_element(
    p: &Printer<'_>,
    ts_type: &TSType<'_>,
) -> bool {
    let inner = p.printed_operand(ts_type);
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeOperator(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    )
}

/// Check if a type needs parentheses when used as the check type of a conditional
/// (`T extends U ? ...`). A function, constructor, or nested conditional check type
/// keeps its parens (`(() => void) extends E ? ...`, `(A extends B ? C : D) extends E ? ...`);
/// without them the `extends`/`?` rebinds. Union/intersection/keyof check types need
/// none. Matches Prettier's `checkType` rule (`needs-parentheses.js`).
pub(super) fn type_needs_parens_for_conditional_check(
    _: &Printer<'_>,
    ts_type: &TSType<'_>,
) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(
        inner,
        TSType::Function(_) | TSType::Constructor(_) | TSType::Conditional(_)
    )
}

/// Check if a type needs parentheses when used as the extends type of a conditional
/// (`T extends U ? ...`). Two cases keep their parens; without them the trailing
/// `? :` rebinds or the canonical parser rejects the form:
/// - a nested conditional (`A extends (B extends C ? D : E) ? ...`);
/// - a function/constructor type whose return type — unwrapping a `p is X` type
///   predicate — is a *constrained* infer (`M extends (() => infer U extends C) ? ...`,
///   `M extends ((x) => x is infer U extends C) ? ...`). The infer's `extends C` and
///   the conditional's `?` are otherwise ambiguous. A bare `infer U` return (no
///   constraint) and ordinary return types strip.
///
/// Matches Prettier's `extendsType` rule (`needs-parentheses.js`).
pub(super) fn type_needs_parens_for_conditional_extends(
    _: &Printer<'_>,
    ts_type: &TSType<'_>,
) -> bool {
    match unwrap_parenthesized(ts_type) {
        TSType::Conditional(_) => true,
        TSType::Function(f) => return_type_is_constrained_infer(&f.return_type),
        TSType::Constructor(c) => return_type_is_constrained_infer(&c.return_type),
        _ => false,
    }
}

/// Whether `ts_type` is an `infer U` carrying an `extends` constraint. The
/// constraint has two paren-forcing consequences, keyed on this one shape: it
/// greedily absorbs a following `|`/`&` (so a constrained infer needs parens as a
/// union/intersection member — see `type_needs_parens_in_union_or_intersection`),
/// and it abuts a trailing conditional `?` (so it needs parens as a nested
/// function/constructor return in a conditional extends-type — see
/// `return_type_is_constrained_infer`). A bare `infer U` forces neither.
pub(super) fn is_constrained_infer(ts_type: &TSType<'_>) -> bool {
    matches!(ts_type, TSType::Infer(i) if i.type_parameter.constraint.is_some())
}

/// True when a function/constructor return type — descending through a `p is X`
/// type predicate and through further nested function/constructor returns — is a
/// `TSInferType` carrying an `extends` constraint. The nesting matters:
/// `() => () => infer U extends C` trails the same trailing-`?` ambiguity through
/// every arrow, so the outermost function type still needs parens.
///
/// ⚠️ **Every descent unwraps parens, and tsv needs that where prettier does not.**
/// Prettier reads `node.returnType.typeAnnotation` straight (`needs-parentheses.js`,
/// the `extendsType` arm) because typescript-estree has no `TSParenthesizedType`
/// node at all — `() => (infer U extends C)` and `() => infer U extends C` are one
/// AST to it. tsv keeps the paren node, so a direct read sees a
/// `TSType::Parenthesized` where prettier sees the infer, answers `false`, and the
/// caller strips the parens the enclosing function type needs — after which the
/// infer's `extends` greedily swallows the conditional's `?` and the output does
/// not reparse. Pinned by `types/infer/constrained_extends_parens`'s
/// `unformatted_inner_parens` variant, which authors the parens on the *infer*
/// rather than on the function type.
fn return_type_is_constrained_infer(return_type: &ast::TSTypeAnnotation<'_>) -> bool {
    let mut ty = unwrap_parenthesized(return_type.type_annotation);
    if let TSType::TypePredicate(pred) = ty {
        match pred.type_annotation {
            Some(inner) => ty = unwrap_parenthesized(inner),
            None => return false,
        }
    }
    match ty {
        TSType::Infer(_) => is_constrained_infer(ty),
        TSType::Function(f) => return_type_is_constrained_infer(&f.return_type),
        TSType::Constructor(c) => return_type_is_constrained_infer(&c.return_type),
        _ => false,
    }
}

/// Check if a type needs parentheses when used as the operand of a prefix type operator
/// (keyof, readonly, unique). Without parens: `keyof A | B` parses as `(keyof A) | B`,
/// and lower-precedence operands lose their meaning entirely (`keyof (() => void)` →
/// the invalid `keyof () => void`). Matches Prettier's `TSTypeOperator` case in
/// `needs-parentheses.js` (parens when `parent.type === "TSTypeOperator"`).
pub(super) fn type_needs_parens_for_prefix_operator(p: &Printer<'_>, ts_type: &TSType<'_>) -> bool {
    let inner = p.printed_operand(ts_type);
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeOperator(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    )
}

/// Check if a type needs parentheses when used as a member of a union or
/// intersection. Function, constructor, and conditional types have lower
/// precedence than `|`/`&`; a nested union/intersection also keeps its parens
/// (`A | (B | C)`, `A & (B & C)`).
///
/// Both operators share one rule because Prettier does: `TSFunctionType`,
/// `TSConstructorType`, `TSConditionalType`, `TSUnionType`, and
/// `TSIntersectionType` all fall through to the same check in
/// `needs-parentheses.js` — `isUnionType(parent) || isIntersectionType(parent)`.
///
/// A **constrained** `infer U extends T` is the one extra case: its `extends`
/// constraint greedily absorbs a following `|`/`&` (`infer U extends A | B` ⇒
/// constraint `A | B`), so as a union/intersection member it must keep its parens
/// or the constraint silently widens (`(infer U extends number) | { a: 1 }` → the
/// constraint would become `number | { a: 1 }`). A bare `infer U` (no constraint)
/// has nothing to absorb and needs no parens. Matches Prettier's dedicated
/// `TSInferType` arm in `needs-parentheses.js` — parens when the node is a `types`
/// member of a union/intersection and `node.typeParameter.constraint` is set.
pub(super) fn type_needs_parens_in_union_or_intersection(
    p: &Printer<'_>,
    ts_type: &TSType<'_>,
) -> bool {
    // A degenerate ONE-element intersection/union (`& b`, `| b` — the leading-operator
    // syntax) is transparent: it prints as just its member (prettier collapses it), so
    // the parens decision applies to that member, not the one-element wrapper. Without
    // this, `a | & b` wraps the member as if it were a real intersection → `a | (b)`,
    // where prettier emits `a | b`. A multi-element intersection/union keeps its parens.
    let inner = p.printed_operand(ts_type);
    match inner {
        TSType::Union(_)
        | TSType::Intersection(_)
        | TSType::Function(_)
        | TSType::Constructor(_)
        | TSType::Conditional(_) => true,
        other => is_constrained_infer(other),
    }
}

//
// Nested brace types
//

/// Whether one of `literal`'s members holds a non-empty **brace-delimited** type
/// (`TypeLiteral` / `Mapped`; an empty `{}` has nothing to break) anywhere in its types, a
/// function type's parameter annotations and return type included — a nested `{` whose own
/// print-width break the next pass reads as authored, since a brace type keeps an authored
/// expansion ([`Printer::literal_authored_expanded`]).
///
/// Asked of the AST, never the source: a `{` inside a string-literal type, a template
/// type's text or a comment opens nothing that breaks.
pub(in crate::printer) fn type_literal_members_hold_brace_type(
    literal: &ast::TSTypeLiteral<'_>,
) -> bool {
    literal.members.iter().any(type_element_holds_brace_type)
}

/// [`type_literal_members_hold_brace_type`] for one member.
fn type_element_holds_brace_type(member: &ast::TSTypeElement<'_>) -> bool {
    use ast::TSTypeElement;
    match member {
        TSTypeElement::PropertySignature(p) => {
            annotation_holds_brace_type(p.type_annotation.as_ref())
        }
        TSTypeElement::MethodSignature(m) => {
            signature_holds_brace_type(m.type_parameters.as_ref(), m.params, m.return_type.as_ref())
        }
        TSTypeElement::CallSignature(c) => {
            signature_holds_brace_type(c.type_parameters.as_ref(), c.params, c.return_type.as_ref())
        }
        TSTypeElement::ConstructSignature(c) => {
            signature_holds_brace_type(c.type_parameters.as_ref(), c.params, c.return_type.as_ref())
        }
        TSTypeElement::IndexSignature(i) => {
            annotation_holds_brace_type(i.type_annotation.as_ref())
                || i.parameters
                    .iter()
                    .any(|param| annotation_holds_brace_type(param.type_annotation()))
        }
    }
}

fn annotation_holds_brace_type(annotation: Option<&ast::TSTypeAnnotation<'_>>) -> bool {
    annotation.is_some_and(|a| type_holds_brace_type(a.type_annotation))
}

fn signature_holds_brace_type(
    type_parameters: Option<&ast::TSTypeParameterDeclaration<'_>>,
    params: &[ast::Expression<'_>],
    return_type: Option<&ast::TSTypeAnnotation<'_>>,
) -> bool {
    type_parameters.is_some_and(type_parameters_hold_brace_type)
        || params.iter().any(param_holds_brace_type)
        || annotation_holds_brace_type(return_type)
}

fn type_parameters_hold_brace_type(decl: &ast::TSTypeParameterDeclaration<'_>) -> bool {
    decl.params.iter().any(type_parameter_holds_brace_type)
}

fn type_parameter_holds_brace_type(param: &ast::TSTypeParameter<'_>) -> bool {
    param.constraint.is_some_and(type_holds_brace_type)
        || param.default.is_some_and(type_holds_brace_type)
}

fn type_arguments_hold_brace_type(args: Option<&ast::TSTypeParameterInstantiation<'_>>) -> bool {
    args.is_some_and(|args| args.params.iter().any(|ty| type_holds_brace_type(ty)))
}

/// A signature parameter's annotation. A destructuring pattern itself is not a brace type:
/// its width break does not read back as authored, so only its annotation is walked.
fn param_holds_brace_type(param: &ast::Expression<'_>) -> bool {
    use ast::ExpressionKind;
    match &param.kind {
        ExpressionKind::Identifier(id) => annotation_holds_brace_type(id.type_annotation()),
        ExpressionKind::ObjectPattern(p) => annotation_holds_brace_type(p.type_annotation.as_ref()),
        ExpressionKind::ArrayPattern(p) => annotation_holds_brace_type(p.type_annotation.as_ref()),
        ExpressionKind::RestElement(r) => {
            param_holds_brace_type(r.argument)
                || annotation_holds_brace_type(r.type_annotation.as_ref())
        }
        ExpressionKind::AssignmentPattern(p) => param_holds_brace_type(p.left),
        _ => false,
    }
}

/// [`type_literal_members_hold_brace_type`] for one type, the type itself included.
fn type_holds_brace_type(ty: &TSType<'_>) -> bool {
    match ty {
        // An empty literal has nothing to break.
        TSType::TypeLiteral(l) => !l.members.is_empty(),
        TSType::Mapped(_) => true,
        TSType::Keyword(_) | TSType::ThisType(_) => false,
        TSType::Literal(TSLiteralType::TemplateLiteral(t)) => {
            t.types.iter().any(|ty| type_holds_brace_type(ty))
        }
        TSType::Literal(_) => false,
        TSType::Array(a) => type_holds_brace_type(a.element_type),
        TSType::Union(TSUnionType { types, .. })
        | TSType::Intersection(TSIntersectionType { types, .. }) => {
            types.iter().any(|ty| type_holds_brace_type(ty))
        }
        TSType::Tuple(t) => t.element_types.iter().any(|ty| type_holds_brace_type(ty)),
        TSType::TypeReference(r) => type_arguments_hold_brace_type(r.type_arguments.as_ref()),
        TSType::Function(f) => {
            signature_holds_brace_type(f.type_parameters.as_ref(), f.params, Some(&f.return_type))
        }
        TSType::Constructor(c) => {
            signature_holds_brace_type(c.type_parameters.as_ref(), c.params, Some(&c.return_type))
        }
        TSType::Parenthesized(TSParenthesizedType {
            type_annotation, ..
        }) => type_holds_brace_type(type_annotation),
        TSType::TypePredicate(p) => p.type_annotation.is_some_and(type_holds_brace_type),
        TSType::Conditional(c) => [c.check_type, c.extends_type, c.true_type, c.false_type]
            .into_iter()
            .any(type_holds_brace_type),
        TSType::TypeOperator(o) => type_holds_brace_type(o.type_annotation),
        // An import type's options object (`import('m', { with: … })`) is an expression,
        // not a type; only its type arguments are walked.
        TSType::Import(i) => type_arguments_hold_brace_type(i.type_arguments.as_ref()),
        TSType::TypeQuery(q) => {
            type_arguments_hold_brace_type(q.type_arguments.as_ref())
                || matches!(&q.expr_name, ast::TSTypeQueryExprName::Import(i)
                    if type_arguments_hold_brace_type(i.type_arguments.as_ref()))
        }
        TSType::IndexedAccess(i) => {
            type_holds_brace_type(i.object_type) || type_holds_brace_type(i.index_type)
        }
        TSType::Rest(r) => type_holds_brace_type(r.type_annotation),
        TSType::Optional(o) => type_holds_brace_type(o.type_annotation),
        TSType::NamedTupleMember(n) => type_holds_brace_type(n.element_type),
        TSType::Infer(i) => type_parameter_holds_brace_type(&i.type_parameter),
    }
}
