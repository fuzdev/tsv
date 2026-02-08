// Helper functions for type printing
//
// Standalone functions that don't require Printer state:
// - Type parenthesization predicates
// - Type unwrapping utilities
// - Source scanning helpers

use crate::ast::internal::{self, TSIntersectionType, TSType};

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
    args: &internal::TSTypeParameterInstantiation,
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
    crate::printer::analysis::find_char_skipping_comments(
        source.as_bytes(),
        start as usize,
        end as usize,
        separator,
    )
    .map(|pos| pos as u32)
}

//
// Type unwrapping
//

/// Recursively unwrap TSParenthesizedType to get the inner type.
pub fn unwrap_parenthesized(ts_type: &TSType) -> &TSType {
    match ts_type {
        TSType::Parenthesized(p) => unwrap_parenthesized(&p.type_annotation),
        _ => ts_type,
    }
}

/// Check if a type is "huggable" - brace-delimited types that expand internally.
///
/// TypeLiteral (`{ a: T }`) and Mapped (`{ [K in T]: V }`) types are huggable:
/// they handle their own expansion and should keep `{` hugged to the context.
#[inline]
pub fn is_huggable_type(ts_type: &TSType) -> bool {
    matches!(ts_type, TSType::TypeLiteral(_) | TSType::Mapped(_))
}

/// Check if the last type in an intersection is "huggable" (like TypeLiteral or MappedType).
///
/// Huggable types expand independently and should not have breaks/indent applied
/// around them in the parent context. This keeps patterns like `& {` hugged together.
#[inline]
pub fn intersection_has_huggable_last_type(intersection: &TSIntersectionType) -> bool {
    intersection
        .types
        .last()
        .is_some_and(|t| is_huggable_type(unwrap_parenthesized(t)))
}

//
// Type parenthesization predicates
//

/// Check if a type needs parentheses when used as the object in indexed access (`T[K]`).
/// Without parens: `A | B[K]` parses as `A | (B[K])`, not `(A | B)[K]`
pub(super) fn type_needs_parens_for_indexed_access_object(ts_type: &TSType) -> bool {
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
pub(super) fn type_needs_parens_for_array_element(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    // TypeOperator excluded: `(readonly T)[]` is invalid TypeScript
    matches!(
        inner,
        TSType::Union(_)
            | TSType::Intersection(_)
            | TSType::TypeQuery(_)
            | TSType::Conditional(_)
            | TSType::Infer(_)
            | TSType::Function(_)
            | TSType::Constructor(_)
    )
}

/// Check if a type needs parentheses when used as the operand of a prefix type operator
/// (keyof, readonly, unique). Without parens: `keyof A | B` parses as `(keyof A) | B`
pub(super) fn type_needs_parens_for_prefix_operator(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(inner, TSType::Union(_) | TSType::Intersection(_))
}

/// Check if a type needs parentheses when used as a member of an intersection.
/// Union, function, constructor, and conditional types have lower precedence than `&`.
pub(super) fn type_needs_parens_in_intersection(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(
        inner,
        TSType::Union(_) | TSType::Function(_) | TSType::Constructor(_) | TSType::Conditional(_)
    )
}

/// Check if a type needs parentheses when used as a member of a union.
/// Function, constructor, and conditional types have lower precedence than `|`.
pub(super) fn type_needs_parens_in_union(ts_type: &TSType) -> bool {
    let inner = unwrap_parenthesized(ts_type);
    matches!(
        inner,
        TSType::Function(_)
            | TSType::Constructor(_)
            | TSType::Conditional(_)
            | TSType::Intersection(_)
    )
}
