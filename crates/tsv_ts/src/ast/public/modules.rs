//! Import/Export declarations for public AST

use serde::Serialize;

use super::declarations::{TSDeclareFunction, TSInterfaceDeclaration};
use super::types::TSEntityName;
use super::{Expression, Identifier, Literal, SourceLocation, Statement};

/// Export named declaration: `export const x = 1;`, `export { x }`, `export { x } from "y"`
#[derive(Debug, Clone, Serialize)]
pub struct ExportNamedDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<&'static str>,
    /// Declaration being exported (for `export const x = 1`), or null for specifiers
    pub declaration: Option<Box<Statement<'src>>>,
    /// Export specifiers: `export { a, b as c }`
    pub specifiers: Vec<ExportSpecifier<'src>>,
    /// Re-export source: `export { x } from "y"` or null for local exports
    pub source: Option<Literal<'src>>,
    /// Import attributes: present in Svelte non-lang="ts" context; omitted in TypeScript context when empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<ImportAttribute<'src>>>,
}

/// Export default declaration: `export default x`, `export default function() {}`
#[derive(Debug, Clone, Serialize)]
pub struct ExportDefaultDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Omitted in Svelte (non-lang="ts") context; always present in TypeScript context
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<&'static str>,
    /// The expression or declaration being exported as default
    pub declaration: ExportDefaultValue<'src>,
}

/// Value of export default - can be expression or declaration
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ExportDefaultValue<'src> {
    Expression(Expression<'src>),
    FunctionDeclaration(super::statements::FunctionDeclaration<'src>),
    /// For ambient function declarations (no body)
    TSDeclareFunction(TSDeclareFunction<'src>),
    ClassDeclaration(super::classes::ClassDeclaration<'src>),
    /// `export default interface Foo {}` (TypeScript)
    TSInterfaceDeclaration(TSInterfaceDeclaration<'src>),
}

/// Export all declaration: `export * from "y"` or `export * as ns from "y"`
#[derive(Debug, Clone, Serialize)]
pub struct ExportAllDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<&'static str>,
    /// For `export * as ns from "y"`, the namespace binding name, or null.
    /// A `Literal` for a string name (`export * as 'str' from "y"`).
    pub exported: Option<ModuleExportName<'src>>,
    /// Module source
    pub source: Literal<'src>,
    /// Import attributes: present in Svelte non-lang="ts" context; omitted in TypeScript context when empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<ImportAttribute<'src>>>,
}

/// TypeScript export assignment: `export = value;`
#[derive(Debug, Clone, Serialize)]
pub struct TSExportAssignment<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression<'src>,
}

/// TypeScript UMD namespace export: `export as namespace Foo;`
#[derive(Debug, Clone, Serialize)]
pub struct TSNamespaceExportDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub id: Identifier<'src>,
}

/// Export specifier: `export { x }` or `export { x as y }`
#[derive(Debug, Clone, Serialize)]
pub struct ExportSpecifier<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Local name (what's exported from this module)
    pub local: ModuleExportName<'src>,
    /// Exported name (what it's called externally)
    pub exported: ModuleExportName<'src>,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<&'static str>,
}

/// Import declaration: `import x from "y"`, `import { a, b } from "y"`, etc.
#[derive(Debug, Clone, Serialize)]
pub struct ImportDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "importKind", skip_serializing_if = "Option::is_none")]
    pub import_kind: Option<&'static str>,
    /// Import phase (`"source"`/`"defer"`); omitted for an ordinary import.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<&'static str>,
    pub specifiers: Vec<ImportSpecifier<'src>>,
    pub source: Literal<'src>,
    /// Present in Svelte non-lang="ts" context (even when empty); omitted in TypeScript context when empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<ImportAttribute<'src>>>,
}

/// Import specifier: default, named, or namespace
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ImportSpecifier<'src> {
    Default(ImportDefaultSpecifier<'src>),
    Named(ImportNamedSpecifier<'src>),
    Namespace(ImportNamespaceSpecifier<'src>),
}

/// Default import: `import x from "y"`
#[derive(Debug, Clone, Serialize)]
pub struct ImportDefaultSpecifier<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub local: Identifier<'src>,
}

/// Named import: `import { a } from "y"` or `import { a as b } from "y"`
#[derive(Debug, Clone, Serialize)]
pub struct ImportNamedSpecifier<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub imported: ModuleExportName<'src>,
    pub local: Identifier<'src>,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "importKind", skip_serializing_if = "Option::is_none")]
    pub import_kind: Option<&'static str>,
}

/// Namespace import: `import * as ns from "y"`
#[derive(Debug, Clone, Serialize)]
pub struct ImportNamespaceSpecifier<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub local: Identifier<'src>,
}

/// Import attribute: `{ type: "json" }` or `{ "resolution-mode": "import" }`
#[derive(Debug, Clone, Serialize)]
pub struct ImportAttribute<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub key: ImportAttributeKey<'src>,
    pub value: Literal<'src>,
}

/// Import attribute key: a bare `Identifier` (`type`) or a `Literal` string
/// (`"resolution-mode"`). Acorn emits whichever the source used; serialized
/// untagged (each variant carries its own `type` discriminator).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ImportAttributeKey<'src> {
    Identifier(Identifier<'src>),
    Literal(Literal<'src>),
}

/// Module export name: a bare `Identifier` or a `Literal` string. Acorn emits
/// whichever the source used; serialized untagged (each variant carries its own
/// `type` discriminator). Per ecma262 `ModuleExportName : IdentifierName | StringLiteral`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ModuleExportName<'src> {
    Identifier(Identifier<'src>),
    Literal(Literal<'src>),
}

/// TypeScript import equals declaration: `import x = require("y")` or `import x = A.B`
#[derive(Debug, Clone, Serialize)]
pub struct TSImportEqualsDeclaration<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "importKind")]
    pub import_kind: &'static str,
    #[serde(rename = "isExport")]
    pub is_export: bool,
    pub id: Identifier<'src>,
    #[serde(rename = "moduleReference")]
    pub module_reference: TSModuleReference<'src>,
}

/// Module reference: either external module reference or entity name
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TSModuleReference<'src> {
    ExternalModuleReference(TSExternalModuleReference<'src>),
    EntityName(TSEntityName<'src>),
}

/// External module reference: `require("module")`
#[derive(Debug, Clone, Serialize)]
pub struct TSExternalModuleReference<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Literal<'src>,
}
