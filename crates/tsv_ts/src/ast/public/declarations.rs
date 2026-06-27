//! TypeScript declaration types for public AST

use serde::Serialize;

use super::types::{
    TSEntityName, TSInterfaceBody, TSTypeAnnotation, TSTypeParameterDeclaration,
    TSTypeParameterInstantiation,
};
use super::{Expression, Identifier, Literal, SourceLocation, Statement};

/// TypeScript interface declaration: `interface Foo { ... }`
#[derive(Debug, Clone, Serialize)]
pub struct TSInterfaceDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier<'src>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    #[serde(rename = "extends", skip_serializing_if = "Vec::is_empty")]
    pub extends: Vec<TSInterfaceHeritage<'src>>,
    pub body: TSInterfaceBody<'src>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
}

/// Interface heritage: `extends Foo, Bar`
#[derive(Debug, Clone, Serialize)]
pub struct TSInterfaceHeritage<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: TSEntityName<'src>,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterInstantiation<'src>>,
}

/// Declare function: `declare function foo(): void`
///
/// Also used for function overload signatures (no body).
#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct TSDeclareFunction<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
    pub id: Identifier<'src>,
    /// Always false for function declarations (only true for function expressions).
    pub expression: bool,
    /// Whether this is a generator function.
    pub generator: bool,
    /// Whether this is an async function.
    #[serde(rename = "async")]
    pub is_async: bool,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration<'src>>,
    pub params: Vec<Expression<'src>>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation<'src>>,
}

/// Enum declaration: `enum Foo { A, B }`, `const enum Foo { A = 1 }`
#[derive(Debug, Clone, Serialize)]
pub struct TSEnumDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Whether this is a const enum (only serialized when true)
    #[serde(rename = "const", skip_serializing_if = "std::ops::Not::not")]
    pub is_const: bool,
    /// Whether this is a declare enum (ambient declaration, only serialized when true)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
    /// Enum name
    pub id: Identifier<'src>,
    /// Enum members
    pub members: Vec<TSEnumMember<'src>>,
}

/// Enum member: `A`, `A = 1`, `A = "value"`
#[derive(Debug, Clone, Serialize)]
pub struct TSEnumMember<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Member name (identifier or string literal)
    pub id: TSEnumMemberId<'src>,
    /// Optional initializer expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer: Option<Expression<'src>>,
}

/// Enum member id - can be identifier or string literal
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSEnumMemberId<'src> {
    Identifier(Identifier<'src>),
    Literal(Literal<'src>),
}

/// TypeScript module/namespace declaration: `namespace Utils { ... }` or `module Utils { ... }`
#[derive(Debug, Clone, Serialize)]
pub struct TSModuleDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// For `declare global {}` - uses module kind but has special semantics
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub global: bool,
    /// Module/namespace name - identifier for regular namespaces, string literal for ambient modules
    pub id: TSModuleName<'src>,
    /// Module body - either a block or nested module declaration (for `A.B.C`)
    /// `None` for shorthand ambient modules: `declare module 'name';`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<TSModuleDeclarationBody<'src>>,
    /// Whether this is an ambient declaration (`declare namespace/module`)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
}

/// Module/namespace name - can be an identifier or a string literal
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSModuleName<'src> {
    /// Regular identifier: `namespace Foo { }`
    Identifier(Identifier<'src>),
    /// String literal for ambient modules: `declare module 'name' { }`
    Literal(Literal<'src>),
}

/// Body of a TypeScript module declaration
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSModuleDeclarationBody<'src> {
    /// Block body with statements: `namespace A { ... }`
    TSModuleBlock(TSModuleBlock<'src>),
    /// Nested module declaration: `namespace A.B { ... }` - the B part
    TSModuleDeclaration(Box<TSModuleDeclaration<'src>>),
}

/// TypeScript module block: the `{ ... }` part of a namespace/module declaration
#[derive(Debug, Clone, Serialize)]
pub struct TSModuleBlock<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Statements inside the module block
    pub body: Vec<Statement<'src>>,
}
