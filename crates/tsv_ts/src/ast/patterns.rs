//! Destructuring patterns
//!
//! Contains object patterns, array patterns, assignment patterns,
//! and rest elements for destructuring declarations and assignments.

use crate::ast::AstView;
use tsv_lang::Span;

use super::{BindingExtraId, Decorator, ExprId, Property, Run, TSTypeAnnotation};

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
#[derive(Debug, Clone, Copy)]
pub struct ObjectPattern {
    pub properties: Run<ObjectPatternProperty>,
    /// Optional destructuring-pattern parameter (`{a}?`). Only ever set in a
    /// parameter position; the `?` extends the expression's span and precedes the type
    /// annotation.
    pub optional: bool,
    /// The type annotation (`{ a }: T`) and parameter decorators (`@dec { a }: T`), set
    /// only in a binding position, as a [`BindingExtra`] row — the store [`Identifier`]'s
    /// binding extras use. A minority of patterns carry one and a tiny share of expressions
    /// are patterns, while inline the pair would set the width of every `Expression`, so
    /// the unannotated pattern pays a 4 B id for them. Read through
    /// [`Self::type_annotation`] and [`Self::decorators`]; the decorators are emitted last
    /// in the wire, matching acorn (which attaches a parameter's decorators to its
    /// top-level binding node).
    ///
    /// [`Identifier`]: super::Identifier
    /// [`BindingExtra`]: super::BindingExtra
    pub extra: Option<BindingExtraId>,
}

impl ObjectPattern {
    /// The type annotation, if any, its extras read through `view`.
    #[inline]
    pub fn type_annotation<'a>(&self, view: AstView<'a>) -> Option<&'a TSTypeAnnotation> {
        view.opt_binding_extra(self.extra)
            .and_then(|e| e.type_annotation.as_ref())
    }

    /// The parameter decorators, if any, their extras read through `view`.
    #[inline]
    pub fn decorators<'a>(&self, view: AstView<'a>) -> Option<&'a [Decorator]> {
        view.opt_binding_extra(self.extra)
            .and_then(|e| e.decorators)
            .map(|list| view.decorators(list))
    }
}

/// Object pattern property - either a regular property or a rest element
#[derive(Debug, Clone, Copy)]
pub enum ObjectPatternProperty {
    Property(Property),
    RestElement(RestElement),
}

impl ObjectPatternProperty {
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
    /// (`{a /* c */ = 1}`) would then be RELOCATED past the initializer. The object literal
    /// reads the value's end for the same reason: its shorthand carries no value text of its
    /// own only in the *valid* spelling, and the permissively parsed `{ a = 1 }` — the
    /// literal shape of this very binding — is exactly where the key anchor cost it that
    /// comment.
    pub fn value_end(&self, view: AstView<'_>) -> u32 {
        match self {
            ObjectPatternProperty::Property(p) => view.expr(p.value).printed_end(view),
            // The rest element's own end: its stripped-paren interior is its doc's share.
            ObjectPatternProperty::RestElement(r) => r.span.end,
        }
    }

    /// The rest element's stripped-paren interior ([`RestElement::paren_interior`]), if this
    /// property is the rest — the pattern counterpart of
    /// [`super::ObjectProperty::paren_interior`].
    pub fn paren_interior(&self, view: AstView<'_>) -> Option<Span> {
        match self {
            ObjectPatternProperty::RestElement(r) => Some(r.paren_interior(view)),
            ObjectPatternProperty::Property(_) => None,
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
#[derive(Debug, Clone, Copy)]
pub struct ArrayPattern {
    /// Elements are Option to support holes like `[a, , b]`
    pub elements: Run<Option<ExprId>>,
    /// Optional destructuring-pattern parameter (`[a]?`). Only ever set in a
    /// parameter position; the `?` extends the expression's span and precedes the type
    /// annotation.
    pub optional: bool,
    /// The type annotation (`[a]: T`) and parameter decorators (`@dec [a]: T`), behind
    /// one id as [`ObjectPattern::extra`]'s are. Read through [`Self::type_annotation`]
    /// and [`Self::decorators`].
    pub extra: Option<BindingExtraId>,
}

impl ArrayPattern {
    /// The type annotation, if any, its extras read through `view`.
    #[inline]
    pub fn type_annotation<'a>(&self, view: AstView<'a>) -> Option<&'a TSTypeAnnotation> {
        view.opt_binding_extra(self.extra)
            .and_then(|e| e.type_annotation.as_ref())
    }

    /// The parameter decorators, if any, their extras read through `view`.
    #[inline]
    pub fn decorators<'a>(&self, view: AstView<'a>) -> Option<&'a [Decorator]> {
        view.opt_binding_extra(self.extra)
            .and_then(|e| e.decorators)
            .map(|list| view.decorators(list))
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
#[derive(Debug, Clone, Copy)]
pub struct AssignmentPattern {
    /// The binding (identifier or nested pattern)
    pub left: ExprId,
    /// The default value expression
    pub right: ExprId,
    /// Parameter decorators (`@dec a = 1`, `@dec { a } = {}`). Only set in a
    /// parameter position; emitted last in the wire — acorn attaches a decorated
    /// default parameter's decorators to the `AssignmentPattern`, not its `left`.
    pub decorators: Option<Run<Decorator>>,
}

/// Rest element in destructuring: `...rest`
///
/// Collects remaining elements in array or object destructuring:
/// - `const [a, ...rest] = arr` (rest gets remaining array elements)
/// - `const {a, ...rest} = obj` (rest gets remaining properties)
#[derive(Debug, Clone, Copy)]
pub struct RestElement {
    /// The binding for the rest (typically an identifier)
    pub argument: ExprId,
    /// Optional rest parameter (`...a?`). Only ever set in a parameter position;
    /// the `?` extends `span` and precedes `type_annotation`. `...a?` is invalid
    /// TypeScript (tsc TS1047, a *deferred* grammar-check like TS1051 `set x(a?)`),
    /// so tsv parses and preserves it — acorn's shape carries `optional` on the
    /// rest element (never on `argument`). Never set for a destructuring rest
    /// (`[...a]` / `{...a}`), which takes no `?`.
    pub optional: bool,
    /// The type annotation (`...a: T[]`), behind one id as [`ObjectPattern::extra`]'s is:
    /// a minority of rests carry one, and inline it would set the width of every
    /// `Expression` (a rest takes no decorators, so the row's are always `None`).
    /// Read through [`Self::type_annotation`].
    pub extra: Option<BindingExtraId>,
    pub span: Span,
}

impl RestElement {
    /// The type annotation, if any, its extras read through `view`.
    #[inline]
    pub fn type_annotation<'a>(&self, view: AstView<'a>) -> Option<&'a TSTypeAnnotation> {
        view.opt_binding_extra(self.extra)
            .and_then(|e| e.type_annotation.as_ref())
    }

    /// The source the grouping parens the parser stripped from the argument leave behind
    /// — `[argument end, span end)`, the region between the argument and the shell's last
    /// `)` (`[...(a /* c */)] = x`). A destructuring-assignment rest is the only one that
    /// can hold a shell (`AssignmentRestElement` takes a `DestructuringAssignmentTarget`);
    /// the spread twin is [`super::SpreadElement::paren_interior`].
    ///
    /// Empty for a rest carrying a `?` marker or a `: T` annotation: that tail is what
    /// extends its span there, and its gaps are the tail emitter's, never an interior.
    pub fn paren_interior(&self, view: AstView<'_>) -> Span {
        let arg_end = view.expr(self.argument).span().end;
        if self.optional || self.type_annotation(view).is_some() {
            Span::new(arg_end, arg_end)
        } else {
            Span::new(arg_end, self.span.end)
        }
    }
}
