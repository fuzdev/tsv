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
                KeywordKind::Const => {
                    // Check for `const enum` declaration
                    if self.peek_kind() == TokenKind::Keyword(KeywordKind::Enum) {
                        self.parse_enum_declaration(true, false)
                    } else {
                        self.parse_variable_declaration()
                    }
                }
                KeywordKind::Let | KeywordKind::Var => self.parse_variable_declaration(),
                KeywordKind::Return => self.parse_return_statement(),
                KeywordKind::Function => {
                    // In ambient context (declare namespace), parse as TSDeclareFunction
                    if self.in_ambient_context {
                        self.parse_ambient_function_declaration()
                    } else {
                        self.parse_function_declaration()
                    }
                }
                KeywordKind::Class => self.parse_class_declaration(),
                KeywordKind::Enum => self.parse_enum_declaration(false, false),
                KeywordKind::Export => self.parse_export_declaration(),
                KeywordKind::Import => {
                    // `import(...)` is a dynamic import expression
                    // `import ...` is an import declaration
                    if self.peek_kind() == TokenKind::ParenOpen {
                        // Dynamic import expression
                        let expr = self.parse_expression()?;
                        let start = expr.span().start;
                        self.semicolon()?;
                        let end = self.prev_token_end() as u32;
                        Ok(Statement::ExpressionStatement(ExpressionStatement {
                            expression: expr,
                            span: Span::new(start, end),
                        }))
                    } else {
                        self.parse_import_declaration()
                    }
                }
                KeywordKind::Async => {
                    // `async function` is a function declaration
                    // `async () => ...` or `async x => ...` is an expression
                    if self.peek_kind() == TokenKind::Keyword(KeywordKind::Function) {
                        self.parse_async_function_declaration()
                    } else {
                        // Async arrow function expression
                        let expr = self.parse_expression()?;
                        let start = expr.span().start;
                        self.semicolon()?;
                        let end = self.prev_token_end() as u32;
                        Ok(Statement::ExpressionStatement(ExpressionStatement {
                            expression: expr,
                            span: Span::new(start, end),
                        }))
                    }
                }
                KeywordKind::Await => {
                    // Check for `await using` declaration (ES2024 Explicit Resource Management)
                    if self.peek_is_identifier() && self.peek_value() == "using" {
                        return self.parse_await_using_declaration();
                    }
                    // Regular await expression
                    let expr = self.parse_expression()?;
                    let start = expr.span().start;
                    self.semicolon()?;
                    let end = self.prev_token_end() as u32;
                    Ok(Statement::ExpressionStatement(ExpressionStatement {
                        expression: expr,
                        span: Span::new(start, end),
                    }))
                }
                KeywordKind::True
                | KeywordKind::False
                | KeywordKind::Null
                | KeywordKind::Undefined
                | KeywordKind::New
                | KeywordKind::Typeof
                | KeywordKind::Void
                | KeywordKind::Delete
                | KeywordKind::Yield
                | KeywordKind::Super => {
                    // These are literals or expression-starting keywords, parse as expression statement
                    let expr = self.parse_expression()?;
                    let start = expr.span().start;
                    self.semicolon()?;
                    let end = self.prev_token_end() as u32;
                    Ok(Statement::ExpressionStatement(ExpressionStatement {
                        expression: expr,
                        span: Span::new(start, end),
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
                | KeywordKind::From => Err(self.error_unexpected_keyword(*kw)),
                // Binary operator keywords are not valid at statement level
                KeywordKind::Instanceof | KeywordKind::In | KeywordKind::Extends => {
                    Err(self.error_unexpected_keyword(*kw))
                }
                // Contextual keywords that can be used as identifiers in expression statements
                // E.g., `as = 'updated';` where `as` is a variable name
                KeywordKind::As
                | KeywordKind::Satisfies
                | KeywordKind::Number
                | KeywordKind::String
                | KeywordKind::Boolean
                | KeywordKind::Any
                | KeywordKind::Never
                | KeywordKind::Unknown
                | KeywordKind::Object
                | KeywordKind::Symbol
                | KeywordKind::Bigint => {
                    // These keywords can be identifiers, so parse as expression statement
                    let expr = self.parse_expression()?;
                    let start = expr.span().start;
                    self.semicolon()?;
                    let end = self.prev_token_end() as u32;
                    Ok(Statement::ExpressionStatement(ExpressionStatement {
                        expression: expr,
                        span: Span::new(start, end),
                    }))
                }
            },
            TokenKind::Identifier => {
                // Check for contextual keyword 'using' followed by identifier (ES2024 Explicit Resource Management)
                if self.current_value() == "using" && self.peek_is_identifier() {
                    return self.parse_using_declaration();
                }
                // Check for contextual keyword 'type' followed by identifier (type alias declaration)
                if self.current_value() == "type" && self.peek_is_identifier() {
                    return self.parse_type_alias_declaration();
                }
                // Check for contextual keyword 'interface' followed by identifier
                if self.current_value() == "interface" && self.peek_is_identifier() {
                    return self.parse_interface_declaration();
                }
                // Check for contextual keyword 'declare' followed by function/class
                if self.current_value() == "declare" {
                    return self.parse_declare_statement();
                }
                // Check for contextual keyword 'abstract' followed by class
                if self.current_value() == "abstract"
                    && self.peek_kind() == TokenKind::Keyword(KeywordKind::Class)
                {
                    return self.parse_abstract_class();
                }
                // Check for contextual keyword 'namespace' or 'module' followed by identifier
                if (self.current_value() == "namespace" || self.current_value() == "module")
                    && self.peek_is_identifier()
                {
                    return self.parse_module_declaration(false, false);
                }
                // Check for labeled statement: `label: statement`
                if self.peek_kind() == TokenKind::Colon {
                    return self.parse_labeled_statement();
                }
                // Regular expression statement
                let expr = self.parse_expression()?;
                let start = expr.span().start;
                self.semicolon()?;
                let end = self.prev_token_end() as u32;
                Ok(Statement::ExpressionStatement(ExpressionStatement {
                    expression: expr,
                    span: Span::new(start, end),
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
            TokenKind::At => {
                // Decorator: `@expression class Foo { }`
                self.parse_decorated_class()
            }
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                let start = expr.span().start;
                self.semicolon()?;
                let end = self.prev_token_end() as u32;
                Ok(Statement::ExpressionStatement(ExpressionStatement {
                    expression: expr,
                    span: Span::new(start, end),
                }))
            }
        }
    }
}
