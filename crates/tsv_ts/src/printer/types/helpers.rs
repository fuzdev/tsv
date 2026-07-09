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

/// Prettier's `shouldHugUnionType` criterion for a single type argument: a union with
/// exactly one brace-delimited member and only void-like siblings (`{…} | null`,
/// `null | {…}`, `{…} | void`), per [`should_hug_union_type`]. Such a union inlines
/// atomically — the object member carries its own group and breaks block-style inside
/// the hugged `<…>`, so the `<…>` itself never needs a break point. Parenthesized
/// wrappers are unwrapped first. The single source of truth shared by the
/// call/`new`/instantiation type-argument builder and the type-position builder so the
/// two agree by construction. Prettier ref: `shouldHugType` → `shouldHugUnionType`.
pub fn is_hugging_union_type_arg(ty: &TSType<'_>) -> bool {
    matches!(unwrap_parenthesized(ty), TSType::Union(u)
        if should_hug_union_type(u)
            && u.types.iter().any(|t| matches!(t, TSType::TypeLiteral(_) | TSType::Mapped(_))))
}

/// Find the `TSParenthesizedType` that directly wraps a union, walking through any
/// redundant nested parens. Returns `None` when `ts_type` is a bare union (the parens
/// are synthetic, added by the printer for precedence — no source comments to preserve).
///
/// Used to recover the paren span so `build_parenthesized_union_doc` can emit comments
/// the user wrote inside retained parens (`(/* c */ a | b)`, `(a | b /* c */)`).
pub(super) fn immediate_union_paren<'a>(
    ts_type: &'a TSType<'a>,
) -> Option<&'a internal::TSParenthesizedType<'a>> {
    match ts_type {
        TSType::Parenthesized(p) => match p.type_annotation {
            TSType::Union(_) => Some(p),
            inner @ TSType::Parenthesized(_) => immediate_union_paren(inner),
            _ => None,
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

/// Check if a union type should be "hugged" — formatted as `A | B | C` inline
/// even when it breaks, rather than using the multi-line `| A\n| B\n| C` format.
///
/// Matches Prettier's `shouldHugUnionType`: hugs when there's exactly one
/// object-like type (TSTypeLiteral or TSTypeReference) and all other members
/// are void types (void, null).
///
/// Example: `{ name: string; value: number } | null` stays hugged.
pub fn should_hug_union_type(union: &TSUnionType<'_>) -> bool {
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
        TSType::Infer(i) => i.type_parameter.constraint.is_some(),
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
pub(super) fn type_needs_parens_in_union_or_intersection(ts_type: &TSType<'_>) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
            | TSType::Conditional(_)
    )
}

/// Member-parens predicate for a *single-member* union/intersection. Prettier
/// drops single-element union/intersection nodes in postprocess, so the lone
/// member prints in the union's own position and needs no precedence parens of
/// its own — any required parens come from the union's parent context, applied
/// one level up. Pairs with `type_needs_parens_in_union_or_intersection` (used
/// for 2+ members).
pub(super) fn type_never_needs_parens(_ts_type: &TSType<'_>) -> bool {
    false
}
