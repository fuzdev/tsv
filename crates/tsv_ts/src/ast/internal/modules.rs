//! ES Module import/export declarations
//!
//! Contains all import and export declaration types, specifiers,
//! and TypeScript module reference types.

use tsv_lang::Span;

use super::{
    ClassDeclaration, Expression, FunctionDeclaration, Identifier, Literal, Statement,
    TSDeclareFunction, TSEntityName, TSInterfaceDeclaration,
};

/// Export named declaration: `export const x = 1;`, `export { x }`, `export { x } from "y"`
#[derive(Debug, Clone)]
pub struct ExportNamedDeclaration<'arena> {
    /// The declaration being exported (VariableDeclaration, FunctionDeclaration, ClassDeclaration)
    /// None when using specifiers
    pub declaration: Option<&'arena Statement<'arena>>,
    /// Export specifiers: `export { a, b as c }`
    pub specifiers: &'arena [ExportSpecifier<'arena>],
    /// Re-export source: `export { x } from "y"` or None for local exports
    pub source: Option<Literal<'arena>>,
    /// Import attributes: `export { x } from "y" with { type: "json" }`.
    /// `None` = no `with` clause; `Some([])` = empty `with {}` (preserved,
    /// matching acorn/prettier). Only a re-export (with `source`) can carry a
    /// clause (spec: `WithClause` attaches to `export ExportFromClause FromClause`).
    pub attributes: Option<&'arena [ImportAttribute<'arena>]>,
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
pub struct ExportDefaultDeclaration<'arena> {
    /// The expression or declaration being exported as default
    pub declaration: ExportDefaultValue<'arena>,
    pub span: Span,
}

/// Function declaration that may be ambient (TSDeclareFunction) or regular (FunctionDeclaration)
///
/// Used when parsing export function declarations which can be either
/// in ambient context (declare module) or regular context.
#[derive(Debug, Clone)]
pub enum ExportFunctionDeclaration<'arena> {
    Declaration(FunctionDeclaration<'arena>),
    Declare(TSDeclareFunction<'arena>),
}

/// Value of export default - can be expression or declaration
#[derive(Debug, Clone)]
pub enum ExportDefaultValue<'arena> {
    // Variants held inline by value: the `Expression` variant already sizes this
    // enum (it is the largest), so inlining the declaration variants is free, avoids
    // a pointer-chase on the format read path, and matches `ExportFunctionDeclaration`
    // (which holds the same structs inline).
    Expression(Expression<'arena>),
    FunctionDeclaration(FunctionDeclaration<'arena>),
    /// A bodiless function declaration: a `declare function`, an overload
    /// signature, or a bodiless signature inside a `declare namespace`.
    TSDeclareFunction(TSDeclareFunction<'arena>),
    ClassDeclaration(ClassDeclaration<'arena>),
    /// `export default interface Foo {}` (TypeScript)
    TSInterfaceDeclaration(TSInterfaceDeclaration<'arena>),
}

/// Export all declaration: `export * from "y"` or `export * as ns from "y"`
/// Also handles type-only: `export type * from "y"`
#[derive(Debug, Clone)]
pub struct ExportAllDeclaration<'arena> {
    /// For `export * as ns from "y"`, the namespace binding name. Per ecma262
    /// `ModuleExportName : IdentifierName | StringLiteral`, so `export * as 'str' from`
    /// is also valid.
    pub exported: Option<ModuleExportName<'arena>>,
    /// Module source
    pub source: Literal<'arena>,
    /// Import attributes: `export * from "y" with { type: "json" }`.
    /// `None` = no `with` clause; `Some([])` = empty `with {}`.
    pub attributes: Option<&'arena [ImportAttribute<'arena>]>,
    /// Export kind: "value" or "type" (for `export type * from`)
    pub export_kind: ExportKind,
    pub span: Span,
}

/// TypeScript export assignment: `export = value;`
/// CommonJS-style export for TypeScript modules
#[derive(Debug, Clone)]
pub struct TSExportAssignment<'arena> {
    pub expression: Expression<'arena>,
    pub span: Span,
}

/// TypeScript UMD namespace export: `export as namespace Foo;`
/// Declares a global (UMD) name a module is also available under; appears in
/// ambient `.d.ts` files.
#[derive(Debug, Clone)]
pub struct TSNamespaceExportDeclaration<'arena> {
    /// The global namespace name
    pub id: Identifier<'arena>,
    pub span: Span,
}

/// Export specifier: `export { x }` or `export { x as y }` or `export { type x }`
#[derive(Debug, Clone)]
pub struct ExportSpecifier<'arena> {
    /// Local name (what's exported from this module). A string in a re-export,
    /// e.g. `export { 'str' } from 'y'`.
    pub local: ModuleExportName<'arena>,
    /// Exported name (what it's called externally, may be same as local)
    pub exported: ModuleExportName<'arena>,
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

/// Import phase for the import-phase proposals (source-phase imports and
/// import defer). `None` is an ordinary import; `Source`/`Defer` tag the static
/// `import source …` / `import defer …` declaration or the dynamic
/// `import.source(…)` / `import.defer(…)` call. Neither proposal is in acorn yet,
/// so parsing them is a deliberate divergence from the Svelte/acorn oracle — see
/// `docs/conformance_svelte.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportPhase {
    #[default]
    None,
    Source,
    Defer,
}

impl ImportPhase {
    /// The public-AST `phase` string (`"source"`/`"defer"`), or `None` for an
    /// ordinary import (the field is omitted from the JSON in that case).
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            ImportPhase::None => None,
            ImportPhase::Source => Some("source"),
            ImportPhase::Defer => Some("defer"),
        }
    }
}

/// Import declaration: `import x from "y"`, `import { a, b } from "y"`, etc.
#[derive(Debug, Clone)]
pub struct ImportDeclaration<'arena> {
    /// Import specifiers (default, named, or namespace)
    pub specifiers: &'arena [ImportSpecifier<'arena>],
    /// Module source (string literal)
    pub source: Literal<'arena>,
    /// Import attributes: `import x from "y" with { type: "json" }`.
    /// `None` = no `with` clause; `Some([])` = empty `with {}`.
    pub attributes: Option<&'arena [ImportAttribute<'arena>]>,
    /// Import kind: "value" or "type" (for `import type { ... }`)
    pub import_kind: ImportKind,
    /// Import phase: `Source`/`Defer` for `import source …` / `import defer …`.
    pub phase: ImportPhase,
    pub span: Span,
}

/// Import specifier variants
#[derive(Debug, Clone)]
pub enum ImportSpecifier<'arena> {
    /// Default import: `import x from "y"`
    Default(ImportDefaultSpecifier<'arena>),
    /// Named import: `import { a, b as c } from "y"`
    Named(ImportNamedSpecifier<'arena>),
    /// Namespace import: `import * as ns from "y"`
    Namespace(ImportNamespaceSpecifier<'arena>),
}

/// Default import specifier: `import x from "y"`
#[derive(Debug, Clone)]
pub struct ImportDefaultSpecifier<'arena> {
    /// Local binding name
    pub local: Identifier<'arena>,
    pub span: Span,
}

/// Named import specifier: `import { a } from "y"` or `import { a as b } from "y"`
#[derive(Debug, Clone)]
pub struct ImportNamedSpecifier<'arena> {
    /// Imported name (the name in the module). A string for arbitrary module
    /// namespace names, e.g. `import { 'str' as b } from 'y'`.
    pub imported: ModuleExportName<'arena>,
    /// Local binding name (may be same as imported, or different for `as` renames).
    /// Always an identifier — a string imported name requires an `as` binding.
    pub local: Identifier<'arena>,
    /// Import kind for inline type modifier: `import { type A, B } from "y"`
    pub import_kind: ImportKind,
    pub span: Span,
}

/// Namespace import specifier: `import * as ns from "y"`
#[derive(Debug, Clone)]
pub struct ImportNamespaceSpecifier<'arena> {
    /// Local binding name
    pub local: Identifier<'arena>,
    pub span: Span,
}

/// Import attribute: `{ type: "json" }` or `{ "resolution-mode": "import" }`
#[derive(Debug, Clone)]
pub struct ImportAttribute<'arena> {
    pub key: ImportAttributeKey<'arena>,
    pub value: Literal<'arena>,
    pub span: Span,
}

/// Import attribute key: a bare identifier (`type`) or a string literal
/// (`"resolution-mode"`). Per ecma262 `AttributeKey : IdentifierName | StringLiteral`.
#[derive(Debug, Clone)]
pub enum ImportAttributeKey<'arena> {
    Identifier(Identifier<'arena>),
    Literal(Literal<'arena>),
}

impl<'arena> ImportAttributeKey<'arena> {
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
pub enum ModuleExportName<'arena> {
    Identifier(Identifier<'arena>),
    Literal(Literal<'arena>),
}

impl<'arena> ModuleExportName<'arena> {
    pub fn span(&self) -> Span {
        match self {
            ModuleExportName::Identifier(id) => id.span,
            ModuleExportName::Literal(lit) => lit.span,
        }
    }
}

/// TypeScript import equals declaration: `import x = require("y")` or `import x = A.B`
#[derive(Debug, Clone)]
pub struct TSImportEqualsDeclaration<'arena> {
    /// The local binding name
    pub id: Identifier<'arena>,
    /// The module reference (either external module or entity name)
    pub module_reference: TSModuleReference<'arena>,
    /// Import kind: "value" or "type"
    pub import_kind: ImportKind,
    /// Whether this is an export: `export import x = require("y")`
    pub is_export: bool,
    pub span: Span,
}

/// Module reference: either external module reference or entity name
#[derive(Debug, Clone)]
pub enum TSModuleReference<'arena> {
    /// `require("module")`
    ExternalModuleReference(TSExternalModuleReference<'arena>),
    /// `A.B.C` (entity name)
    EntityName(TSEntityName<'arena>),
}

/// External module reference: `require("module")`
#[derive(Debug, Clone)]
pub struct TSExternalModuleReference<'arena> {
    /// The module specifier (string literal)
    pub expression: Literal<'arena>,
    pub span: Span,
}
