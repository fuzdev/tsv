//! TypeScript declaration types for public AST

use serde::Serialize;

use super::types::{
    TSEntityName, TSInterfaceBody, TSTypeAnnotation, TSTypeParameterDeclaration,
    TSTypeParameterInstantiation,
};
use super::{Expression, Identifier, Literal, SourceLocation, Statement};

/// TypeScript interface declaration: `interface Foo { ... }`
#[derive(Debug, Clone, Serialize)]
pub struct TSInterfaceDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    #[serde(rename = "extends", skip_serializing_if = "Vec::is_empty")]
    pub extends: Vec<TSInterfaceHeritage>,
    pub body: TSInterfaceBody,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
}

/// Interface heritage: `extends Foo, Bar`
#[derive(Debug, Clone, Serialize)]
pub struct TSInterfaceHeritage {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: TSEntityName,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterInstantiation>,
}

/// Declare function: `declare function foo(): void`
///
/// Also used for function overload signatures (no body).
#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct TSDeclareFunction {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
    pub id: Identifier,
    /// Always false for function declarations (only true for function expressions).
    pub expression: bool,
    /// Whether this is a generator function.
    pub generator: bool,
    /// Whether this is an async function.
    #[serde(rename = "async")]
    pub is_async: bool,
    #[serde(rename = "typeParameters", skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<TSTypeParameterDeclaration>,
    pub params: Vec<Expression>,
    #[serde(rename = "returnType", skip_serializing_if = "Option::is_none")]
    pub return_type: Option<TSTypeAnnotation>,
}

/// Enum declaration: `enum Foo { A, B }`, `const enum Foo { A = 1 }`
#[derive(Debug, Clone, Serialize)]
pub struct TSEnumDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
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
    pub id: Identifier,
    /// Enum members
    pub members: Vec<TSEnumMember>,
}

/// Enum member: `A`, `A = 1`, `A = "value"`
#[derive(Debug, Clone, Serialize)]
pub struct TSEnumMember {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Member name (identifier or string literal)
    pub id: TSEnumMemberId,
    /// Optional initializer expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initializer: Option<Expression>,
}

/// Enum member id - can be identifier or string literal
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSEnumMemberId {
    Identifier(Identifier),
    Literal(Literal),
}

/// TypeScript module/namespace declaration: `namespace Utils { ... }` or `module Utils { ... }`
#[derive(Debug, Clone, Serialize)]
pub struct TSModuleDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// For `declare global {}` - uses module kind but has special semantics
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub global: bool,
    /// Module/namespace name - identifier for regular namespaces, string literal for ambient modules
    pub id: TSModuleName,
    /// Module body - either a block or nested module declaration (for `A.B.C`)
    /// `None` for shorthand ambient modules: `declare module 'name';`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<TSModuleDeclarationBody>,
    /// Whether this is an ambient declaration (`declare namespace/module`)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub declare: bool,
}

/// Module/namespace name - can be an identifier or a string literal
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSModuleName {
    /// Regular identifier: `namespace Foo { }`
    Identifier(Identifier),
    /// String literal for ambient modules: `declare module 'name' { }`
    Literal(Literal),
}

/// Body of a TypeScript module declaration
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSModuleDeclarationBody {
    /// Block body with statements: `namespace A { ... }`
    TSModuleBlock(TSModuleBlock),
    /// Nested module declaration: `namespace A.B { ... }` - the B part
    TSModuleDeclaration(Box<TSModuleDeclaration>),
}

/// TypeScript module block: the `{ ... }` part of a namespace/module declaration
#[derive(Debug, Clone, Serialize)]
pub struct TSModuleBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Statements inside the module block
    pub body: Vec<Statement>,
}
