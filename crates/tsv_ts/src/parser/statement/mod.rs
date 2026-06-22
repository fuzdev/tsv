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
mod type_declarations;
mod variable;

impl<'a> Parser<'a> {
    pub(crate) fn parse_statement(&mut self) -> Result<Statement, ParseError> {
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
                    // `import.meta` is a meta property expression
                    // `import ...` is an import declaration
                    if self.peek_kind() == TokenKind::ParenOpen
                        || self.peek_kind() == TokenKind::Dot
                    {
                        self.parse_expression_statement()
                    } else {
                        self.parse_import_declaration()
                    }
                }
                KeywordKind::Async => {
                    // `async function` is a function declaration
                    // `async () => ...` or `async x => ...` is an expression
                    // peek_kind() skips comments between `async` and `function`
                    if self.peek_kind() == TokenKind::Keyword(KeywordKind::Function) {
                        self.parse_async_function_declaration()
                    } else {
                        // Async arrow function expression
                        self.parse_expression_statement()
                    }
                }
                KeywordKind::Await => {
                    // Check for `await using` declaration (ES2024 Explicit Resource Management);
                    // both gaps carry [no LineTerminator here] — a break before `using` or
                    // before the binding makes this an `await using` expression statement
                    if self.peek_is_same_line_identifier()
                        && self.peek_value() == "using"
                        && self.peek_followed_by_same_line_identifier()
                    {
                        return self.parse_await_using_declaration();
                    }
                    // Regular await expression
                    self.parse_expression_statement()
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
                | KeywordKind::This
                | KeywordKind::Super => {
                    // These are literals or expression-starting keywords, parse as expression statement
                    self.parse_expression_statement()
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
                KeywordKind::Debugger => self.parse_debugger_statement(),
                // Continuation keywords - these appear mid-statement, not at start
                KeywordKind::Else
                | KeywordKind::Case
                | KeywordKind::Default
                | KeywordKind::Catch
                | KeywordKind::Finally => Err(self.error_unexpected_keyword(*kw)),
                // Binary operator keywords are not valid at statement level
                KeywordKind::Instanceof | KeywordKind::In | KeywordKind::Extends => {
                    Err(self.error_unexpected_keyword(*kw))
                }
                // Contextual keywords that can be used as identifiers in expression statements
                // E.g., `from.shift();` or `as = 'updated';` where the keyword is a variable name
                KeywordKind::From
                | KeywordKind::As
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
                    self.parse_expression_statement()
                }
            },
            TokenKind::Identifier => {
                // Check for contextual keyword 'using' followed by identifier (ES2024
                // Explicit Resource Management); `using [no LineTerminator here]
                // BindingIdentifier` — a break makes `using` an identifier statement
                if self.current_value() == "using" && self.peek_is_same_line_identifier() {
                    return self.parse_using_declaration();
                }
                // Contextual keyword `type` starts a type alias only when the name is
                // on the SAME line (tsc `nextTokenIsIdentifierOnSameLine`). A line
                // break demotes `type` to a plain identifier and ASI splits the
                // statement in two. peek_kind() skips comments: `type /* c */ A = T`.
                if self.current_value() == "type" && self.peek_is_same_line_identifier() {
                    return self.parse_type_alias_declaration();
                }
                // Contextual keyword `interface` starts a declaration only when the
                // name is on the SAME line (tsc `nextTokenIsIdentifierOnSameLine`); a
                // line break demotes it to an identifier. peek_kind() skips comments.
                if self.current_value() == "interface" && self.peek_is_same_line_identifier() {
                    return self.parse_interface_declaration();
                }
                // Contextual keyword `declare` is an ambient-declaration modifier only
                // when a declaration starter follows on the SAME line (tsc
                // `isDeclaration`: `nextToken(); if (hasPrecedingLineBreak()) return
                // false`). Otherwise `declare` is a plain identifier.
                if self.current_value() == "declare" && self.peek_starts_ambient_declaration() {
                    return self.parse_declare_statement();
                }
                // Check for contextual keyword 'abstract' followed by class
                // peek_kind() skips comments: `abstract /* c */ class A {}`.
                // `abstract [no LineTerminator here] class` — a break makes `abstract`
                // an identifier statement and the class a plain declaration (tsc + acorn)
                if self.current_value() == "abstract"
                    && self.peek_kind() == TokenKind::Keyword(KeywordKind::Class)
                    && !self.peek_preceded_by_line_terminator()
                {
                    return self.parse_abstract_class();
                }
                // Contextual keywords `namespace`/`module` start a declaration only
                // when the name is on the SAME line (tsc
                // `nextTokenIsIdentifierOrStringLiteralOnSameLine`); a line break
                // demotes them to identifiers. peek_kind() skips comments.
                if matches!(self.current_value(), "namespace" | "module")
                    && self.peek_is_same_line_identifier()
                {
                    return self.parse_module_declaration(false, false);
                }
                // Check for labeled statement: `label: statement`
                // peek_kind() skips comments: `label /* c */: statement`
                if self.peek_kind() == TokenKind::Colon {
                    return self.parse_labeled_statement();
                }
                // Regular expression statement
                self.parse_expression_statement()
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
            _ => self.parse_expression_statement(),
        }
    }

    /// Parse an expression statement: `<expr>;`
    ///
    /// Captures the start position before parsing so the span includes any
    /// surrounding parens: `('hello');` → span starts at `(`, not `'`.
    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        let start = self.current_pos().0 as u32;
        let expr = self.parse_expression()?;
        let end = self.semicolon_end()?;
        Ok(Statement::ExpressionStatement(ExpressionStatement {
            expression: expr,
            span: Span::new(start, end),
            is_directive: false,
        }))
    }

    /// Mark the directive prologue of a `Program` or function body.
    ///
    /// Mirrors acorn's `adaptDirectivePrologue`: the leading run of
    /// unparenthesized string-literal expression statements are directives
    /// (`"use strict";` and friends). Iteration stops at the first statement
    /// that isn't a directive candidate.
    pub(super) fn adapt_directive_prologue(&self, statements: &mut [Statement]) {
        for stmt in statements {
            let Statement::ExpressionStatement(expr_stmt) = stmt else {
                break;
            };
            let Expression::Literal(lit) = &expr_stmt.expression else {
                break;
            };
            if !matches!(lit.value, LiteralValue::String { .. }) {
                break;
            }
            // Reject parenthesized strings: the statement must open with a quote.
            let local_start = (expr_stmt.span.start as usize).saturating_sub(self.base_offset);
            if !matches!(self.source.as_bytes().get(local_start), Some(b'"' | b'\'')) {
                break;
            }
            expr_stmt.is_directive = true;
        }
    }
}
