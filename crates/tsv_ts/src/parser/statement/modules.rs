// Import and export declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

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
            // export default ...
            TokenKind::Keyword(KeywordKind::Default) => {
                self.parse_export_default_declaration(start as u32)
            }
            // export * from "y" or export * as ns from "y"
            TokenKind::Star => self.parse_export_all_declaration(start as u32),
            // export { x, y as z } or export { x } from "y"
            TokenKind::BraceOpen => self.parse_export_named_specifiers(start as u32),
            // export const/let/var/function/class
            TokenKind::Keyword(KeywordKind::Const | KeywordKind::Let | KeywordKind::Var) => {
                let declaration = self.parse_variable_declaration()?;
                let end = declaration.span().end;
                Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
                    declaration: Some(Box::new(declaration)),
                    specifiers: Vec::new(),
                    source: None,
                    span: Span::new(start as u32, end),
                }))
            }
            TokenKind::Keyword(KeywordKind::Function) => {
                let declaration = self.parse_function_declaration()?;
                let end = declaration.span().end;
                Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
                    declaration: Some(Box::new(declaration)),
                    specifiers: Vec::new(),
                    source: None,
                    span: Span::new(start as u32, end),
                }))
            }
            TokenKind::Keyword(KeywordKind::Class) => {
                let declaration = self.parse_class_declaration()?;
                let end = declaration.span().end;
                Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
                    declaration: Some(Box::new(declaration)),
                    specifiers: Vec::new(),
                    source: None,
                    span: Span::new(start as u32, end),
                }))
            }
            // TODO: export type { T } - TypeScript type exports
            _ => Err(ParseError::InvalidSyntax {
                message: "Expected declaration, '{', '*', or 'default' after 'export'".to_string(),
                position: self.current_pos().0,
                context: None,
            }),
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
                    return Err(ParseError::InvalidSyntax {
                        message: "Expected 'function' after 'async' in export default".to_string(),
                        position: self.current_pos().0,
                        context: None,
                    });
                }

                let mut func = self.parse_function_declaration_inner(false, true)?;
                // Update span to include 'async' keyword
                func.span = Span::new(async_start, func.span.end);
                let end = func.span.end;
                (ExportDefaultValue::FunctionDeclaration(Box::new(func)), end)
            }
            TokenKind::Keyword(KeywordKind::Function) => {
                // Name is optional for export default function() {}
                let func = self.parse_function_declaration_inner(false, false)?;
                let end = func.span.end;
                (ExportDefaultValue::FunctionDeclaration(Box::new(func)), end)
            }
            TokenKind::Keyword(KeywordKind::Class) => {
                // Name is optional for export default class {}
                let class = self.parse_class_declaration_inner(false)?;
                let end = class.span.end;
                (ExportDefaultValue::ClassDeclaration(Box::new(class)), end)
            }
            _ => {
                // Expression
                let expr = self.parse_expression()?;
                let end = expr.span().end;
                self.semicolon()?;
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
    fn parse_export_all_declaration(&mut self, start: u32) -> Result<Statement, ParseError> {
        // Consume '*'
        debug_assert!(matches!(self.current_kind(), TokenKind::Star));
        self.advance()?;

        // Check for `as ns`
        let exported = if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::As)) {
            self.advance()?; // consume 'as'

            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected identifier after 'as' in export".to_string(),
                    position: self.current_pos().0,
                    context: None,
                });
            }
            let (id_start, id_end) = self.current_pos();
            let name = self.intern(self.current_value());
            self.advance()?;

            Some(Identifier {
                name,
                optional: false,
                type_annotation: None,
                span: Span::new(id_start as u32, id_end as u32),
            })
        } else {
            None
        };

        // Expect 'from'
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::From)) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected 'from' in export all declaration".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }
        self.advance()?;

        // Parse source string
        let source = self.parse_string_literal()?;
        let end = source.span.end;
        self.semicolon()?;

        Ok(Statement::ExportAllDeclaration(ExportAllDeclaration {
            exported,
            source,
            span: Span::new(start, end),
        }))
    }

    /// Parse export named specifiers:
    /// - `export { x, y as z }`
    /// - `export { x } from "y"`
    fn parse_export_named_specifiers(&mut self, start: u32) -> Result<Statement, ParseError> {
        // Consume '{'
        debug_assert!(matches!(self.current_kind(), TokenKind::BraceOpen));
        self.advance()?;

        let mut specifiers = Vec::new();

        // Parse specifiers until '}'
        while !matches!(self.current_kind(), TokenKind::BraceClose) {
            let (spec_start, _) = self.current_pos();

            // Parse local name (the name being exported)
            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected identifier in export specifier".to_string(),
                    position: self.current_pos().0,
                    context: None,
                });
            }
            let (local_start, local_end) = self.current_pos();
            let local_name = self.intern(self.current_value());
            self.advance()?;

            let local = Identifier {
                name: local_name,
                optional: false,
                type_annotation: None,
                span: Span::new(local_start as u32, local_end as u32),
            };

            // Check for 'as exported_name'
            let (exported, spec_end) =
                if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::As)) {
                    self.advance()?; // consume 'as'

                    if !matches!(self.current_kind(), TokenKind::Identifier) {
                        return Err(ParseError::InvalidSyntax {
                            message: "Expected identifier after 'as' in export".to_string(),
                            position: self.current_pos().0,
                            context: None,
                        });
                    }
                    let (exp_start, exp_end) = self.current_pos();
                    let exported_name = self.intern(self.current_value());
                    self.advance()?;

                    (
                        Identifier {
                            name: exported_name,
                            optional: false,
                            type_annotation: None,
                            span: Span::new(exp_start as u32, exp_end as u32),
                        },
                        exp_end as u32,
                    )
                } else {
                    (local.clone(), local_end as u32)
                };

            specifiers.push(ExportSpecifier {
                local,
                exported,
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
            return Err(ParseError::InvalidSyntax {
                message: "Expected '}' to close export specifiers".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }
        let (_, brace_end) = self.current_pos();
        self.advance()?;

        // Check for 'from "source"'
        let (source, end) = if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::From))
        {
            self.advance()?;
            let source = self.parse_string_literal()?;
            let end = source.span.end;
            (Some(source), end)
        } else {
            (None, brace_end as u32)
        };

        self.semicolon()?;

        Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
            declaration: None,
            specifiers,
            source,
            span: Span::new(start, end),
        }))
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
            let (attributes, attr_end) = self.parse_import_attributes()?;
            let end = attr_end.unwrap_or(source.span.end);
            self.semicolon()?;

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
            // vs `import type from "y"` (importing a default export named "type")
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
        if matches!(self.current_kind(), TokenKind::Identifier) {
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern(self.current_value());
            self.advance()?;

            specifiers.push(ImportSpecifier::Default(ImportDefaultSpecifier {
                local: Identifier {
                    name: symbol,
                    optional: false,
                    type_annotation: None,
                    span: Span::new(id_start as u32, id_end as u32),
                },
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
                return Err(ParseError::InvalidSyntax {
                    message: "Expected 'as' after '*' in namespace import".to_string(),
                    position: self.current_pos().0,
                    context: None,
                });
            }
            self.advance()?;

            // Parse local name
            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected identifier after 'as' in namespace import".to_string(),
                    position: self.current_pos().0,
                    context: None,
                });
            }
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern(self.current_value());
            self.advance()?;

            specifiers.push(ImportSpecifier::Namespace(ImportNamespaceSpecifier {
                local: Identifier {
                    name: symbol,
                    optional: false,
                    type_annotation: None,
                    span: Span::new(id_start as u32, id_end as u32),
                },
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
                    // Look ahead to see if next is identifier (inline type) or 'as'/',' (regular import named "type")
                    let next_kind = self.peek_kind();
                    if matches!(next_kind, TokenKind::Identifier) {
                        self.advance()?; // consume 'type'
                        ImportKind::Type
                    } else {
                        ImportKind::Value
                    }
                } else {
                    ImportKind::Value
                };

                // Parse imported name
                if !matches!(self.current_kind(), TokenKind::Identifier) {
                    return Err(ParseError::InvalidSyntax {
                        message: "Expected identifier in import specifier".to_string(),
                        position: self.current_pos().0,
                        context: None,
                    });
                }
                let (imp_start, imp_end) = self.current_pos();
                let imported_symbol = self.intern(self.current_value());
                self.advance()?;

                let imported = Identifier {
                    name: imported_symbol,
                    optional: false,
                    type_annotation: None,
                    span: Span::new(imp_start as u32, imp_end as u32),
                };

                // Check for 'as' rename
                let (local, spec_end) =
                    if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::As)) {
                        self.advance()?;

                        if !matches!(self.current_kind(), TokenKind::Identifier) {
                            return Err(ParseError::InvalidSyntax {
                                message: "Expected identifier after 'as' in import specifier"
                                    .to_string(),
                                position: self.current_pos().0,
                                context: None,
                            });
                        }
                        let (local_start, local_end) = self.current_pos();
                        let local_symbol = self.intern(self.current_value());
                        self.advance()?;

                        (
                            Identifier {
                                name: local_symbol,
                                optional: false,
                                type_annotation: None,
                                span: Span::new(local_start as u32, local_end as u32),
                            },
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
            return Err(ParseError::InvalidSyntax {
                message: "Expected 'from' after import specifiers".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }
        self.advance()?;

        // Parse module source
        if !matches!(self.current_kind(), TokenKind::String) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected string literal as module source".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }
        let source = self.parse_string_literal()?;

        // Parse import attributes: `with { type: "json" }`
        let (attributes, attr_end) = self.parse_import_attributes()?;

        let end = attr_end.unwrap_or(source.span.end);
        self.semicolon()?;

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
            return Err(ParseError::InvalidSyntax {
                message: "Expected '{' after 'with' in import attributes".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }
        self.advance()?;

        let mut attributes = Vec::new();

        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let (attr_start, _) = self.current_pos();

            // Parse attribute key (identifier)
            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected identifier as import attribute key".to_string(),
                    position: self.current_pos().0,
                    context: None,
                });
            }
            let (key_start, key_end) = self.current_pos();
            let key_symbol = self.intern(self.current_value());
            self.advance()?;

            let key = Identifier {
                name: key_symbol,
                optional: false,
                type_annotation: None,
                span: Span::new(key_start as u32, key_end as u32),
            };

            // Expect colon
            if !matches!(self.current_kind(), TokenKind::Colon) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected ':' after import attribute key".to_string(),
                    position: self.current_pos().0,
                    context: None,
                });
            }
            self.advance()?;

            // Parse attribute value (string literal)
            if !matches!(self.current_kind(), TokenKind::String) {
                return Err(ParseError::InvalidSyntax {
                    message: "Expected string literal as import attribute value".to_string(),
                    position: self.current_pos().0,
                    context: None,
                });
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
}
