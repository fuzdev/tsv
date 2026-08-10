//! Destructuring patterns
//!
//! Contains object patterns, array patterns, assignment patterns,
//! and rest elements for destructuring declarations and assignments.

use tsv_lang::Span;

use super::{Decorator, Expression, Property, TSTypeAnnotation};

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
    /// Optional destructuring-pattern parameter (`{a}?`). Only ever set in a
    /// parameter position; the `?` extends `span` and precedes `type_annotation`.
    pub optional: bool,
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    /// Parameter decorators (`@dec { a }: T`). Only set in a parameter position;
    /// emitted last in the wire, matching acorn (which attaches a parameter's
    /// decorators to its top-level binding node).
    pub decorators: Option<&'arena [Decorator<'arena>]>,
    pub span: Span,
}

impl ObjectPattern<'_> {
    /// Where the `{…}` body ENDS — the closing brace, or the start of a `: T` annotation
    /// the pattern's span swallowed.
    ///
    /// The bound every comment scan over the pattern takes, and the link is load-bearing:
    /// the annotation prints its own comments, so a range run to `span.end` claims one the
    /// annotation's doc also prints. A `?` is inside the body either way (it precedes the
    /// annotation and rides inside `span`), which is the pre-existing reading — no comment
    /// can sit between the `}` and it that the brace scan doesn't already own.
    pub fn body_end(&self) -> u32 {
        self.type_annotation
            .as_ref()
            .map_or(self.span.end, |t| t.span.start)
    }
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

    /// Where the property's doc STOPS PRINTING — the pattern counterpart of
    /// [`super::ObjectProperty::value_end`], and the anchor its trailing-comment scan
    /// takes (`Expression::printed_end` carries the argument).
    ///
    /// Always the VALUE's end, never the key's, even for a shorthand property: `{a = (1)}`
    /// is shorthand with an `AssignmentPattern` value, so a key anchor would open the scan
    /// over `= 1` — text the property's own doc prints, and where a comment before the `=`
    /// (`{a /* c */ = 1}`) would then be printed twice. The object literal can take the key
    /// there only because its shorthand carries no value text at all.
    pub fn value_end(&self) -> u32 {
        match self {
            ObjectPatternProperty::Property(p) => p.value.printed_end(),
            // The rest element's own end: its stripped-paren interior is its doc's share.
            ObjectPatternProperty::RestElement(r) => r.span.end,
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
    /// Optional destructuring-pattern parameter (`[a]?`). Only ever set in a
    /// parameter position; the `?` extends `span` and precedes `type_annotation`.
    pub optional: bool,
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    /// Parameter decorators (`@dec [a]: T`). Only set in a parameter position;
    /// emitted last in the wire, matching acorn.
    pub decorators: Option<&'arena [Decorator<'arena>]>,
    pub span: Span,
}

impl ArrayPattern<'_> {
    /// Where the `[…]` body ENDS — the object pattern's [`ObjectPattern::body_end`] in its
    /// bracket spelling, and load-bearing for the same reason.
    pub fn body_end(&self) -> u32 {
        self.type_annotation
            .as_ref()
            .map_or(self.span.end, |t| t.span.start)
    }
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
    /// Parameter decorators (`@dec a = 1`, `@dec { a } = {}`). Only set in a
    /// parameter position; emitted last in the wire — acorn attaches a decorated
    /// default parameter's decorators to the `AssignmentPattern`, not its `left`.
    pub decorators: Option<&'arena [Decorator<'arena>]>,
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
    /// Optional rest parameter (`...a?`). Only ever set in a parameter position;
    /// the `?` extends `span` and precedes `type_annotation`. `...a?` is invalid
    /// TypeScript (tsc TS1047, a *deferred* grammar-check like TS1051 `set x(a?)`),
    /// so tsv parses and preserves it — acorn's shape carries `optional` on the
    /// rest element (never on `argument`). Never set for a destructuring rest
    /// (`[...a]` / `{...a}`), which takes no `?`.
    pub optional: bool,
    // Inline by value — see `TSFunctionType.return_type`; `TSTypeAnnotation` is held
    // inline (`Option<TSTypeAnnotation>`) everywhere else.
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    pub span: Span,
}
