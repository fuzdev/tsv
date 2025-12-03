// Function declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        let (start, return_end) = self.current_pos();

        // Consume 'return' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Return)
        ));
        self.advance()?;

        // ASI Rule: If semicolon present or can insert semicolon, return has no argument.
        // This handles: `return\n1+2` → `return;` (then `1+2;` as separate statement)
        // The `1+2` becomes an unreachable expression statement.
        if self.eat(TokenKind::Semicolon) || self.can_insert_semicolon() {
            return Ok(Statement::ReturnStatement(ReturnStatement {
                argument: None,
                span: Span::new(start as u32, return_end as u32),
            }));
        }

        // No ASI - parse the return value expression
        let argument = self.parse_expression()?;
        let arg_end = argument.span().end;
        self.semicolon()?;

        Ok(Statement::ReturnStatement(ReturnStatement {
            argument: Some(argument),
            span: Span::new(start as u32, arg_end),
        }))
    }

    pub(super) fn parse_function_declaration(&mut self) -> Result<Statement, ParseError> {
        let func = self.parse_function_declaration_inner(true, false)?;
        Ok(Statement::FunctionDeclaration(func))
    }

    /// Parse async function declaration: `async function foo() {}`
    pub(super) fn parse_async_function_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'async' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Async)
        ));
        self.advance()?;

        // Parse the function (name required for declarations)
        let mut func = self.parse_function_declaration_inner(true, true)?;
        // Update span to include 'async' keyword
        func.span = Span::new(start as u32, func.span.end);

        Ok(Statement::FunctionDeclaration(func))
    }

    /// Inner function that returns the FunctionDeclaration directly
    /// Used by parse_function_declaration, parse_async_function_declaration, and export default
    ///
    /// `name_required`: If true, function name is required. If false, name is optional
    /// (for `export default function() {}`)
    /// `is_async`: If true, this is an async function
    pub(super) fn parse_function_declaration_inner(
        &mut self,
        name_required: bool,
        is_async: bool,
    ) -> Result<FunctionDeclaration, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'function' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Function)
        ));
        self.advance()?;

        // Parse function name (required for declarations, optional for export default)
        let id = if matches!(self.current_kind(), TokenKind::Identifier) {
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern(self.current_value());
            self.advance()?;

            Some(Identifier {
                name: symbol,
                optional: false,
                type_annotation: None,
                span: Span::new(id_start as u32, id_end as u32),
            })
        } else if name_required {
            return Err(ParseError::InvalidSyntax {
                message: "Expected function name after 'function'".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        } else {
            None
        };

        // Parse parameter list
        let params = self.parse_parameter_list()?;

        // Check for return type annotation: (): type
        let return_type = if self.check(&TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_block_statement()?;
        let end = body.span.end;

        Ok(FunctionDeclaration {
            id,
            params,
            return_type,
            body,
            generator: false, // TODO: Support generator functions
            r#async: is_async,
            span: Span::new(start as u32, end),
        })
    }
}
