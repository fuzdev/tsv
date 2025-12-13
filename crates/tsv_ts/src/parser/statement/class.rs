// Class declaration parsing

use crate::ast::internal::{
    Accessibility, BlockStatement, CallExpression, ClassBody, ClassDeclaration, ClassExpression,
    ClassMember, Decorator, ExportDefaultDeclaration, ExportDefaultValue, ExportKind,
    ExportNamedDeclaration, Expression, FunctionExpression, Identifier, MemberExpression,
    MethodDefinition, MethodKind, PropertyDefinition, PropertyModifier, Statement, StaticBlock,
};
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

impl<'a> Parser<'a> {
    pub(super) fn parse_class_declaration(&mut self) -> Result<Statement, ParseError> {
        let class = self.parse_class_declaration_inner(true, false)?;
        Ok(Statement::ClassDeclaration(class))
    }

    /// Parse an abstract class declaration: `abstract class Foo { ... }`
    pub(super) fn parse_abstract_class(&mut self) -> Result<Statement, ParseError> {
        // Consume 'abstract' contextual keyword
        debug_assert!(self.current_value() == "abstract");
        self.advance()?;

        let class = self.parse_class_declaration_inner(true, true)?;
        Ok(Statement::ClassDeclaration(class))
    }

    /// Parse a decorated class: `@decorator class Foo { }`
    ///
    /// Decorators can be stacked: `@dec1 @dec2 class Foo { }`
    /// Decorator can be followed by `abstract class` or `export class`
    pub(super) fn parse_decorated_class(&mut self) -> Result<Statement, ParseError> {
        let start = self.current_pos().0;

        // Parse one or more decorators
        let decorators = self.parse_decorators()?;

        // Check for `export` before class
        let is_export = self.current_kind() == TokenKind::Keyword(KeywordKind::Export);
        let is_default = if is_export {
            self.advance()?; // consume 'export'
            if self.current_kind() == TokenKind::Keyword(KeywordKind::Default) {
                self.advance()?; // consume 'default'
                true
            } else {
                false
            }
        } else {
            false
        };

        // Check for `abstract` before `class`
        let is_abstract = if self.current_kind() == TokenKind::Identifier
            && self.current_value() == "abstract"
            && self.peek_kind() == TokenKind::Keyword(KeywordKind::Class)
        {
            self.advance()?; // consume 'abstract'
            true
        } else {
            false
        };

        // Expect `class` keyword
        if self.current_kind() != TokenKind::Keyword(KeywordKind::Class) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected 'class' after decorator".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }

        // Parse the class (name optional for export default)
        let mut class = self.parse_class_declaration_inner(!is_default, is_abstract)?;

        // Attach decorators to the class
        class.decorators = decorators;

        // Update span to include decorators
        class.span = Span::new(start as u32, class.span.end);

        // Wrap in export if needed
        if is_export {
            if is_default {
                let end = class.span.end;
                Ok(Statement::ExportDefaultDeclaration(
                    ExportDefaultDeclaration {
                        declaration: ExportDefaultValue::ClassDeclaration(Box::new(class)),
                        span: Span::new(start as u32, end),
                    },
                ))
            } else {
                let end = class.span.end;
                Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
                    declaration: Some(Box::new(Statement::ClassDeclaration(class))),
                    specifiers: Vec::new(),
                    source: None,
                    export_kind: ExportKind::Value,
                    span: Span::new(start as u32, end),
                }))
            }
        } else {
            Ok(Statement::ClassDeclaration(class))
        }
    }

    /// Parse a list of decorators: `@dec1 @dec2 ...`
    fn parse_decorators(&mut self) -> Result<Vec<Decorator>, ParseError> {
        let mut decorators = Vec::new();

        while self.current_kind() == TokenKind::At {
            decorators.push(self.parse_decorator()?);
        }

        Ok(decorators)
    }

    /// Parse a single decorator: `@expression`
    ///
    /// The expression can be:
    /// - Identifier: `@foo`
    /// - Call expression: `@foo()` or `@foo(arg)`
    /// - Member expression: `@foo.bar` or `@foo.bar()`
    fn parse_decorator(&mut self) -> Result<Decorator, ParseError> {
        let start = self.current_pos().0;

        // Consume '@'
        debug_assert!(self.current_kind() == TokenKind::At);
        self.advance()?;

        // Parse the decorator expression (identifier, member, or call)
        // We use parse_assignment_expression which handles identifier.member and identifier()
        let expression = self.parse_assignment_expression()?;

        let end = expression.span().end;

        Ok(Decorator {
            expression,
            span: Span::new(start as u32, end),
        })
    }

    /// Parse a class expression: `class { }` or `class Foo<T> extends Bar { }`
    ///
    /// Class expressions are similar to class declarations but:
    /// - The name is always optional
    /// - They appear in expression position
    /// - No `declare` field
    pub fn parse_class_expression(&mut self) -> Result<Expression, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'class' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Class)
        ));
        self.advance()?;

        // Parse optional class name
        let id = if matches!(self.current_kind(), TokenKind::Identifier) {
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern_identifier();
            self.advance()?;

            Some(Identifier {
                name: symbol,
                optional: false,
                type_annotation: None,
                span: Span::new(id_start as u32, id_end as u32),
            })
        } else {
            None
        };

        // Parse type parameters (TypeScript generics): class Foo<T>
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Parse optional `extends` clause
        let (super_class, super_type_parameters) = if matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Extends)
        ) {
            self.advance()?; // consume 'extends'

            // Parse superclass expression
            let expr = self.parse_heritage_expression()?;

            // Parse optional type arguments: `extends Base<T>`
            let type_args = if self.check(&TokenKind::LessThan) {
                Some(self.parse_type_arguments()?)
            } else {
                None
            };

            (Some(Box::new(expr)), type_args)
        } else {
            (None, None)
        };

        // Parse optional `implements` clause
        let implements = if self.eat_contextual_keyword("implements") {
            self.parse_interface_heritage_list()?
        } else {
            Vec::new()
        };

        // Parse class body
        let body = self.parse_class_body()?;
        let end = body.span.end;

        Ok(Expression::ClassExpression(ClassExpression {
            decorators: Vec::new(),
            id,
            super_class,
            super_type_parameters,
            implements,
            body,
            r#abstract: false,
            type_parameters,
            span: Span::new(start as u32, end),
        }))
    }

    /// Inner function that returns the ClassDeclaration directly
    /// Used by both parse_class_declaration and export default
    ///
    /// `name_required`: If true, class name is required. If false, name is optional
    /// (for `export default class {}`)
    /// `is_abstract`: If true, this is an abstract class
    pub(super) fn parse_class_declaration_inner(
        &mut self,
        name_required: bool,
        is_abstract: bool,
    ) -> Result<ClassDeclaration, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'class' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Class)
        ));
        self.advance()?;

        // Parse class name (required for declarations, optional for export default)
        let id = if matches!(self.current_kind(), TokenKind::Identifier) {
            let (id_start, id_end) = self.current_pos();
            let symbol = self.intern_identifier();
            self.advance()?;

            Some(Identifier {
                name: symbol,
                optional: false,
                type_annotation: None,
                span: Span::new(id_start as u32, id_end as u32),
            })
        } else if name_required {
            return Err(ParseError::InvalidSyntax {
                message: "Expected class name after 'class'".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        } else {
            None
        };

        // Parse type parameters (TypeScript generics): class Foo<T>()
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Parse optional `extends` clause
        let (super_class, super_type_parameters) = if matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Extends)
        ) {
            self.advance()?; // consume 'extends'

            // Parse superclass expression (identifier or member expression like Foo.Bar)
            let expr = self.parse_heritage_expression()?;

            // Parse optional type arguments: `extends Base<T>`
            let type_args = if self.check(&TokenKind::LessThan) {
                Some(self.parse_type_arguments()?)
            } else {
                None
            };

            (Some(Box::new(expr)), type_args)
        } else {
            (None, None)
        };

        // Parse optional `implements` clause
        let implements = if self.eat_contextual_keyword("implements") {
            self.parse_interface_heritage_list()?
        } else {
            Vec::new()
        };

        // Parse class body
        let body = self.parse_class_body()?;
        let end = body.span.end;

        Ok(ClassDeclaration {
            decorators: Vec::new(),
            id,
            super_class,
            super_type_parameters,
            implements,
            body,
            declare: false,
            r#abstract: is_abstract,
            type_parameters,
            span: Span::new(start as u32, end),
        })
    }

    fn parse_class_body(&mut self) -> Result<ClassBody, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BraceOpen)?;

        let mut body = Vec::new();

        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let member = self.parse_class_member()?;
            body.push(member);
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(ClassBody {
            body,
            span: Span::new(start as u32, end as u32),
        })
    }

    fn parse_class_member(&mut self) -> Result<ClassMember, ParseError> {
        let (start, _) = self.current_pos();

        // Parse any decorators on this member
        let decorators = self.parse_decorators()?;

        // Handle accessibility modifiers (public, private, protected)
        let accessibility = if self.eat_contextual_keyword("public") {
            Some(Accessibility::Public)
        } else if self.eat_contextual_keyword("private") {
            Some(Accessibility::Private)
        } else if self.eat_contextual_keyword("protected") {
            Some(Accessibility::Protected)
        } else {
            None
        };

        // Handle 'static' contextual keyword
        let is_static = self.eat_contextual_keyword("static");

        // Check for static initialization block: `static { ... }` (ES2022)
        if is_static && matches!(self.current_kind(), TokenKind::BraceOpen) {
            // Parse the block body
            let block = self.parse_block_statement()?;
            let end = block.span.end;

            return Ok(ClassMember::StaticBlock(StaticBlock {
                body: block.body,
                span: Span::new(start as u32, end),
            }));
        }

        // Handle 'override' contextual keyword - only if followed by a class member name
        let is_override = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "override"
            && self.peek_is_class_member_name()
        {
            self.advance().ok();
            true
        } else {
            false
        };

        // Handle 'abstract' contextual keyword (only if followed by identifier, bracket, or #)
        let is_abstract = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "abstract"
            && self.peek_is_class_member_name()
        {
            self.advance().ok();
            true
        } else {
            false
        };

        // Handle 'readonly' contextual keyword
        let readonly = self.eat_contextual_keyword("readonly");

        // Handle 'accessor' contextual keyword (ES decorator proposal)
        let accessor = self.eat_contextual_keyword("accessor");

        // Handle 'async' keyword for async methods
        // async is only a modifier if followed by: identifier, [, #, or *
        let is_async = if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Async))
            && (self.peek_is_class_member_name() || self.peek_is(&TokenKind::Star))
        {
            self.advance().ok();
            true
        } else {
            false
        };

        // Handle '*' for generator methods
        let is_generator = self.eat(TokenKind::Star);

        // Handle 'get' and 'set' contextual keywords for getters/setters
        let accessor_kind = if matches!(self.current_kind(), TokenKind::Identifier) {
            let kind = match self.current_value() {
                "get" => Some(MethodKind::Get),
                "set" => Some(MethodKind::Set),
                _ => None,
            };
            // Peek ahead to see if next token is a class member name (identifier, bracket, or #)
            // vs `(` or `=` (property named 'get'/'set')
            if kind.is_some() && self.peek_is_class_member_name() {
                self.advance().ok();
                kind
            } else {
                None
            }
        } else {
            None
        };

        // Parse member name (key)
        let (computed, key, method_name) = if matches!(self.current_kind(), TokenKind::BracketOpen)
        {
            // Computed key: [expr]
            self.advance()?;
            let expr = self.parse_expression()?;
            self.expect(&TokenKind::BracketClose)?;
            (true, expr, None)
        } else if matches!(self.current_kind(), TokenKind::Hash) {
            // Private identifier key: #name
            let private_id = self.parse_private_identifier()?;
            (false, Expression::PrivateIdentifier(private_id), None)
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
                    optional: false,
                    type_annotation: None,
                    span: Span::new(key_start as u32, key_end as u32),
                }),
                Some(name_str),
            )
        } else {
            return Err(ParseError::InvalidSyntax {
                message: "Expected class member name".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        };

        // Detect if this is a method (has `(`) or property (has `=` or `;` or end of class)
        if matches!(self.current_kind(), TokenKind::ParenOpen) {
            // Method definition - use accessor_kind if set, otherwise check for constructor
            let kind = accessor_kind.unwrap_or(match method_name.as_deref() {
                Some("constructor") => MethodKind::Constructor,
                _ => MethodKind::Method,
            });

            // Parse parameter list and block body (like a function)
            let params = self.parse_parameter_list()?;

            // Check for return type annotation: (): type or type predicate
            let return_type = if self.check(&TokenKind::Colon) {
                Some(self.parse_return_type_annotation()?)
            } else {
                None
            };

            // Abstract methods and overload signatures have no body - just a semicolon
            // Method overloads: `parse(x: string): object;` followed by implementation
            // Note: ASI can insert semicolon on line terminator, but NOT if next token is `{`
            // (a method with body on next line: `fn()\n{` is valid)
            let is_overload_or_abstract = is_abstract
                || self.check(&TokenKind::Semicolon)
                || (self.can_insert_semicolon() && !self.check(&TokenKind::BraceOpen));
            let (body_block, end) = if is_overload_or_abstract {
                let end = return_type
                    .as_ref()
                    .map_or_else(|| self.current_pos().0 as u32, |rt| rt.span.end);
                self.eat(TokenKind::Semicolon);
                // Create empty body for abstract methods and overload signatures
                (
                    BlockStatement {
                        body: Vec::new(),
                        span: Span::new(end, end),
                    },
                    end,
                )
            } else {
                let body_block = self.parse_block_statement()?;
                let end = body_block.span.end;
                (body_block, end)
            };

            // Create FunctionExpression for the method value
            let value = FunctionExpression {
                id: None,
                type_parameters: None, // TODO: parse type parameters for class methods
                params,
                return_type,
                body: body_block,
                generator: is_generator,
                r#async: is_async,
                span: Span::new(start as u32, end),
            };

            Ok(ClassMember::MethodDefinition(MethodDefinition {
                decorators,
                key,
                value,
                kind,
                accessibility,
                is_static,
                r#override: is_override,
                r#abstract: is_abstract,
                computed,
                span: Span::new(start as u32, end),
            }))
        } else {
            // Property definition: `name: type = value;` or `name: type;` or `name = value;` or `name;`

            // Check for optional marker (`?`) or definite assignment assertion (`!`)
            let modifier = if self.eat(TokenKind::Question) {
                PropertyModifier::Optional
            } else if self.eat(TokenKind::Bang) {
                PropertyModifier::Definite
            } else {
                PropertyModifier::None
            };

            // Check for type annotation: `name: type`
            let type_annotation = if self.check(&TokenKind::Colon) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            // Check for value: `= value`
            let value = if self.eat(TokenKind::Equals) {
                Some(self.parse_assignment_expression()?)
            } else {
                None
            };

            let end = value.as_ref().map_or_else(
                || {
                    type_annotation
                        .as_ref()
                        .map_or_else(|| key.span().end, |ta| ta.span.end)
                },
                |v| v.span().end,
            );

            // Consume optional semicolon (ASI applies)
            self.eat(TokenKind::Semicolon);

            Ok(ClassMember::PropertyDefinition(PropertyDefinition {
                decorators,
                key,
                type_annotation,
                value,
                accessibility,
                is_static,
                r#abstract: is_abstract,
                readonly,
                computed,
                accessor,
                modifier,
                span: Span::new(start as u32, end),
            }))
        }
    }

    /// Check if current position has type arguments followed by a call: `<T>(`
    /// Used for extends clause to distinguish `getMixin<T>(Base)` from `extends Base<T>`
    fn is_type_args_followed_by_call(&self) -> bool {
        let bytes = self.source.as_bytes();
        let mut pos = self.current_start;

        // Must start with '<'
        if pos >= bytes.len() || bytes[pos] != b'<' {
            return false;
        }
        pos += 1;

        // Track nesting to find the matching '>'
        let mut depth = 1;
        while pos < bytes.len() && depth > 0 {
            match bytes[pos] {
                b'<' => depth += 1,
                b'>' => depth -= 1,
                b'\'' | b'"' | b'`' => {
                    // Skip strings
                    let quote = bytes[pos];
                    pos += 1;
                    while pos < bytes.len() && bytes[pos] != quote {
                        if bytes[pos] == b'\\' {
                            pos += 1; // skip escaped char
                        }
                        pos += 1;
                    }
                }
                _ => {}
            }
            pos += 1;
        }

        // Skip whitespace after '>'
        while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
            pos += 1;
        }

        // Check if '(' follows
        pos < bytes.len() && bytes[pos] == b'('
    }

    /// Parse a heritage expression for extends clause
    /// This can be an identifier, member expression, or call expression (mixin pattern)
    /// e.g., `Base`, `Foo.Bar`, `getMixin(Base)`, `ns.getMixin(Base)`
    fn parse_heritage_expression(&mut self) -> Result<Expression, ParseError> {
        // Start with an identifier
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(ParseError::InvalidSyntax {
                message: "Expected class name after 'extends'".to_string(),
                position: self.current_pos().0,
                context: None,
            });
        }

        let (start, end) = self.current_pos();
        let name = self.intern_identifier();
        self.advance()?;

        let mut expr = Expression::Identifier(Identifier {
            name,
            optional: false,
            type_annotation: None,
            span: Span::new(start as u32, end as u32),
        });

        // Parse member access chain and call expressions: `.identifier`, `(args)`
        // Keywords are valid property names in member expressions
        loop {
            match self.current_kind() {
                TokenKind::Dot => {
                    self.advance()?; // consume '.'

                    if !self.current_is_identifier_or_keyword() {
                        return Err(ParseError::InvalidSyntax {
                            message: "Expected property name after '.'".to_string(),
                            position: self.current_pos().0,
                            context: None,
                        });
                    }

                    let (prop_start, prop_end) = self.current_pos();
                    let prop_name = self.intern(self.current_property_name());
                    self.advance()?;

                    expr = Expression::MemberExpression(MemberExpression {
                        object: Box::new(expr),
                        property: Box::new(Expression::Identifier(Identifier {
                            name: prop_name,
                            optional: false,
                            type_annotation: None,
                            span: Span::new(prop_start as u32, prop_end as u32),
                        })),
                        computed: false,
                        optional: false,
                        span: Span::new(start as u32, prop_end as u32),
                    });
                }
                TokenKind::LessThan
                    if self.is_type_arguments_start() && self.is_type_args_followed_by_call() =>
                {
                    // Type arguments on call: getMixin<T>(Base)
                    // We've verified with lookahead that ( follows after >
                    let type_args = self.parse_type_parameter_instantiation()?;
                    self.advance()?; // consume '('
                    let (arguments, paren_end) = self.parse_call_arguments()?;
                    expr = Expression::CallExpression(CallExpression {
                        callee: Box::new(expr),
                        type_arguments: Some(type_args),
                        arguments,
                        optional: false,
                        span: Span::new(start as u32, paren_end as u32),
                    });
                }
                TokenKind::ParenOpen => {
                    // Call expression: getMixin(Base) or ns.getMixin(Base)
                    self.advance()?; // consume '('
                    let (arguments, paren_end) = self.parse_call_arguments()?;
                    expr = Expression::CallExpression(CallExpression {
                        callee: Box::new(expr),
                        type_arguments: None,
                        arguments,
                        optional: false,
                        span: Span::new(start as u32, paren_end as u32),
                    });
                }
                _ => break,
            }
        }

        Ok(expr)
    }
}
