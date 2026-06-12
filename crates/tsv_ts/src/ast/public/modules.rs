//! Import/Export declarations for public AST

use serde::{Deserialize, Serialize};

use super::declarations::TSDeclareFunction;
use super::types::TSEntityName;
use super::{Expression, Identifier, Literal, SourceLocation, Statement};

/// Export named declaration: `export const x = 1;`, `export { x }`, `export { x } from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportNamedDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<String>,
    /// Declaration being exported (for `export const x = 1`), or null for specifiers
    pub declaration: Option<Box<Statement>>,
    /// Export specifiers: `export { a, b as c }`
    pub specifiers: Vec<ExportSpecifier>,
    /// Re-export source: `export { x } from "y"` or null for local exports
    pub source: Option<Literal>,
    /// Import attributes: present in Svelte non-lang="ts" context; omitted in TypeScript context when empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<ImportAttribute>>,
}

/// Export default declaration: `export default x`, `export default function() {}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportDefaultDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Omitted in Svelte (non-lang="ts") context; always present in TypeScript context
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<String>,
    /// The expression or declaration being exported as default
    pub declaration: ExportDefaultValue,
}

/// Value of export default - can be expression or declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExportDefaultValue {
    Expression(Expression),
    FunctionDeclaration(super::statements::FunctionDeclaration),
    /// For ambient function declarations (no body)
    TSDeclareFunction(TSDeclareFunction),
    ClassDeclaration(super::classes::ClassDeclaration),
}

/// Export all declaration: `export * from "y"` or `export * as ns from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportAllDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<String>,
    /// For `export * as ns from "y"`, the namespace binding name, or null
    pub exported: Option<Identifier>,
    /// Module source
    pub source: Literal,
    /// Import attributes: present in Svelte non-lang="ts" context; omitted in TypeScript context when empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<ImportAttribute>>,
}

/// TypeScript export assignment: `export = value;`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSExportAssignment {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Expression,
}

/// Export specifier: `export { x }` or `export { x as y }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSpecifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Local name (what's exported from this module)
    pub local: Identifier,
    /// Exported name (what it's called externally)
    pub exported: Identifier,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "exportKind", skip_serializing_if = "Option::is_none")]
    pub export_kind: Option<String>,
}

/// Import declaration: `import x from "y"`, `import { a, b } from "y"`, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "importKind", skip_serializing_if = "Option::is_none")]
    pub import_kind: Option<String>,
    pub specifiers: Vec<ImportSpecifier>,
    pub source: Literal,
    /// Present in Svelte non-lang="ts" context (even when empty); omitted in TypeScript context when empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<ImportAttribute>>,
}

/// Import specifier: default, named, or namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImportSpecifier {
    Default(ImportDefaultSpecifier),
    Named(ImportNamedSpecifier),
    Namespace(ImportNamespaceSpecifier),
}

/// Default import: `import x from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDefaultSpecifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub local: Identifier,
}

/// Named import: `import { a } from "y"` or `import { a as b } from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportNamedSpecifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub imported: Identifier,
    pub local: Identifier,
    /// Omitted in Svelte (non-lang="ts") context when "value"; always present in TypeScript context
    #[serde(rename = "importKind", skip_serializing_if = "Option::is_none")]
    pub import_kind: Option<String>,
}

/// Namespace import: `import * as ns from "y"`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportNamespaceSpecifier {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub local: Identifier,
}

/// Import attribute: `{ type: "json" }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportAttribute {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub key: Identifier,
    pub value: Literal,
}

/// TypeScript import equals declaration: `import x = require("y")` or `import x = A.B`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSImportEqualsDeclaration {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    #[serde(rename = "importKind")]
    pub import_kind: String,
    #[serde(rename = "isExport")]
    pub is_export: bool,
    pub id: Identifier,
    #[serde(rename = "moduleReference")]
    pub module_reference: TSModuleReference,
}

/// Module reference: either external module reference or entity name
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TSModuleReference {
    ExternalModuleReference(TSExternalModuleReference),
    EntityName(TSEntityName),
}

/// External module reference: `require("module")`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSExternalModuleReference {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub expression: Literal,
}
