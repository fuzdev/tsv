// Helper functions for type printing
//
// Standalone functions that don't require Printer state:
// - Type parenthesization predicates
// - Type unwrapping utilities
// - Source scanning helpers

use crate::ast::internal::{
    self, TSIntersectionType, TSKeywordKind, TSLiteralType, TSType, TSUnionType,
};
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
    args: &internal::TSTypeParameterInstantiation<'_>,
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

/// Prettier's `isSimpleType` inline/hug criterion for a single type argument: an
/// atomic type that never benefits from breaking — a primitive keyword, a plain
/// literal (string / number / bigint / negative), `this`, or a bare type reference
/// with no type arguments of its own (a nested `Array<X>` is *not* simple).
/// Parenthesized wrappers are unwrapped first. The single source of truth shared by
/// the call/`new`/instantiation type-argument builder and the type-position builder so
/// the two agree by construction. Prettier ref: `utilities/is-simple-type.js` (booleans
/// are `TSType::Keyword` here, so they fall under the keyword arm rather than `Literal`).
///
/// Template-literal types are **excluded** even though Prettier's `isSimpleType` accepts
/// them: tsv's template-literal-type printer carries an internal `${…}` break point, so
/// inlining the `<…>` would let the template break *there* (defeating the atomicity the
/// caller relies on). They stay in the breakable group path — a residual divergence
/// pending an atomic template-literal-type printer.
pub fn is_simple_type_arg(ty: &TSType<'_>) -> bool {
    let unwrapped = unwrap_parenthesized(ty);
    matches!(unwrapped, TSType::Keyword(_) | TSType::ThisType(_))
        || matches!(unwrapped, TSType::Literal(lit) if !matches!(lit, TSLiteralType::TemplateLiteral(_)))
        || matches!(unwrapped, TSType::TypeReference(r) if r.type_arguments.is_none())
}

/// Whether any union member is **brace-delimited** (`TypeLiteral`/`Mapped`) — the extra
/// narrowing a *type-argument* hug requires on top of [`union_hug_shape`]: the object
/// member carries its own group and breaks block-style inside the hugged `<…>`, so the
/// `<…>` itself never needs a break point.
///
/// **Deliberately stricter than** prettier's `isObjectLikeType`, which also accepts a bare
/// `TSTypeReference`: excluding it is the sanctioned `return_type_generic_union` print-width
/// family (a `Promise<…> | null` argument keeps its break point). Don't widen it to match
/// prettier.
///
/// Sole caller is
/// [`Printer::type_arg_union_prints_hugged`](super::super::Printer::type_arg_union_prints_hugged)
/// — this is one *clause* of that gate, never the gate. It is safe to ask bare (unlike
/// [`union_hug_shape`], it makes no claim to answer "does this hug?").
pub(super) fn union_has_brace_member(union: &TSUnionType<'_>) -> bool {
    union
        .types
        .iter()
        .any(|t| matches!(t, TSType::TypeLiteral(_) | TSType::Mapped(_)))
}

/// Find the `TSParenthesizedType` that directly wraps `ts_type`'s underlying type,
/// walking through any redundant nested parens. Returns `None` when `ts_type` is not
/// parenthesized in source (the parens are synthetic, added by the printer for
/// precedence — there is no author gap, so no comments to preserve).
///
/// Used to recover the paren span so the paren-retaining member printers can emit
/// comments the user wrote inside retained parens — `build_parenthesized_union_doc`
/// (`(/* c */ a | b)`, `(a | b /* c */)`) and
/// `build_parenthesized_intersection_trailing_object_doc` (`(// c⏎a & { … })`). Both are
/// handed their already-unwrapped inner type, so the paren's own gap is invisible to them
/// otherwise, and a comment in it would be silently dropped.
pub(super) fn immediate_paren<'a>(
    ts_type: &'a TSType<'a>,
) -> Option<&'a internal::TSParenthesizedType<'a>> {
    match ts_type {
        TSType::Parenthesized(p) => match p.type_annotation {
            inner @ TSType::Parenthesized(_) => immediate_paren(inner),
            _ => Some(p),
        },
        _ => None,
    }
}

/// Check if a type is "huggable" - brace-delimited types that expand internally.
///
/// TypeLiteral (`{ a: T }`) and Mapped (`{ [K in T]: V }`) types are huggable:
/// they handle their own expansion and should keep `{` hugged to the context.
#[inline]
pub fn is_huggable_type(ts_type: &TSType<'_>) -> bool {
    matches!(ts_type, TSType::TypeLiteral(_) | TSType::Mapped(_))
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
/// (see [`Self::union_return_hugs`](super::super::Printer::union_return_hugs) and
/// [`Printer::type_arg_union_prints_hugged`](super::super::Printer::type_arg_union_prints_hugged)
/// for the two shapes that do).
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
#[inline]
fn is_object_like_type(ts_type: &TSType<'_>) -> bool {
    matches!(ts_type, TSType::TypeLiteral(_) | TSType::TypeReference(_))
}

/// Check if a type is a "void type" for union hugging purposes.
/// Matches Prettier's `isVoidType`: void and null keywords.
#[inline]
fn is_void_type(ts_type: &TSType<'_>) -> bool {
    matches!(
        ts_type,
        TSType::Keyword(kw) if matches!(kw.kind, TSKeywordKind::Void | TSKeywordKind::Null)
    )
}

/// Check if the last type in an intersection is "huggable" (like TypeLiteral or MappedType).
///
/// Huggable types expand independently and should not have breaks/indent applied
/// around them in the parent context. This keeps patterns like `& {` hugged together.
#[inline]
pub fn intersection_has_huggable_last_type(intersection: &TSIntersectionType<'_>) -> bool {
    intersection
        .types
        .last()
        .is_some_and(|t| is_huggable_type(unwrap_parenthesized(t)))
}

/// Check if the first type in an intersection is "expanding" (like TypeLiteral or MappedType).
///
/// When the first type expands (contains hardlines from multiline object body),
/// the continuation should use space instead of line to keep `} & Type` together.
/// This is the mirror of `intersection_has_huggable_last_type` for the first position.
#[inline]
pub fn intersection_has_expanding_first_type(intersection: &TSIntersectionType<'_>) -> bool {
    intersection
        .types
        .first()
        .is_some_and(|t| is_huggable_type(unwrap_parenthesized(t)))
}

//
// Type parenthesization predicates
//

/// Check if a type needs parentheses when used as the object in indexed access (`T[K]`).
/// Without parens: `A | B[K]` parses as `A | (B[K])`, not `(A | B)[K]`
pub(super) fn type_needs_parens_for_indexed_access_object(ts_type: &TSType<'_>) -> bool {
    let inner = unwrap_parenthesized(ts_type);
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
    )
}

/// Check if a type needs parentheses when used as the element type in an array (`T[]`).
/// Without parens: `A | B[]` parses as `A | (B[])`, not `(A | B)[]`
pub(super) fn type_needs_parens_for_array_element(ts_type: &TSType<'_>) -> bool {
    let inner = unwrap_parenthesized(ts_type);
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
    )
}

/// Check if a type needs parentheses when used as an optional tuple element (`[T?]`).
/// Matches Prettier's `TSOptionalType` parent rule in `needs-parentheses.js`
/// (union/intersection plus the `TSTypeOperator`-case fall-through). Without
/// parens the `?` rebinds: `[() => void?]` / `[A | B?]` are invalid or change
/// meaning.
pub(super) fn type_needs_parens_for_optional_element(ts_type: &TSType<'_>) -> bool {
    let inner = unwrap_parenthesized(ts_type);
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
pub(super) fn type_needs_parens_for_conditional_check(ts_type: &TSType<'_>) -> bool {
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
pub(super) fn type_needs_parens_for_conditional_extends(ts_type: &TSType<'_>) -> bool {
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
fn return_type_is_constrained_infer(return_type: &internal::TSTypeAnnotation<'_>) -> bool {
    let mut ty = return_type.type_annotation;
    if let TSType::TypePredicate(pred) = ty {
        match pred.type_annotation {
            Some(inner) => ty = inner,
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
pub(super) fn type_needs_parens_for_prefix_operator(ts_type: &TSType<'_>) -> bool {
    let inner = unwrap_parenthesized(ts_type);
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
pub(super) fn type_needs_parens_in_union_or_intersection(ts_type: &TSType<'_>) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    // A degenerate ONE-element intersection/union (`& b`, `| b` — the leading-operator
    // syntax) is transparent: it prints as just its member (prettier collapses it), so
    // the parens decision applies to that member, not the one-element wrapper. Without
    // this, `a | & b` wraps the member as if it were a real intersection → `a | (b)`,
    // where prettier emits `a | b`. A multi-element intersection/union keeps its parens.
    if let Some(single) = single_member_composite(inner) {
        return type_needs_parens_in_union_or_intersection(single);
    }
    match inner {
        TSType::Union(_)
        | TSType::Intersection(_)
        | TSType::Function(_)
        | TSType::Constructor(_)
        | TSType::Conditional(_) => true,
        other => is_constrained_infer(other),
    }
}

/// The single member of a one-element `TSUnionType` / `TSIntersectionType`, else `None`.
/// A one-element composite is semantically just its member (the leading-`|`/`&` syntax),
/// and prettier collapses it, so callers see through it.
fn single_member_composite<'a>(ts_type: &'a TSType<'a>) -> Option<&'a TSType<'a>> {
    match ts_type {
        TSType::Union(u) if u.types.len() == 1 => Some(&u.types[0]),
        TSType::Intersection(i) if i.types.len() == 1 => Some(&i.types[0]),
        _ => None,
    }
}
