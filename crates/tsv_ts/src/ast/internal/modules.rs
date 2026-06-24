//! ES Module import/export declarations
//!
//! Contains all import and export declaration types, specifiers,
//! and TypeScript module reference types.

use tsv_lang::Span;

use super::{
    ClassDeclaration, Expression, FunctionDeclaration, Identifier, Literal, Statement,
    TSDeclareFunction, TSEntityName,
};

/// Export named declaration: `export const x = 1;`, `export { x }`, `export { x } from "y"`
#[derive(Debug, Clone)]
pub struct ExportNamedDeclaration {
    /// The declaration being exported (VariableDeclaration, FunctionDeclaration, ClassDeclaration)
    /// None when using specifiers
    pub declaration: Option<Box<Statement>>,
    /// Export specifiers: `export { a, b as c }`
    pub specifiers: Vec<ExportSpecifier>,
    /// Re-export source: `export { x } from "y"` or None for local exports
    pub source: Option<Literal>,
    /// Import attributes: `export { x } from "y" with { type: "json" }`.
    /// `None` = no `with` clause; `Some([])` = empty `with {}` (preserved,
    /// matching acorn/prettier). Only a re-export (with `source`) can carry a
    /// clause (spec: `WithClause` attaches to `export ExportFromClause FromClause`).
    pub attributes: Option<Vec<ImportAttribute>>,
    /// Export kind: "value" for regular exports, "type" for type-only exports
    pub export_kind: ExportKind,
    pub span: Span,
}

/// Export kind for TypeScript type-only exports
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportKind {
    /// Regular value export: `export { x }`
    #[default]
    Value,
    /// Type-only export: `export type { X }`
    Type,
}

/// Export default declaration: `export default x`, `export default function() {}`
#[derive(Debug, Clone)]
pub struct ExportDefaultDeclaration {
    /// The expression or declaration being exported as default
    pub declaration: ExportDefaultValue,
    pub span: Span,
}

/// Function declaration that may be ambient (TSDeclareFunction) or regular (FunctionDeclaration)
///
/// Used when parsing export function declarations which can be either
/// in ambient context (declare module) or regular context.
#[derive(Debug, Clone)]
pub enum ExportFunctionDeclaration {
    Declaration(FunctionDeclaration),
    Declare(TSDeclareFunction),
}

/// Value of export default - can be expression or declaration
#[derive(Debug, Clone)]
pub enum ExportDefaultValue {
    Expression(Expression),
    FunctionDeclaration(Box<FunctionDeclaration>),
    /// For ambient function declarations (no body)
    TSDeclareFunction(Box<TSDeclareFunction>),
    ClassDeclaration(Box<ClassDeclaration>),
}

/// Export all declaration: `export * from "y"` or `export * as ns from "y"`
/// Also handles type-only: `export type * from "y"`
#[derive(Debug, Clone)]
pub struct ExportAllDeclaration {
    /// For `export * as ns from "y"`, the namespace binding name. Per ecma262
    /// `ModuleExportName : IdentifierName | StringLiteral`, so `export * as 'str' from`
    /// is also valid.
    pub exported: Option<ModuleExportName>,
    /// Module source
    pub source: Literal,
    /// Import attributes: `export * from "y" with { type: "json" }`.
    /// `None` = no `with` clause; `Some([])` = empty `with {}`.
    pub attributes: Option<Vec<ImportAttribute>>,
    /// Export kind: "value" or "type" (for `export type * from`)
    pub export_kind: ExportKind,
    pub span: Span,
}

/// TypeScript export assignment: `export = value;`
/// CommonJS-style export for TypeScript modules
#[derive(Debug, Clone)]
pub struct TSExportAssignment {
    pub expression: Expression,
    pub span: Span,
}

/// Export specifier: `export { x }` or `export { x as y }` or `export { type x }`
#[derive(Debug, Clone)]
pub struct ExportSpecifier {
    /// Local name (what's exported from this module). A string in a re-export,
    /// e.g. `export { 'str' } from 'y'`.
    pub local: ModuleExportName,
    /// Exported name (what it's called externally, may be same as local)
    pub exported: ModuleExportName,
    /// Export kind for inline type modifier: `export { type A, b }`
    pub export_kind: ExportKind,
    pub span: Span,
}

/// Import kind: value import or type-only import
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportKind {
    #[default]
    Value,
    Type,
}

/// Import declaration: `import x from "y"`, `import { a, b } from "y"`, etc.
#[derive(Debug, Clone)]
pub struct ImportDeclaration {
    /// Import specifiers (default, named, or namespace)
    pub specifiers: Vec<ImportSpecifier>,
    /// Module source (string literal)
    pub source: Literal,
    /// Import attributes: `import x from "y" with { type: "json" }`.
    /// `None` = no `with` clause; `Some([])` = empty `with {}`.
    pub attributes: Option<Vec<ImportAttribute>>,
    /// Import kind: "value" or "type" (for `import type { ... }`)
    pub import_kind: ImportKind,
    pub span: Span,
}

/// Import specifier variants
#[derive(Debug, Clone)]
pub enum ImportSpecifier {
    /// Default import: `import x from "y"`
    Default(ImportDefaultSpecifier),
    /// Named import: `import { a, b as c } from "y"`
    Named(ImportNamedSpecifier),
    /// Namespace import: `import * as ns from "y"`
    Namespace(ImportNamespaceSpecifier),
}

/// Default import specifier: `import x from "y"`
#[derive(Debug, Clone)]
pub struct ImportDefaultSpecifier {
    /// Local binding name
    pub local: Identifier,
    pub span: Span,
}

/// Named import specifier: `import { a } from "y"` or `import { a as b } from "y"`
#[derive(Debug, Clone)]
pub struct ImportNamedSpecifier {
    /// Imported name (the name in the module). A string for arbitrary module
    /// namespace names, e.g. `import { 'str' as b } from 'y'`.
    pub imported: ModuleExportName,
    /// Local binding name (may be same as imported, or different for `as` renames).
    /// Always an identifier — a string imported name requires an `as` binding.
    pub local: Identifier,
    /// Import kind for inline type modifier: `import { type A, B } from "y"`
    pub import_kind: ImportKind,
    pub span: Span,
}

/// Namespace import specifier: `import * as ns from "y"`
#[derive(Debug, Clone)]
pub struct ImportNamespaceSpecifier {
    /// Local binding name
    pub local: Identifier,
    pub span: Span,
}

/// Import attribute: `{ type: "json" }` or `{ "resolution-mode": "import" }`
#[derive(Debug, Clone)]
pub struct ImportAttribute {
    pub key: ImportAttributeKey,
    pub value: Literal,
    pub span: Span,
}

/// Import attribute key: a bare identifier (`type`) or a string literal
/// (`"resolution-mode"`). Per ecma262 `AttributeKey : IdentifierName | StringLiteral`.
#[derive(Debug, Clone)]
pub enum ImportAttributeKey {
    Identifier(Identifier),
    Literal(Literal),
}

impl ImportAttributeKey {
    pub fn span(&self) -> Span {
        match self {
            ImportAttributeKey::Identifier(id) => id.span,
            ImportAttributeKey::Literal(lit) => lit.span,
        }
    }
}

/// Module export name: a bare identifier (`x`) or a string literal (`'str'`).
/// Per ecma262 `ModuleExportName : IdentifierName | StringLiteral` (ES2022
/// arbitrary module namespace names). Used for import/export specifier names
/// and the `export * as` namespace name. Mirrors `ImportAttributeKey`.
#[derive(Debug, Clone)]
pub enum ModuleExportName {
    Identifier(Identifier),
    Literal(Literal),
}

impl ModuleExportName {
    pub fn span(&self) -> Span {
        match self {
            ModuleExportName::Identifier(id) => id.span,
            ModuleExportName::Literal(lit) => lit.span,
        }
    }
}

/// TypeScript import equals declaration: `import x = require("y")` or `import x = A.B`
#[derive(Debug, Clone)]
pub struct TSImportEqualsDeclaration {
    /// The local binding name
    pub id: Identifier,
    /// The module reference (either external module or entity name)
    pub module_reference: TSModuleReference,
    /// Import kind: "value" or "type"
    pub import_kind: ImportKind,
    /// Whether this is an export: `export import x = require("y")`
    pub is_export: bool,
    pub span: Span,
}

/// Module reference: either external module reference or entity name
#[derive(Debug, Clone)]
pub enum TSModuleReference {
    /// `require("module")`
    ExternalModuleReference(TSExternalModuleReference),
    /// `A.B.C` (entity name)
    EntityName(TSEntityName),
}

/// External module reference: `require("module")`
#[derive(Debug, Clone)]
pub struct TSExternalModuleReference {
    /// The module specifier (string literal)
    pub expression: Literal,
    pub span: Span,
}
