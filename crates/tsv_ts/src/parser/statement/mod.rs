// Statement parsing - main dispatcher

use crate::ast::internal::*;
use crate::lexer::KeywordKind;
use crate::lexer::TokenKind;
use tsv_lang::{ParseError, Span};

use super::Parser;

// Sub-modules for different statement categories
mod class;
mod control_flow;
mod function;
mod modules;
mod types;
mod variable;

impl<'a> Parser<'a> {
    pub(super) fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        // Check if this is a variable declaration
        match self.current_kind() {
            TokenKind::Keyword(kw) => match kw {
                KeywordKind::Const | KeywordKind::Let | KeywordKind::Var => {
                    self.parse_variable_declaration()
                }
                KeywordKind::Return => self.parse_return_statement(),
                KeywordKind::Function => self.parse_function_declaration(),
                KeywordKind::Class => self.parse_class_declaration(),
                KeywordKind::Export => self.parse_export_declaration(),
                KeywordKind::Import => self.parse_import_declaration(),
                KeywordKind::Async => {
                    // `async function` is a function declaration
                    // `async () => ...` or `async x => ...` is an expression
                    if self.peek_kind() == TokenKind::Keyword(KeywordKind::Function) {
                        self.parse_async_function_declaration()
                    } else {
                        // Async arrow function expression
                        let expr = self.parse_expression()?;
                        let expr_span = expr.span();
                        self.semicolon()?;
                        Ok(Statement::ExpressionStatement(ExpressionStatement {
                            expression: expr,
                            span: expr_span,
                        }))
                    }
                }
                KeywordKind::True
                | KeywordKind::False
                | KeywordKind::Null
                | KeywordKind::Undefined
                | KeywordKind::New
                | KeywordKind::Typeof
                | KeywordKind::Void
                | KeywordKind::Delete
                | KeywordKind::Await
                | KeywordKind::Super => {
                    // These are literals or expression-starting keywords, parse as expression statement
                    let expr = self.parse_expression()?;
                    let expr_span = expr.span();
                    self.semicolon()?;
                    Ok(Statement::ExpressionStatement(ExpressionStatement {
                        expression: expr,
                        span: expr_span,
                    }))
                }
                // Control flow statements
                KeywordKind::If => self.parse_if_statement(),
                KeywordKind::For => self.parse_for_statement(),
                KeywordKind::While => self.parse_while_statement(),
                KeywordKind::Do => self.parse_do_while_statement(),
                KeywordKind::Switch => self.parse_switch_statement(),
                KeywordKind::Try => self.parse_try_statement(),
                KeywordKind::Throw => self.parse_throw_statement(),
                KeywordKind::Break => self.parse_break_statement(),
                KeywordKind::Continue => self.parse_continue_statement(),
                // Continuation keywords - these appear mid-statement, not at start
                KeywordKind::Else
                | KeywordKind::Case
                | KeywordKind::Default
                | KeywordKind::Catch
                | KeywordKind::Finally
                | KeywordKind::From
                | KeywordKind::As => Err(ParseError::InvalidSyntax {
                    message: format!("Unexpected keyword '{kw}'"),
                    position: self.current_pos().0,
                    context: None,
                }),
                // Type-only keywords and binary operator keywords are not valid at statement level
                KeywordKind::Number
                | KeywordKind::String
                | KeywordKind::Boolean
                | KeywordKind::Any
                | KeywordKind::Never
                | KeywordKind::Unknown
                | KeywordKind::Object
                | KeywordKind::Symbol
                | KeywordKind::Bigint
                | KeywordKind::Instanceof
                | KeywordKind::In
                | KeywordKind::Extends => Err(ParseError::InvalidSyntax {
                    message: format!("Unexpected keyword '{kw}'"),
                    position: self.current_pos().0,
                    context: None,
                }),
            },
            TokenKind::Identifier => {
                // Check for contextual keyword 'type' followed by identifier (type alias declaration)
                if self.current_value() == "type" && self.peek_is_identifier() {
                    return self.parse_type_alias_declaration();
                }
                // Check for labeled statement: `label: statement`
                if self.peek_kind() == TokenKind::Colon {
                    return self.parse_labeled_statement();
                }
                // Regular expression statement
                let expr = self.parse_expression()?;
                let expr_span = expr.span();
                self.semicolon()?;
                Ok(Statement::ExpressionStatement(ExpressionStatement {
                    expression: expr,
                    span: expr_span,
                }))
            }
            TokenKind::Semicolon => {
                // Empty statement: `;`
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(Statement::EmptyStatement(EmptyStatement {
                    span: Span::new(start as u32, end as u32),
                }))
            }
            TokenKind::BraceOpen => {
                // Block statement: `{ ... }`
                let block = self.parse_block_statement()?;
                Ok(Statement::BlockStatement(block))
            }
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                let expr_span = expr.span();
                self.semicolon()?;
                Ok(Statement::ExpressionStatement(ExpressionStatement {
                    expression: expr,
                    span: expr_span,
                }))
            }
        }
    }
}
