// Import and export declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use string_interner::DefaultSymbol;
use tsv_lang::{ParseError, Span};

use super::super::Parser;

/// Wrap a declaration statement in an `ExportNamedDeclaration` with no
/// specifiers or source (`export <declaration>`).
fn export_named(start: usize, declaration: Statement, export_kind: ExportKind) -> Statement {
    let end = declaration.span().end;
    Statement::ExportNamedDeclaration(ExportNamedDeclaration {
        declaration: Some(Box::new(declaration)),
        specifiers: Vec::new(),
        source: None,
        export_kind,
        span: Span::new(start as u32, end),
    })
}

impl<'a> Parser<'a> {
    pub(super) fn parse_export_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'export' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Export)
        ));
        self.advance()?;

        match self.current_kind() {
            // export = expr; (TypeScript CommonJS-style export)
            TokenKind::Equals => {
                self.advance()?; // consume '='
                let expression = self.parse_expression()?;
                let end = self.semicolon_end()?;
                Ok(Statement::TSExportAssignment(TSExportAssignment {
                    expression,
                    span: Span::new(start as u32, end),
                }))
            }
            // export default ...
            TokenKind::Keyword(KeywordKind::Default) => {
                self.parse_export_default_declaration(start as u32)
            }
            // export * from "y" or export * as ns from "y"
            TokenKind::Star => self.parse_export_all_declaration(start as u32, ExportKind::Value),
            // export { x, y as z } or export { x } from "y"
            TokenKind::BraceOpen => self.parse_export_specifiers(start as u32, ExportKind::Value),
            // export const/let/var
            TokenKind::Keyword(KeywordKind::Let | KeywordKind::Var) => Ok(export_named(
                start,
                self.parse_variable_declaration()?,
                ExportKind::Value,
            )),
            // export const ... or export const enum ...
            TokenKind::Keyword(KeywordKind::Const) => {
                // Check for `export const enum` declaration
                let declaration = if self.peek_kind() == TokenKind::Keyword(KeywordKind::Enum) {
                    self.parse_enum_declaration(true, false)?
                } else {
                    self.parse_variable_declaration()?
                };
                Ok(export_named(start, declaration, ExportKind::Value))
            }
            // export enum ...
            TokenKind::Keyword(KeywordKind::Enum) => Ok(export_named(
                start,
                self.parse_enum_declaration(false, false)?,
                ExportKind::Value,
            )),
            TokenKind::Keyword(KeywordKind::Function) => Ok(export_named(
                start,
                self.parse_function_declaration()?,
                ExportKind::Value,
            )),
            // export async function foo() {}
            TokenKind::Keyword(KeywordKind::Async) => Ok(export_named(
                start,
                self.parse_async_function_declaration()?,
                ExportKind::Value,
            )),
            TokenKind::Keyword(KeywordKind::Class) => Ok(export_named(
                start,
                self.parse_class_declaration()?,
                ExportKind::Value,
            )),
            // export type X = T or export interface X { } or export declare function/class
            TokenKind::Identifier => {
                let value = self.current_value().to_string();
                match value.as_str() {
                    "type" => {
                        // Could be:
                        // - export type { Name } from "..." - type-only re-export
                        // - export type * from "..." - type-only re-export all
                        // - export type * as ns from "..." - type-only namespace re-export
                        // - export type X = T - type alias declaration
                        let type_start = self.current_pos().0;
                        self.advance()?; // consume 'type'

                        if matches!(self.current_kind(), TokenKind::BraceOpen) {
                            // export type { Name } from "..." - type-only re-export
                            self.parse_export_specifiers(start as u32, ExportKind::Type)
                        } else if matches!(self.current_kind(), TokenKind::Star) {
                            // export type * from "..." or export type * as ns from "..."
                            self.parse_export_all_declaration(start as u32, ExportKind::Type)
                        } else {
                            // export type X = T - type alias declaration
                            Ok(export_named(
                                start,
                                self.parse_type_alias_declaration_inner(type_start)?,
                                ExportKind::Type,
                            ))
                        }
                    }
                    // export interface X { }
                    "interface" => Ok(export_named(
                        start,
                        self.parse_interface_declaration()?,
                        ExportKind::Type,
                    )),
                    // export declare function/class — ambient declarations are type-level
                    "declare" => Ok(export_named(
                        start,
                        self.parse_declare_statement()?,
                        ExportKind::Type,
                    )),
                    // export abstract class Foo {}
                    "abstract" => Ok(export_named(
                        start,
                        self.parse_abstract_class()?,
                        ExportKind::Value,
                    )),
                    // export namespace/module
                    "namespace" | "module" => Ok(export_named(
                        start,
                        self.parse_module_declaration(false, false)?,
                        ExportKind::Value,
                    )),
                    _ => {
                        Err(self
                            .error_expected_after("declaration, '{', '*', or 'default'", "export"))
                    }
                }
            }
            _ => Err(self.error_expected_after("declaration, '{', '*', or 'default'", "export")),
        }
    }

    /// Parse export default declaration:
    /// - `export default x`
    /// - `export default function() {}`
    /// - `export default function foo() {}`
    /// - `export default class {}`
    /// - `export default class Foo {}`
    fn parse_export_default_declaration(&mut self, start: u32) -> Result<Statement, ParseError> {
        // Consume 'default' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Default)
        ));
        self.advance()?;

        let (declaration, end) = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Async) => {
                // export default async function() {}
                let async_start = self.current_pos().0 as u32;
                self.advance()?; // consume 'async'

                if !matches!(
                    self.current_kind(),
                    TokenKind::Keyword(KeywordKind::Function)
                ) {
                    return Err(self.error_expected_after("'function'", "async"));
                }

                let result = self.parse_function_declaration_or_declare(false, true)?;
                match result {
                    ExportFunctionDeclaration::Declaration(mut func) => {
                        // Update span to include 'async' keyword
                        func.span = Span::new(async_start, func.span.end);
                        let end = func.span.end;
                        (ExportDefaultValue::FunctionDeclaration(Box::new(func)), end)
                    }
                    ExportFunctionDeclaration::Declare(mut func) => {
                        func.span = Span::new(async_start, func.span.end);
                        let end = func.span.end;
                        (ExportDefaultValue::TSDeclareFunction(Box::new(func)), end)
                    }
                }
            }
            TokenKind::Keyword(KeywordKind::Function) => {
                // Name is optional for export default function() {}
                let result = self.parse_function_declaration_or_declare(false, false)?;
                match result {
                    ExportFunctionDeclaration::Declaration(func) => {
                        let end = func.span.end;
                        (ExportDefaultValue::FunctionDeclaration(Box::new(func)), end)
                    }
                    ExportFunctionDeclaration::Declare(func) => {
                        let end = func.span.end;
                        (ExportDefaultValue::TSDeclareFunction(Box::new(func)), end)
                    }
                }
            }
            TokenKind::Keyword(KeywordKind::Class) => {
                // Name is optional for export default class {}
                let class = self.parse_class_declaration_inner(false, false)?;
                let end = class.span.end;
                (ExportDefaultValue::ClassDeclaration(Box::new(class)), end)
            }
            TokenKind::Identifier if self.current_value() == "abstract" => {
                // export default abstract class {}
                let abstract_start = self.current_pos().0 as u32;
                self.advance()?; // consume 'abstract'

                if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Class)) {
                    return Err(self.error_expected_after("'class'", "abstract"));
                }

                let mut class = self.parse_class_declaration_inner(false, true)?;
                // Update span to include 'abstract' keyword
                class.span = Span::new(abstract_start, class.span.end);
                let end = class.span.end;
                (ExportDefaultValue::ClassDeclaration(Box::new(class)), end)
            }
            _ => {
                // Expression
                let expr = self.parse_expression()?;
                let end = self.semicolon_end()?;
                return Ok(Statement::ExportDefaultDeclaration(
                    ExportDefaultDeclaration {
                        declaration: ExportDefaultValue::Expression(expr),
                        span: Span::new(start, end),
                    },
                ));
            }
        };

        Ok(Statement::ExportDefaultDeclaration(
            ExportDefaultDeclaration {
                declaration,
                span: Span::new(start, end),
            },
        ))
    }

    /// Parse export all declaration:
    /// - `export * from "y"`
    /// - `export * as ns from "y"`
    /// - `export type * from "y"`
    /// - `export type * as ns from "y"`
    fn parse_export_all_declaration(
        &mut self,
        start: u32,
        export_kind: ExportKind,
    ) -> Result<Statement, ParseError> {
        // Consume '*'
        debug_assert!(matches!(self.current_kind(), TokenKind::Star));
        self.advance()?;

        // Check for `as ns`
        let exported = if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::As)) {
            self.advance()?; // consume 'as'

            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(self.error_expected_after("identifier", "as"));
            }
            let (id_start, id_end) = self.current_pos();
            let name = self.intern_identifier();
            self.advance()?;

            Some(Identifier::simple(
                name,
                Span::new(id_start as u32, id_end as u32),
            ))
        } else {
            None
        };

        // Expect 'from'
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::From)) {
            return Err(self.error_expected("'from' in export all declaration"));
        }
        self.advance()?;

        // Parse source string
        let source = self.parse_string_literal()?;
        let end = self.semicolon_end()?;

        Ok(Statement::ExportAllDeclaration(ExportAllDeclaration {
            exported,
            source,
            export_kind,
            span: Span::new(start, end),
        }))
    }

    /// Parse export specifiers `{ x, y as z }` with optional `from "source"`:
    /// - `export { x, y as z }` / `export { x } from "y"` (`export_kind: Value`)
    /// - `export { type x, y }` (inline type modifier, value exports only)
    /// - `export type { Name } from "..."` (`export_kind: Type`; specifiers
    ///   stay value — the type-ness lives on the declaration)
    fn parse_export_specifiers(
        &mut self,
        start: u32,
        export_kind: ExportKind,
    ) -> Result<Statement, ParseError> {
        // Consume '{'
        debug_assert!(matches!(self.current_kind(), TokenKind::BraceOpen));
        self.advance()?;

        let mut specifiers = Vec::new();

        // Parse specifiers until '}'
        while !matches!(self.current_kind(), TokenKind::BraceClose) {
            let (spec_start, _) = self.current_pos();

            // Check for inline type modifier: `export { type A, B }`.
            // Not recognized inside `export type { ... }` (TS rejects doubled
            // type modifiers), so `type A` there errors at `A` below.
            let specifier_export_kind = if matches!(export_kind, ExportKind::Value)
                && matches!(self.current_kind(), TokenKind::Identifier)
                && self.current_value() == "type"
            {
                // Look ahead to see if next is identifier or keyword-as-identifier
                // (inline type) or 'as'/',' (regular export named "type")
                let next_kind = self.peek_kind();
                let next_is_identifier = matches!(next_kind, TokenKind::Identifier)
                    || matches!(next_kind, TokenKind::Keyword(kw) if kw.can_be_identifier());
                if next_is_identifier {
                    self.advance()?; // consume 'type'
                    ExportKind::Type
                } else {
                    ExportKind::Value
                }
            } else {
                ExportKind::Value
            };

            let (local, exported, spec_end) = self.parse_export_specifier_names()?;

            specifiers.push(ExportSpecifier {
                local,
                exported,
                export_kind: specifier_export_kind,
                span: Span::new(spec_start as u32, spec_end),
            });

            // Check for comma
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance()?;
            } else {
                break;
            }
        }

        // Expect '}'
        if !matches!(self.current_kind(), TokenKind::BraceClose) {
            return Err(self.error_expected("'}' to close export specifiers"));
        }
        self.advance()?;

        // Check for 'from "source"'
        let source = if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::From)) {
            self.advance()?;
            Some(self.parse_string_literal()?)
        } else {
            None
        };

        let end = self.semicolon_end()?;

        Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
            declaration: None,
            specifiers,
            source,
            export_kind,
            span: Span::new(start, end),
        }))
    }

    /// Parse an export specifier: `local`, `local as exported`, or `default`.
    ///
    /// Returns (local, exported, spec_end_pos).
    /// Accepts contextual keywords as local names and any keyword as exported names.
    fn parse_export_specifier_names(
        &mut self,
    ) -> Result<(Identifier, Identifier, u32), ParseError> {
        // Parse local name: identifier, contextual keyword, or 'default'
        let (local_start, local_end) = self.current_pos();
        let local_name = if matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Default)
        ) {
            self.intern(KeywordKind::Default.as_str())
        } else {
            match self.try_intern_identifier_or_keyword() {
                Some(sym) => sym,
                None => {
                    return Err(self.error_expected("identifier in export specifier"));
                }
            }
        };
        self.advance()?;

        let local = Identifier::simple(local_name, Span::new(local_start as u32, local_end as u32));

        // Check for 'as exported_name'
        // ES spec: exported name is a ModuleExportName (any IdentifierName or string)
        let (exported, spec_end) =
            if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::As)) {
                self.advance()?; // consume 'as'

                let (exp_start, exp_end) = self.current_pos();
                let Some(exported_name) = self.try_intern_identifier_name() else {
                    return Err(self.error_expected_after("identifier", "as"));
                };
                self.advance()?;

                (
                    Identifier::simple(exported_name, Span::new(exp_start as u32, exp_end as u32)),
                    exp_end as u32,
                )
            } else {
                (local.clone(), local_end as u32)
            };

        Ok((local, exported, spec_end))
    }

    /// Parse import declaration:
    /// - `import x from "y"` (default)
    /// - `import { a, b } from "y"` (named)
    /// - `import * as ns from "y"` (namespace)
    /// - `import "y"` (side-effect)
    /// - `import x, { a, b } from "y"` (default + named)
    /// - `import type { a } from "y"` (type-only import)
    /// - `import { type a, b } from "y"` (inline type modifier)
    /// - `import x from "y" with { type: "json" }` (import attributes)
    pub(super) fn parse_import_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'import' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Import)
        ));
        self.advance()?;

        let mut specifiers = Vec::new();

        // Check for side-effect import: `import "y"`
        if matches!(self.current_kind(), TokenKind::String) {
            let source = self.parse_string_literal()?;
            // Check for import attributes after source
            let (attributes, _attr_end) = self.parse_import_attributes()?;
            let end = self.semicolon_end()?;

            return Ok(Statement::ImportDeclaration(ImportDeclaration {
                specifiers: Vec::new(),
                source,
                attributes,
                import_kind: ImportKind::Value,
                span: Span::new(start as u32, end),
            }));
        }

        // Check for `import type` (type-only import)
        let import_kind = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "type"
        {
            // Look ahead to see if this is `import type { ... }` or `import type X from ...`
            // vs `import type from "y"` (importing a default export named "type").
            // Skip comments so `import type /* c */ {}` isn't misread as a default
            // import named `type` (the comment is collected for the printer).
            let next_kind = self.peek_kind();
            if matches!(
                next_kind,
                TokenKind::BraceOpen | TokenKind::Star | TokenKind::Identifier
            ) && !matches!(next_kind, TokenKind::Keyword(KeywordKind::From))
            {
                self.advance()?; // consume 'type'
                ImportKind::Type
            } else {
                ImportKind::Value
            }
        } else {
            ImportKind::Value
        };

        // Parse default import: `import x from "y"` or `import type X from "y"`
        // Also check for `import x = require("y")` or `import x = A.B`
        if matches!(self.current_kind(), TokenKind::Identifier) {
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern_identifier();
            self.advance()?;

            // Check for `import x = ...` (TSImportEqualsDeclaration)
            if matches!(self.current_kind(), TokenKind::Equals) {
                return self.parse_import_equals_declaration(
                    start,
                    id_start,
                    id_end,
                    symbol,
                    import_kind,
                    false, // is_export
                );
            }

            specifiers.push(ImportSpecifier::Default(ImportDefaultSpecifier {
                local: Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32)),
                span: Span::new(id_start as u32, id_end as u32),
            }));

            // Check for comma (default + named/namespace)
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance()?;
            }
        }

        // Parse namespace import: `import * as ns from "y"`
        if matches!(self.current_kind(), TokenKind::Star) {
            let ns_start = self.current_pos().0;
            self.advance()?;

            // Expect 'as' keyword
            if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::As)) {
                return Err(self.error_expected_after("'as'", "*"));
            }
            self.advance()?;

            // Parse local name
            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(self.error_expected_after("identifier", "as"));
            }
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern_identifier();
            self.advance()?;

            specifiers.push(ImportSpecifier::Namespace(ImportNamespaceSpecifier {
                local: Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32)),
                span: Span::new(ns_start as u32, id_end as u32),
            }));
        }

        // Parse named imports: `import { a, b as c } from "y"`
        if matches!(self.current_kind(), TokenKind::BraceOpen) {
            self.advance()?;

            while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
                let (spec_start, _) = self.current_pos();

                // Check for inline type modifier: `import { type A, B } from "y"`
                let specifier_import_kind = if matches!(self.current_kind(), TokenKind::Identifier)
                    && self.current_value() == "type"
                {
                    // Look ahead to see if next is identifier or keyword-as-identifier
                    // (inline type) or 'as'/',' (regular import named "type")
                    let next_kind = self.peek_kind();
                    let next_is_identifier = matches!(next_kind, TokenKind::Identifier)
                        || matches!(next_kind, TokenKind::Keyword(kw) if kw.can_be_identifier());
                    if next_is_identifier {
                        self.advance()?; // consume 'type'
                        ImportKind::Type
                    } else {
                        ImportKind::Value
                    }
                } else {
                    ImportKind::Value
                };

                // Parse imported name (keywords can be specifier names: `import { object }`)
                let (imp_start, imp_end) = self.current_pos();
                let Some(imported_symbol) = self.try_intern_identifier_or_keyword() else {
                    return Err(self.error_expected("identifier in import specifier"));
                };
                self.advance()?;

                let imported = Identifier::simple(
                    imported_symbol,
                    Span::new(imp_start as u32, imp_end as u32),
                );

                // Check for 'as' rename
                let (local, spec_end) =
                    if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::As)) {
                        self.advance()?;

                        let (local_start, local_end) = self.current_pos();
                        let Some(local_symbol) = self.try_intern_binding_name() else {
                            return Err(self.error_expected_after("identifier", "as"));
                        };
                        self.advance()?;

                        (
                            Identifier::simple(
                                local_symbol,
                                Span::new(local_start as u32, local_end as u32),
                            ),
                            local_end,
                        )
                    } else {
                        // local is same as imported
                        (imported.clone(), imp_end)
                    };

                specifiers.push(ImportSpecifier::Named(ImportNamedSpecifier {
                    imported,
                    local,
                    import_kind: specifier_import_kind,
                    span: Span::new(spec_start as u32, spec_end as u32),
                }));

                // Comma separator
                if matches!(self.current_kind(), TokenKind::Comma) {
                    self.advance()?;
                } else {
                    break;
                }
            }

            self.expect(&TokenKind::BraceClose)?;
        }

        // Expect 'from' keyword
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::From)) {
            return Err(self.error_expected_after("'from'", "import specifiers"));
        }
        self.advance()?;

        // Parse module source
        if !matches!(self.current_kind(), TokenKind::String) {
            return Err(self.error_expected("string literal as module source"));
        }
        let source = self.parse_string_literal()?;

        // Parse import attributes: `with { type: "json" }`
        let (attributes, _attr_end) = self.parse_import_attributes()?;

        let end = self.semicolon_end()?;

        Ok(Statement::ImportDeclaration(ImportDeclaration {
            specifiers,
            source,
            attributes,
            import_kind,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse import attributes: `with { type: "json" }`
    /// Returns (attributes, end_position) where end_position is Some if attributes were parsed
    fn parse_import_attributes(
        &mut self,
    ) -> Result<(Vec<ImportAttribute>, Option<u32>), ParseError> {
        // Check for 'with' keyword (contextual - it's an identifier, not a keyword)
        if !matches!(self.current_kind(), TokenKind::Identifier) || self.current_value() != "with" {
            return Ok((Vec::new(), None));
        }
        self.advance()?; // consume 'with'

        // Expect opening brace
        if !matches!(self.current_kind(), TokenKind::BraceOpen) {
            return Err(self.error_expected_after("'{'", "with"));
        }
        self.advance()?;

        let mut attributes = Vec::new();

        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let (attr_start, _) = self.current_pos();

            // Parse attribute key (identifier)
            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(self.error_expected("identifier as import attribute key"));
            }
            let (key_start, key_end) = self.current_pos();
            let key_symbol = self.intern_identifier();
            self.advance()?;

            let key = Identifier::simple(key_symbol, Span::new(key_start as u32, key_end as u32));

            // Expect colon
            if !matches!(self.current_kind(), TokenKind::Colon) {
                return Err(self.error_expected_after("':'", "import attribute key"));
            }
            self.advance()?;

            // Parse attribute value (string literal)
            if !matches!(self.current_kind(), TokenKind::String) {
                return Err(self.error_expected("string literal as import attribute value"));
            }
            let value = self.parse_string_literal()?;
            let attr_end = value.span.end;

            attributes.push(ImportAttribute {
                key,
                value,
                span: Span::new(attr_start as u32, attr_end),
            });

            // Comma separator
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance()?;
            } else {
                break;
            }
        }

        let (_, brace_end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok((attributes, Some(brace_end as u32)))
    }

    /// Parse `import x = require("y")` or `import x = A.B`
    fn parse_import_equals_declaration(
        &mut self,
        start: usize,
        id_start: usize,
        id_end: usize,
        symbol: DefaultSymbol,
        import_kind: ImportKind,
        is_export: bool,
    ) -> Result<Statement, ParseError> {
        // Already have: import <identifier>
        // Current token is `=`
        self.advance()?; // consume `=`

        let id = Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32));

        let module_reference = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "require"
            && matches!(self.peek_kind(), TokenKind::ParenOpen)
        {
            // `require("module")`
            let ref_start = self.current_pos().0;
            self.advance()?; // consume `require`
            self.advance()?; // consume `(`

            // Parse string literal
            if !matches!(self.current_kind(), TokenKind::String) {
                return Err(self.error_expected("string literal in require()"));
            }
            let expression = self.parse_string_literal()?;

            // Handle optional trailing comma before closing paren
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance()?;
            }

            let (_, ref_end) = self.current_pos();
            self.expect(&TokenKind::ParenClose)?;

            TSModuleReference::ExternalModuleReference(TSExternalModuleReference {
                expression,
                span: Span::new(ref_start as u32, ref_end as u32),
            })
        } else {
            // `A.B.C` (entity name)
            TSModuleReference::EntityName(self.parse_entity_name()?)
        };

        let end = self.semicolon_end()?;

        Ok(Statement::TSImportEqualsDeclaration(
            TSImportEqualsDeclaration {
                id,
                module_reference,
                import_kind,
                is_export,
                span: Span::new(start as u32, end),
            },
        ))
    }
}
