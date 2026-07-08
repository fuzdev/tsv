//! Class declarations and class members
//!
//! Contains class declarations/expressions, class body, and all class member types
//! including methods, properties, static blocks, and accessors.

use tsv_lang::Span;

use super::{
    Decorator, Expression, FunctionExpression, Identifier, Statement, TSIndexSignature,
    TSInterfaceHeritage, TSTypeAnnotation, TSTypeParameterDeclaration,
    TSTypeParameterInstantiation,
};

/// Class declaration: `class Foo { ... }` or `class Foo extends Bar { ... }`
///
/// Represents a class declaration with optional superclass.
/// For `export default class {}`, the name is optional.
#[derive(Debug, Clone)]
pub struct ClassDeclaration<'arena> {
    /// Decorators applied to this class
    pub decorators: Option<&'arena [Decorator<'arena>]>,
    /// Class name (required for declarations, optional for export default)
    pub id: Option<Identifier<'arena>>,
    /// Optional superclass expression (for `extends`)
    pub super_class: Option<&'arena Expression<'arena>>,
    /// Type arguments for superclass (e.g., `<T>` in `extends Base<T>`)
    pub super_type_parameters: Option<TSTypeParameterInstantiation<'arena>>,
    /// Implements clause for declare class: `implements Foo, Bar`
    pub implements: &'arena [TSInterfaceHeritage<'arena>],
    /// Class body containing methods and properties
    pub body: ClassBody<'arena>,
    /// Whether this is a declare class (ambient declaration)
    pub declare: bool,
    /// Whether this is an abstract class
    pub r#abstract: bool,
    /// Type parameters (e.g., `<T>` in `class Foo<T>`)
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    pub span: Span,
}

/// Class expression: `class { }` or `class Foo<T> extends Bar { }`
///
/// Same as ClassDeclaration but used in expression position.
/// The name is always optional.
#[derive(Debug, Clone)]
pub struct ClassExpression<'arena> {
    /// Decorators applied to this class
    pub decorators: Option<&'arena [Decorator<'arena>]>,
    /// Class name (always optional for expressions)
    pub id: Option<Identifier<'arena>>,
    /// Optional superclass expression (for `extends`)
    pub super_class: Option<&'arena Expression<'arena>>,
    /// Type arguments for superclass (e.g., `<T>` in `extends Base<T>`)
    pub super_type_parameters: Option<TSTypeParameterInstantiation<'arena>>,
    /// Implements clause: `implements Foo, Bar`
    pub implements: &'arena [TSInterfaceHeritage<'arena>],
    /// Class body containing methods and properties
    pub body: ClassBody<'arena>,
    /// Whether this is an abstract class
    pub r#abstract: bool,
    /// Type parameters (e.g., `<T>` in `class Foo<T>`)
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    pub span: Span,
}

/// Class body: `{ constructor() {} method() {} prop = value; }`
///
/// Contains the methods and properties of a class.
#[derive(Debug, Clone)]
pub struct ClassBody<'arena> {
    pub body: &'arena [ClassMember<'arena>],
    pub span: Span,
}

/// Class member - method definition, property definition, or static block
#[derive(Debug, Clone)]
pub enum ClassMember<'arena> {
    MethodDefinition(MethodDefinition<'arena>),
    PropertyDefinition(PropertyDefinition<'arena>),
    StaticBlock(StaticBlock<'arena>),
    IndexSignature(TSIndexSignature<'arena>),
}

impl<'arena> ClassMember<'arena> {
    pub fn span(&self) -> Span {
        match self {
            ClassMember::MethodDefinition(m) => m.span,
            ClassMember::PropertyDefinition(p) => p.span,
            ClassMember::StaticBlock(s) => s.span,
            ClassMember::IndexSignature(i) => i.span,
        }
    }
}

/// Static initialization block in a class: `static { ... }` (ES2022)
#[derive(Debug, Clone)]
pub struct StaticBlock<'arena> {
    pub body: &'arena [Statement<'arena>],
    pub span: Span,
}

/// Method definition kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MethodKind {
    Constructor = 0,
    Method = 1,
    Get = 2,
    Set = 3,
}

impl MethodKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            MethodKind::Constructor => "constructor",
            MethodKind::Method => "method",
            MethodKind::Get => "get",
            MethodKind::Set => "set",
        }
    }
}

/// Accessibility modifier for class members: public, private, protected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accessibility {
    Public,
    Private,
    Protected,
}

impl Accessibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Accessibility::Public => "public",
            Accessibility::Private => "private",
            Accessibility::Protected => "protected",
        }
    }

    /// The keyword text with a trailing space (`"public "` / `"private "` /
    /// `"protected "`), for a printer that emits it as a member/parameter modifier.
    pub const fn as_keyword(self) -> &'static str {
        match self {
            Accessibility::Public => "public ",
            Accessibility::Private => "private ",
            Accessibility::Protected => "protected ",
        }
    }
}

/// TypeScript parameter property in constructor: `constructor(public x: number)`
#[derive(Debug, Clone)]
pub struct TSParameterProperty<'arena> {
    /// Accessibility modifier: public, private, protected
    pub accessibility: Option<Accessibility>,
    /// Whether the parameter is readonly
    pub readonly: bool,
    /// Whether the parameter property carries the `override` modifier
    pub r#override: bool,
    /// The actual parameter - can be Identifier or AssignmentPattern (with default value)
    pub parameter: &'arena Expression<'arena>,
    pub span: Span,
}

/// Method definition in a class body: `method() { ... }` or `get x() { ... }`
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // independent flags, not a state machine
pub struct MethodDefinition<'arena> {
    /// Decorators applied to this method
    pub decorators: Option<&'arena [Decorator<'arena>]>,
    /// Method name (key)
    pub key: Expression<'arena>,
    /// Method implementation (value)
    pub value: FunctionExpression<'arena>,
    /// Method kind (constructor, method, get, set)
    pub kind: MethodKind,
    /// Accessibility modifier (public, private, protected)
    pub accessibility: Option<Accessibility>,
    /// Whether this is a static method
    pub is_static: bool,
    /// Whether this method overrides a base class method
    pub r#override: bool,
    /// Whether this is an abstract method (no body)
    pub r#abstract: bool,
    /// Whether the key is computed (`[expr]()`)
    pub computed: bool,
    /// Whether this is an optional method (`m?()`) — valid in interfaces,
    /// abstract classes, and ambient (`declare`) classes
    pub optional: bool,
    pub span: Span,
}

/// Modifier for class property optionality/definiteness.
///
/// These are mutually exclusive syntactically - they occupy the same position
/// after the property name (`a?: T` vs `a!: T`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropertyModifier {
    /// No modifier (regular property)
    #[default]
    None,
    /// Optional property (`a?: string`)
    Optional,
    /// Definite assignment assertion (`a!: string`)
    Definite,
}

/// Property definition in a class body: `name = value;` or `name;`
///
/// Unlike methods, properties use `=` for initialization.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct PropertyDefinition<'arena> {
    /// Decorators applied to this property
    pub decorators: Option<&'arena [Decorator<'arena>]>,
    /// Property name (key)
    pub key: Expression<'arena>,
    /// Type annotation (e.g., `: number` in `a: number = 0;`)
    pub type_annotation: Option<TSTypeAnnotation<'arena>>,
    /// Optional initial value
    pub value: Option<Expression<'arena>>,
    /// Accessibility modifier (public, private, protected)
    pub accessibility: Option<Accessibility>,
    /// Whether this is a static property
    pub is_static: bool,
    /// Whether this property has the declare modifier (ambient)
    pub declare: bool,
    /// Whether this is an abstract property
    pub r#abstract: bool,
    /// Whether this property has the override modifier
    pub r#override: bool,
    /// Whether this is a readonly property
    pub readonly: bool,
    /// Whether the key is computed (`[expr] = value`)
    pub computed: bool,
    /// Whether this property uses the accessor keyword (ES decorator proposal)
    pub accessor: bool,
    /// Optional/definite modifier (`?` or `!` after property name)
    pub modifier: PropertyModifier,
    pub span: Span,
}
