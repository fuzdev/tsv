//! Class-related types for public AST

use serde::Serialize;

use super::statements::BlockStatement;
use super::types::{
    TSIndexSignature, TSTypeAnnotation, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
};
use super::{Decorator, Expression, Identifier, SourceLocation};

/// Class declaration: `class Foo { ... }`
/// For `export default class {}`, id is null.
#[derive(Debug, Clone, Serialize)]
pub struct ClassDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Decorators applied to this class
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator<'src>>>,
    /// Whether this is a declare class (ambient declaration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declare: Option<bool>,
    /// Whether this is an abstract class
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_: Option<bool>,
    /// Class name (None for anonymous export default classes)
    pub id: Option<Identifier<'src>>,
    /// Type parameters (e.g., `<T>` in `class Foo<T>`)
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "superClass")]
    pub super_class: Option<Box<Expression<'src>>>,
    /// Type arguments for superclass (e.g., `<T>` in `extends Base<T>`)
    #[serde(
        rename = "superTypeParameters",
        skip_serializing_if = "Option::is_none"
    )]
    pub super_type_parameters: Option<TSTypeParameterInstantiation<'src>>,
    /// Implements clause: `implements Foo, Bar`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<Vec<TSExpressionWithTypeArguments<'src>>>,
    pub body: ClassBody<'src>,
}

/// Class expression: `class { }` or `class Foo<T> extends Bar { }`
#[derive(Debug, Clone, Serialize)]
pub struct ClassExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Decorators applied to this class
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator<'src>>>,
    /// Whether this is an abstract class
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub abstract_: Option<bool>,
    /// Class name (always optional for expressions)
    pub id: Option<Identifier<'src>>,
    /// Type parameters (e.g., `<T>` in `class Foo<T>`)
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "superClass")]
    pub super_class: Option<Box<Expression<'src>>>,
    /// Type arguments for superclass (e.g., `<T>` in `extends Base<T>`)
    #[serde(
        rename = "superTypeParameters",
        skip_serializing_if = "Option::is_none"
    )]
    pub super_type_parameters: Option<TSTypeParameterInstantiation<'src>>,
    /// Implements clause: `implements Foo, Bar`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<Vec<TSExpressionWithTypeArguments<'src>>>,
    pub body: ClassBody<'src>,
}

/// Class body: `{ constructor() {} method() {} prop = value; }`
#[derive(Debug, Clone, Serialize)]
pub struct ClassBody<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<ClassMember<'src>>,
}

/// Class member - method definition, property definition, or static block
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ClassMember<'src> {
    MethodDefinition(MethodDefinition<'src>),
    PropertyDefinition(PropertyDefinition<'src>),
    StaticBlock(StaticBlock<'src>),
    TSIndexSignature(TSIndexSignature<'src>),
}

/// Static initialization block in a class: `static { ... }` (ES2022)
#[derive(Debug, Clone, Serialize)]
pub struct StaticBlock<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub body: Vec<super::Statement<'src>>,
}

/// Method definition in a class body
#[derive(Debug, Clone, Serialize)]
pub struct MethodDefinition<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Decorators applied to this method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator<'src>>>,
    /// Accessibility modifier (public, private, protected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<&'static str>,
    /// Whether this is an abstract method (no body)
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub is_abstract: Option<bool>,
    #[serde(rename = "static")]
    pub is_static: bool,
    /// Whether this method overrides a base class method
    #[serde(rename = "override", skip_serializing_if = "super::is_false")]
    pub is_override: bool,
    /// Whether this is an optional method (`m?()`); emitted only when true
    #[serde(rename = "optional", skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    pub computed: bool,
    pub key: Box<Expression<'src>>,
    pub kind: &'static str,
    /// Type parameters for the method (moved from FunctionExpression)
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    pub value: MethodValue<'src>,
}

/// Either a FunctionExpression (for regular methods) or TSDeclareMethod (for abstract/overload methods)
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MethodValue<'src> {
    FunctionExpression(FunctionExpression<'src>),
    TSDeclareMethod(TSDeclareMethod<'src>),
}

/// TSDeclareMethod: abstract method or overload signature (no body)
#[derive(Debug, Clone, Serialize)]
pub struct TSDeclareMethod<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Option<Identifier<'src>>,
    pub expression: bool,
    pub generator: bool,
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Function parameters
    pub params: Vec<Expression<'src>>,
    /// Return type annotation
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation<'src>>,
}

/// Property definition in a class body: `name = value;`
#[derive(Debug, Clone, Serialize)]
pub struct PropertyDefinition<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Decorators applied to this property
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<Decorator<'src>>>,
    /// Whether this is an abstract property (no initializer)
    #[serde(rename = "abstract", skip_serializing_if = "Option::is_none")]
    pub is_abstract: Option<bool>,
    /// Whether this property uses the accessor keyword (ES decorator proposal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessor: Option<bool>,
    /// Accessibility modifier (public, private, protected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<&'static str>,
    /// Whether this is a readonly property
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<bool>,
    /// Whether this property has the override modifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#override: Option<bool>,
    /// Whether this property has the declare modifier (ambient)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declare: Option<bool>,
    #[serde(rename = "static")]
    pub is_static: bool,
    pub computed: bool,
    pub key: Box<Expression<'src>>,
    /// Whether this is an optional property (`a?: string`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    /// Whether this has definite assignment assertion (`a!: string`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definite: Option<bool>,
    /// Type annotation (e.g., `: number`)
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation<'src>>,
    pub value: Option<Box<Expression<'src>>>,
}

/// Function expression: `function() {}` or method shorthand `{ foo() {} }`
#[derive(Debug, Clone, Serialize)]
pub struct FunctionExpression<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Option<Identifier<'src>>,
    pub expression: bool,
    pub generator: bool,
    #[serde(rename = "async")]
    pub is_async: bool,
    /// Type parameters (TypeScript generics): `function<T>() {}`
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    /// Function parameters (Identifier, ArrayPattern, ObjectPattern, or AssignmentPattern for defaults)
    pub params: Vec<Expression<'src>>,
    /// Return type annotation (e.g., `: number`)
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation<'src>>,
    pub body: BlockStatement<'src>,
}

/// Expression with type arguments for implements clause: `implements Foo<T>`
#[derive(Debug, Clone, Serialize)]
pub struct TSExpressionWithTypeArguments<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression<'src>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterInstantiation<'src>>,
}

/// TypeScript parameter property: `constructor(public x: number)`
#[derive(Debug, Clone, Serialize)]
pub struct TSParameterProperty<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Accessibility modifier: "public", "private", or "protected"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<&'static str>,
    /// Whether the parameter is readonly
    #[serde(skip_serializing_if = "super::is_false")]
    pub readonly: bool,
    /// Whether the parameter property carries the `override` modifier
    #[serde(rename = "override", skip_serializing_if = "super::is_false")]
    pub r#override: bool,
    /// The parameter - can be Identifier or AssignmentPattern (with default value)
    pub parameter: Box<Expression<'src>>,
}
