//! Destructuring patterns
//!
//! Contains object patterns, array patterns, assignment patterns,
//! and rest elements for destructuring declarations and assignments.

use tsv_lang::Span;

use super::{Expression, Property, TSTypeAnnotation};

/// Object pattern for destructuring: `{a, b}`, `{a: x, b: y}`, `{...rest}`
///
/// Used as the left-hand side in destructuring assignments and declarations:
/// - `const {a, b} = obj`
/// - `({a, b} = obj)`
///
/// Properties can include:
/// - Shorthand: `{a}` (key equals value binding)
/// - Renamed: `{a: x}` (bind obj.a to variable x)
/// - Default values: `{a = 1}` (use 1 if obj.a is undefined)
/// - Rest: `{...rest}` (collect remaining properties)
#[derive(Debug, Clone)]
pub struct ObjectPattern<'arena> {
    pub properties: &'arena [ObjectPatternProperty<'arena>],
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    pub span: Span,
}

/// Object pattern property - either a regular property or a rest element
#[derive(Debug, Clone)]
pub enum ObjectPatternProperty<'arena> {
    Property(Property<'arena>),
    RestElement(RestElement<'arena>),
}

impl<'arena> ObjectPatternProperty<'arena> {
    pub fn span(&self) -> Span {
        match self {
            ObjectPatternProperty::Property(p) => p.span,
            ObjectPatternProperty::RestElement(r) => r.span,
        }
    }
}

/// Array pattern for destructuring: `[a, b]`, `[a, , b]`, `[...rest]`
///
/// Used as the left-hand side in destructuring assignments and declarations:
/// - `const [a, b] = arr`
/// - `([a, b] = arr)`
///
/// Elements can include:
/// - Identifiers: `[a, b]`
/// - Nested patterns: `[{a}, [b]]`
/// - Default values: `[a = 1]`
/// - Rest: `[...rest]`
/// - Holes: `[a, , b]` (skip element at index 1)
#[derive(Debug, Clone)]
pub struct ArrayPattern<'arena> {
    /// Elements are Option to support holes like `[a, , b]`
    pub elements: &'arena [Option<Expression<'arena>>],
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    pub span: Span,
}

/// Assignment pattern for default values in destructuring: `a = 1`
///
/// Used when a destructured variable has a default value:
/// - `const {a = 1} = obj`
/// - `const [a = 1] = arr`
/// - `function foo({a = 1}) {}`
///
/// The left side is the binding pattern, the right side is the default value.
#[derive(Debug, Clone)]
pub struct AssignmentPattern<'arena> {
    /// The binding (identifier or nested pattern)
    pub left: &'arena Expression<'arena>,
    /// The default value expression
    pub right: &'arena Expression<'arena>,
    pub span: Span,
}

/// Rest element in destructuring: `...rest`
///
/// Collects remaining elements in array or object destructuring:
/// - `const [a, ...rest] = arr` (rest gets remaining array elements)
/// - `const {a, ...rest} = obj` (rest gets remaining properties)
#[derive(Debug, Clone)]
pub struct RestElement<'arena> {
    /// The binding for the rest (typically an identifier)
    pub argument: &'arena Expression<'arena>,
    // Inline by value — see `TSFunctionType.return_type`; `TSTypeAnnotation` is held
    // inline (`Option<TSTypeAnnotation>`) everywhere else.
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    pub span: Span,
}
