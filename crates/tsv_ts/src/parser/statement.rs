// Statement parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::Parser;

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
                KeywordKind::True
                | KeywordKind::False
                | KeywordKind::Null
                | KeywordKind::Undefined
                | KeywordKind::New
                | KeywordKind::Typeof
                | KeywordKind::Void
                | KeywordKind::Delete
                | KeywordKind::Await
                | KeywordKind::Async
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
                // Regular expression statement
                let expr = self.parse_expression()?;
                let expr_span = expr.span();
                self.semicolon()?;
                Ok(Statement::ExpressionStatement(ExpressionStatement {
                    expression: expr,
                    span: expr_span,
                }))
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

    fn parse_variable_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Get the keyword kind
        let kind = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Const) => VariableDeclarationKind::Const,
            TokenKind::Keyword(KeywordKind::Let) => VariableDeclarationKind::Let,
            TokenKind::Keyword(KeywordKind::Var) => VariableDeclarationKind::Var,
            _ => unreachable!(),
        };
        self.advance()?;

        // Parse declarators (comma-separated list)
        let mut declarations = vec![self.parse_variable_declarator()?];
        while self.eat(TokenKind::Comma) {
            declarations.push(self.parse_variable_declarator()?);
        }

        // Span ends at last declarator (ASI may not have explicit semicolon)
        // SAFETY: declarations was initialized with at least one element above
        #[allow(clippy::unwrap_used)]
        let decl_end = declarations.last().unwrap().span.end;
        self.semicolon()?;

        Ok(Statement::VariableDeclaration(VariableDeclaration {
            kind,
            declarations,
            span: Span::new(start as u32, decl_end),
        }))
    }

    fn parse_variable_declarator(&mut self) -> Result<VariableDeclarator, ParseError> {
        let id_start = self.current_pos().0;

        // Parse binding pattern: identifier, array pattern [a, b], or object pattern {a, b}
        let id = match self.current_kind() {
            TokenKind::Identifier => {
                // Simple identifier binding
                let (start, end) = self.current_pos();
                let symbol = self.intern(self.current_value());
                self.advance()?;

                // Check for type annotation on identifier
                let type_annotation = if self.check(&TokenKind::Colon) {
                    Some(self.parse_type_annotation()?)
                } else {
                    None
                };

                let id_end = type_annotation
                    .as_ref()
                    .map_or(end, |ta| ta.span.end as usize);

                Expression::Identifier(Identifier {
                    name: symbol,
                    type_annotation,
                    span: Span::new(start as u32, id_end as u32),
                })
            }
            TokenKind::BracketOpen => {
                // Array destructuring pattern: [a, b] = arr
                let expr = self.parse_array_expression()?;
                self.to_assignable(expr)?
            }
            TokenKind::BraceOpen => {
                // Object destructuring pattern: {a, b} = obj
                let expr = self.parse_object_expression()?;
                self.to_assignable(expr)?
            }
            _ => {
                return Err(ParseError::InvalidSyntax {
                    message: format!(
                        "Expected identifier or destructuring pattern, found {}",
                        self.current_kind()
                    ),
                    position: self.current_pos().0,
                    context: None,
                });
            }
        };

        let id_end = id.span().end as usize;

        // Check for initializer
        // Use assignment_expression because comma separates declarators
        let init = if self.eat(TokenKind::Equals) {
            Some(self.parse_assignment_expression()?)
        } else {
            None
        };

        let end = init.as_ref().map_or(id_end, |e| e.span().end as usize);

        Ok(VariableDeclarator {
            id,
            init,
            span: Span::new(id_start as u32, end as u32),
        })
    }

    fn parse_type_annotation(&mut self) -> Result<TSTypeAnnotation, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::Colon)?;

        let type_node = self.parse_type()?;
        let end = type_node.span().end;

        Ok(TSTypeAnnotation {
            type_annotation: Box::new(type_node),
            span: Span::new(start as u32, end),
        })
    }

    fn parse_type(&mut self) -> Result<TSType, ParseError> {
        let (start, end) = self.current_pos();
        let span = Span::new(start as u32, end as u32);

        // Try to convert the current keyword to a type keyword
        if let TokenKind::Keyword(kw) = self.current_kind()
            && let Some(ts_kind) = TSKeywordKind::from_lexer_keyword(kw)
        {
            self.advance()?;
            return Ok(TSType::Keyword(TSKeywordType::new(ts_kind, span)));
        }

        // Check for template literal type: `hello` or `hello ${T} world`
        if matches!(
            self.current_kind(),
            TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead
        ) {
            let template = self.parse_template_literal_type()?;
            return Ok(TSType::Literal(TSLiteralType::TemplateLiteral(template)));
        }

        Err(ParseError::InvalidSyntax {
            message: format!("Expected type, found {}", self.current_kind()),
            position: self.current_pos().0,
            context: None,
        })
    }

    /// Parse a template literal in type context: `hello ${string} world`
    ///
    /// Parallel structure to `parse_template_literal()` in expression.rs but parses
    /// types inside ${} instead of expressions. Kept separate for clarity despite
    /// duplication - the two contexts (expression vs type) rarely change together.
    fn parse_template_literal_type(&mut self) -> Result<TemplateLiteralType, ParseError> {
        let (start, _) = self.current_pos();
        let mut quasis = Vec::new();
        let mut types = Vec::new();

        match self.current_kind() {
            TokenKind::NoSubstitutionTemplate => {
                // Simple template with no interpolation: `hello world`
                let (elem_start, elem_end) = self.current_pos();
                let raw = self.current_value().to_string();

                // Extract content between backticks
                let content = if raw.len() >= 2 {
                    raw[1..raw.len() - 1].to_string()
                } else {
                    String::new()
                };

                // Decode escapes for cooked value
                let cooked = if let Some(decoded) = self.current_decoded() {
                    Some(decoded.to_string())
                } else {
                    Some(content.clone())
                };

                self.advance()?;

                quasis.push(TemplateElement {
                    raw: content,
                    cooked,
                    tail: true,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                Ok(TemplateLiteralType {
                    quasis,
                    types,
                    span: Span::new(start as u32, elem_end as u32),
                })
            }
            TokenKind::TemplateHead => {
                // Template with interpolation: `hello ${string}...`
                let (elem_start, elem_end) = self.current_pos();
                let raw = self.current_value().to_string();

                // Extract content: remove leading ` and trailing ${
                let content = if raw.len() >= 3 {
                    raw[1..raw.len() - 2].to_string()
                } else {
                    String::new()
                };

                let cooked = if let Some(decoded) = self.current_decoded() {
                    Some(decoded.to_string())
                } else {
                    Some(content.clone())
                };

                self.advance()?;

                quasis.push(TemplateElement {
                    raw: content,
                    cooked,
                    tail: false,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                // Parse types and remaining template parts
                loop {
                    // Parse the interpolated type (not expression!)
                    let ts_type = self.parse_type()?;
                    types.push(ts_type);

                    // Expect closing } of the interpolation
                    let (brace_start, _) = self.current_pos();
                    if !self.check(&TokenKind::BraceClose) {
                        return Err(ParseError::InvalidSyntax {
                            message: "Expected '}' after type in template literal".to_string(),
                            position: self.current_pos().0,
                            context: None,
                        });
                    }

                    // Use lexer to continue template from }
                    let token = self
                        .lexer
                        .continue_template_from_brace(self.current_raw_end())?;
                    self.update_current(token);

                    match self.current_kind().clone() {
                        TokenKind::TemplateTail => {
                            // Final part: }content`
                            let (_tail_start, tail_end) = self.current_pos();
                            let tail_raw = self.current_value().to_string();

                            // Extract content: remove leading } and trailing `
                            let tail_content = if tail_raw.len() >= 2 {
                                tail_raw[1..tail_raw.len() - 1].to_string()
                            } else {
                                String::new()
                            };

                            let tail_cooked = if let Some(decoded) = self.current_decoded() {
                                Some(decoded.to_string())
                            } else {
                                Some(tail_content.clone())
                            };

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw: tail_content,
                                cooked: tail_cooked,
                                tail: true,
                                span: Span::new(brace_start as u32, tail_end as u32),
                            });

                            return Ok(TemplateLiteralType {
                                quasis,
                                types,
                                span: Span::new(start as u32, tail_end as u32),
                            });
                        }
                        TokenKind::TemplateMiddle => {
                            // Middle part: }content${
                            let (_mid_start, mid_end) = self.current_pos();
                            let mid_raw = self.current_value().to_string();

                            // Extract content: remove leading } and trailing ${
                            let mid_content = if mid_raw.len() >= 3 {
                                mid_raw[1..mid_raw.len() - 2].to_string()
                            } else {
                                String::new()
                            };

                            let mid_cooked = if let Some(decoded) = self.current_decoded() {
                                Some(decoded.to_string())
                            } else {
                                Some(mid_content.clone())
                            };

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw: mid_content,
                                cooked: mid_cooked,
                                tail: false,
                                span: Span::new(brace_start as u32, mid_end as u32),
                            });
                            // Continue loop for next interpolation
                        }
                        _ => {
                            return Err(ParseError::InvalidSyntax {
                                message: "Unexpected token in template literal type".to_string(),
                                position: self.current_pos().0,
                                context: None,
                            });
                        }
                    }
                }
            }
            _ => Err(ParseError::InvalidSyntax {
                message: format!(
                    "Expected template literal type, found {}",
                    self.current_kind()
                ),
                position: self.current_pos().0,
                context: None,
            }),
        }
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
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

    fn parse_function_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'function' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Function)
        ));
        self.advance()?;

        // Parse function name (required for declarations)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected function name after 'function'".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern(self.current_value());
        self.advance()?;

        let id = Identifier {
            name: symbol,
            type_annotation: None,
            span: Span::new(id_start as u32, id_end as u32),
        };

        // Parse parameter list and block body
        let params = self.parse_parameter_list()?;
        let body = self.parse_block_statement()?;
        let end = body.span.end;

        Ok(Statement::FunctionDeclaration(FunctionDeclaration {
            id,
            params,
            body,
            generator: false, // TODO: Support generator functions
            r#async: false,   // TODO: Support async functions
            span: Span::new(start as u32, end),
        }))
    }

    fn parse_class_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'class' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Class)
        ));
        self.advance()?;

        // Parse class name (required for declarations)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected class name after 'class'".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern(self.current_value());
        self.advance()?;

        let id = Identifier {
            name: symbol,
            type_annotation: None,
            span: Span::new(id_start as u32, id_end as u32),
        };

        // Parse optional `extends` clause
        let super_class = if matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Extends)
        ) {
            self.advance()?; // consume 'extends'

            // Parse superclass expression (typically an identifier, but could be member expression)
            let expr = self.parse_expression()?;
            Some(Box::new(expr))
        } else {
            None
        };

        // Parse class body
        let body = self.parse_class_body()?;
        let end = body.span.end;

        Ok(Statement::ClassDeclaration(ClassDeclaration {
            id,
            super_class,
            body,
            span: Span::new(start as u32, end),
        }))
    }

    fn parse_class_body(&mut self) -> Result<ClassBody, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BraceOpen)?;

        let mut body = Vec::new();

        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let method = self.parse_method_definition()?;
            body.push(method);
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(ClassBody {
            body,
            span: Span::new(start as u32, end as u32),
        })
    }

    fn parse_method_definition(&mut self) -> Result<MethodDefinition, ParseError> {
        let (start, _) = self.current_pos();

        // TODO: Handle 'static' keyword
        let is_static = false;

        // TODO: Handle 'get' and 'set' contextual keywords
        // For now, just parse methods and constructors

        // Parse method name
        let (computed, key, method_name) = if matches!(self.current_kind(), TokenKind::BracketOpen)
        {
            // Computed key: [expr]
            self.advance()?;
            let expr = self.parse_expression()?;
            self.expect(&TokenKind::BracketClose)?;
            (true, expr, None)
        } else if matches!(self.current_kind(), TokenKind::Identifier) {
            // Regular identifier key - capture name before advancing
            let name_str = self.current_value().to_string();
            let (key_start, key_end) = self.current_pos();
            let symbol = self.intern(&name_str);
            self.advance()?;
            (
                false,
                Expression::Identifier(Identifier {
                    name: symbol,
                    type_annotation: None,
                    span: Span::new(key_start as u32, key_end as u32),
                }),
                Some(name_str),
            )
        } else {
            return Err(ParseError::InvalidSyntax {
                message: "Expected method name".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        };

        // Determine method kind based on name
        let kind = match method_name.as_deref() {
            Some("constructor") => MethodKind::Constructor,
            _ => MethodKind::Method,
        };

        // Parse parameter list and block body (like a function)
        let params = self.parse_parameter_list()?;
        let body_block = self.parse_block_statement()?;
        let end = body_block.span.end;

        // Create FunctionExpression for the method value
        let value = FunctionExpression {
            id: None,
            params,
            body: body_block,
            span: Span::new(start as u32, end),
        };

        Ok(MethodDefinition {
            key,
            value,
            kind,
            is_static,
            computed,
            span: Span::new(start as u32, end),
        })
    }

    fn parse_type_alias_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'type' contextual keyword
        debug_assert!(self.current_value() == "type");
        self.advance()?;

        // Parse type name (identifier)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected type name after 'type'".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern(self.current_value());
        self.advance()?;

        let id = Identifier {
            name: symbol,
            type_annotation: None,
            span: Span::new(id_start as u32, id_end as u32),
        };

        // Expect '='
        self.expect(&TokenKind::Equals)?;

        // Parse the type
        let type_annotation = self.parse_type()?;
        let type_end = type_annotation.span().end;
        self.semicolon()?;

        Ok(Statement::TSTypeAliasDeclaration(TSTypeAliasDeclaration {
            id,
            type_annotation,
            span: Span::new(start as u32, type_end),
        }))
    }
}
