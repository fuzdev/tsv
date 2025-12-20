// TypeScript type parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

/// Check if a byte can start an ASCII identifier
#[inline]
fn is_ident_start(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$')
}

/// Check if a byte can continue an ASCII identifier
#[inline]
fn is_ident_continue(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'$')
}

impl<'a> Parser<'a> {
    pub(in crate::parser) fn parse_type_annotation(
        &mut self,
    ) -> Result<TSTypeAnnotation, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::Colon)?;

        let type_node = self.parse_type()?;
        let end = type_node.span().end;

        Ok(TSTypeAnnotation {
            type_annotation: Box::new(type_node),
            span: Span::new(start as u32, end),
        })
    }

    /// Parse a complete type expression (handles unions and conditional types at top level)
    pub(in crate::parser) fn parse_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        let check_type = self.parse_union_type()?;

        // Check for conditional type: `T extends U ? V : W`
        if self.check(&TokenKind::Keyword(KeywordKind::Extends)) {
            self.advance()?; // consume 'extends'

            let extends_type = self.parse_union_type()?;

            // Expect '?'
            self.expect(&TokenKind::Question)?;

            let true_type = self.parse_type()?;

            // Expect ':'
            self.expect(&TokenKind::Colon)?;

            let false_type = self.parse_type()?;
            let end = false_type.span().end;

            Ok(TSType::Conditional(TSConditionalType {
                check_type: Box::new(check_type),
                extends_type: Box::new(extends_type),
                true_type: Box::new(true_type),
                false_type: Box::new(false_type),
                span: Span::new(start as u32, end),
            }))
        } else {
            Ok(check_type)
        }
    }

    /// Parse union type: `A | B | C` or `| A | B | C`
    fn parse_union_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;

        // Handle leading pipe: `| A | B`
        let has_leading_pipe = self.check(&TokenKind::Pipe);
        if has_leading_pipe {
            self.advance()?; // consume leading '|'
        }

        let first = self.parse_intersection_type()?;

        if !has_leading_pipe && !self.check(&TokenKind::Pipe) {
            return Ok(first);
        }

        let mut types = vec![first];
        while self.check(&TokenKind::Pipe) {
            self.advance()?; // consume '|'
            types.push(self.parse_intersection_type()?);
        }

        let end = types.last().map_or_else(|| start as u32, |t| t.span().end);
        Ok(TSType::Union(TSUnionType {
            types,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse intersection type: `A & B & C` or `& A & B & C`
    fn parse_intersection_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;

        // Handle leading ampersand: `& A & B`
        let has_leading_amp = self.check(&TokenKind::Ampersand);
        if has_leading_amp {
            self.advance()?; // consume leading '&'
        }

        let first = self.parse_array_type()?;

        if !has_leading_amp && !self.check(&TokenKind::Ampersand) {
            return Ok(first);
        }

        let mut types = vec![first];
        while self.check(&TokenKind::Ampersand) {
            self.advance()?; // consume '&'
            types.push(self.parse_array_type()?);
        }

        let end = types.last().map_or_else(|| start as u32, |t| t.span().end);
        Ok(TSType::Intersection(TSIntersectionType {
            types,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse array type suffix `T[]` or indexed access type `T[K]`
    fn parse_array_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        let mut result = self.parse_primary_type()?;

        // Check for array type suffix [] or indexed access T[K]
        while self.check(&TokenKind::BracketOpen) {
            if matches!(self.peek_kind(), TokenKind::BracketClose) {
                // Array type: T[]
                self.advance()?; // consume '['
                let (_, arr_end) = self.current_pos();
                self.expect(&TokenKind::BracketClose)?;

                result = TSType::Array(TSArrayType {
                    element_type: Box::new(result),
                    span: Span::new(start as u32, arr_end as u32),
                });
            } else {
                // Indexed access type: T[K]
                self.advance()?; // consume '['
                let index_type = self.parse_type()?;
                let (_, end) = self.current_pos();
                self.expect(&TokenKind::BracketClose)?;

                result = TSType::IndexedAccess(TSIndexedAccessType {
                    object_type: Box::new(result),
                    index_type: Box::new(index_type),
                    span: Span::new(start as u32, end as u32),
                });
            }
        }

        Ok(result)
    }

    /// Parse primary type (highest precedence)
    fn parse_primary_type(&mut self) -> Result<TSType, ParseError> {
        let (start, end) = self.current_pos();
        let span = Span::new(start as u32, end as u32);

        // Keyword types: string, number, boolean, true, false, etc.
        if let TokenKind::Keyword(kw) = self.current_kind()
            && let Some(ts_kind) = TSKeywordKind::from_lexer_keyword(*kw)
        {
            self.advance()?;
            return Ok(TSType::Keyword(TSKeywordType::new(ts_kind, span)));
        }

        match self.current_kind() {
            // `const` keyword in type context (for `as const`)
            // Treated as a type reference with name "const"
            TokenKind::Keyword(KeywordKind::Const) => {
                let (start, end) = self.current_pos();
                let symbol = self.intern("const");
                self.advance()?;

                Ok(TSType::TypeReference(TSTypeReference {
                    type_name: TSEntityName::Identifier(Identifier::simple(
                        symbol,
                        Span::new(start as u32, end as u32),
                    )),
                    type_arguments: None,
                    span: Span::new(start as u32, end as u32),
                }))
            }
            // Numeric literal types: `1`, `42.5`, `1n`
            TokenKind::Number => {
                let (start, end) = self.current_pos();
                let raw = self.current_value();
                let is_bigint = raw.ends_with('n');

                let literal = if is_bigint {
                    // BigInt: strip the 'n' suffix for value
                    let value = raw[..raw.len() - 1].to_string();
                    Literal {
                        value: LiteralValue::BigInt(value),
                        span: Span::new(start as u32, end as u32),
                    }
                } else {
                    // Regular number
                    let number = super::super::expression::parse_number_literal(raw)
                        .map_err(|_| self.error_msg_at(&format!("Invalid number: {raw}"), start))?;
                    Literal {
                        value: LiteralValue::Number(number),
                        span: Span::new(start as u32, end as u32),
                    }
                };
                self.advance()?;

                if is_bigint {
                    Ok(TSType::Literal(TSLiteralType::BigInt(literal)))
                } else {
                    Ok(TSType::Literal(TSLiteralType::Number(literal)))
                }
            }
            // String literal types: `"hello"`, `'world'`
            TokenKind::String => {
                let (start, end) = self.current_pos();
                let (content, quote) = self.extract_string_literal();
                self.advance()?;

                Ok(TSType::Literal(TSLiteralType::String(Literal {
                    value: LiteralValue::String { content, quote },
                    span: Span::new(start as u32, end as u32),
                })))
            }
            // Negative number literal types: `-1`, `-42n`
            TokenKind::Minus => {
                let start = self.current_pos().0;
                self.advance()?; // consume '-'

                if !matches!(self.current_kind(), TokenKind::Number) {
                    return Err(self.error_expected("number after '-' in type context"));
                }

                let (num_start, num_end) = self.current_pos();
                let raw = self.current_value();
                let is_bigint = raw.ends_with('n');

                let argument = if is_bigint {
                    let value = raw[..raw.len() - 1].to_string();
                    Literal {
                        value: LiteralValue::BigInt(value),
                        span: Span::new(num_start as u32, num_end as u32),
                    }
                } else {
                    let number =
                        super::super::expression::parse_number_literal(raw).map_err(|_| {
                            self.error_msg_at(&format!("Invalid number: {raw}"), num_start)
                        })?;
                    Literal {
                        value: LiteralValue::Number(number),
                        span: Span::new(num_start as u32, num_end as u32),
                    }
                };
                self.advance()?;

                let unary = UnaryExpression {
                    operator: UnaryOperator::Minus,
                    prefix: true,
                    argument: Box::new(Expression::Literal(argument)),
                    span: Span::new(start as u32, num_end as u32),
                };
                Ok(TSType::Literal(TSLiteralType::UnaryExpression(unary)))
            }
            // Template literal types
            TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead => {
                let template = self.parse_template_literal_type()?;
                Ok(TSType::Literal(TSLiteralType::TemplateLiteral(template)))
            }
            // Parenthesized type or function type: (T) or (x: T) => U
            TokenKind::ParenOpen => self.parse_parenthesized_or_function_type(),
            // Object type: { prop: T }
            TokenKind::BraceOpen => self.parse_object_type(),
            // Tuple type: [T, U]
            TokenKind::BracketOpen => self.parse_tuple_type(),
            // Type reference or type operator (keyof, unique, readonly) or infer or abstract constructor
            TokenKind::Identifier => {
                // Check for type operators: keyof, unique, readonly, abstract
                match self.current_value() {
                    "keyof" => self.parse_type_operator(TSTypeOperatorKind::Keyof),
                    "unique" => self.parse_type_operator(TSTypeOperatorKind::Unique),
                    "readonly" => {
                        // `readonly` could be:
                        // 1. Type operator: `readonly T[]`
                        // 2. Part of mapped type: `{ readonly [K in T]: V }` (handled elsewhere)
                        // 3. Type reference named "readonly" (rare but valid)
                        // We'll parse as type operator when followed by a type
                        if self.peek_is_type_start() {
                            self.parse_type_operator(TSTypeOperatorKind::Readonly)
                        } else {
                            self.parse_type_reference()
                        }
                    }
                    "abstract" => {
                        // Check if next token is 'new' for abstract constructor type
                        if matches!(self.peek_kind(), TokenKind::Keyword(KeywordKind::New)) {
                            self.parse_constructor_type(true)
                        } else {
                            // 'abstract' as a type reference (rare but valid)
                            self.parse_type_reference()
                        }
                    }
                    "infer" => self.parse_infer_type(),
                    _ => self.parse_type_reference(),
                }
            }
            // Import type: import('module') or import('module').Foo<T>
            TokenKind::Keyword(KeywordKind::Import) => self.parse_import_type(),
            // Generic function type: <T>() => U
            TokenKind::LessThan => self.parse_generic_function_type(),
            // Type query: typeof x, typeof Foo.bar, typeof import("module")
            TokenKind::Keyword(KeywordKind::Typeof) => self.parse_type_query(),
            // Constructor type: new () => T or new <T>() => T
            TokenKind::Keyword(KeywordKind::New) => self.parse_constructor_type(false),
            _ => Err(self.error_expected_found("type")),
        }
    }

    /// Parse type operator: `keyof T`, `unique symbol`, `readonly T[]`
    fn parse_type_operator(&mut self, operator: TSTypeOperatorKind) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        self.advance()?; // consume the operator keyword (keyof, unique, readonly)

        // Parse the type being operated on
        let type_annotation = self.parse_primary_type()?;
        let end = type_annotation.span().end;

        Ok(TSType::TypeOperator(TSTypeOperator {
            operator,
            type_annotation: Box::new(type_annotation),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse infer type: `infer U` (in conditional type extends clause)
    fn parse_infer_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        self.advance()?; // consume 'infer'

        // Parse the type parameter name (must be an identifier)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected("type parameter name after 'infer'"));
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern_identifier();
        self.advance()?;

        let name = Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32));

        Ok(TSType::Infer(TSInferType {
            type_parameter: TSTypeParameter {
                name,
                constraint: None,
                default: None,
                is_const: false,
                is_in: false,
                is_out: false,
                span: Span::new(id_start as u32, id_end as u32),
            },
            span: Span::new(start as u32, id_end as u32),
        }))
    }

    /// Check if the peek token could start a type
    fn peek_is_type_start(&mut self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Identifier
                | TokenKind::ParenOpen
                | TokenKind::BraceOpen
                | TokenKind::BracketOpen
                | TokenKind::Keyword(_)
        )
    }

    /// Check if current position starts an index signature: `[key: type]: T`
    /// vs a computed property: `[expr]: T`
    ///
    /// Index signatures always have the form `[identifier: type]` where the identifier
    /// is immediately followed by `:`. Computed properties have `[expression]` where
    /// the expression can be any expression followed by `]`.
    pub(in crate::parser) fn is_index_signature_start(&self) -> bool {
        // Must start with '['
        if !matches!(self.current_kind, TokenKind::BracketOpen) {
            return false;
        }

        // Lookahead: check if pattern is `[identifier:`
        // We need to look past the '[', then the identifier, then check for ':'
        let bytes = self.source.as_bytes();
        let mut pos = self.current_start + 1; // skip '['

        // Skip whitespace
        while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
            pos += 1;
        }

        // Must be followed by an identifier
        if pos >= bytes.len() || !is_ident_start(bytes[pos]) {
            return false;
        }

        // Skip the identifier
        pos += 1;
        while pos < bytes.len() && is_ident_continue(bytes[pos]) {
            pos += 1;
        }

        // Skip whitespace
        while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
            pos += 1;
        }

        // Check for ':'
        pos < bytes.len() && bytes[pos] == b':'
    }

    /// Parse type reference: `Foo` or `Foo.Bar` or `Foo<T>`
    fn parse_type_reference(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        let type_name = self.parse_entity_name()?;

        // Check for type arguments: <T, U>
        let type_arguments = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_arguments()?)
        } else {
            None
        };

        let end = type_arguments
            .as_ref()
            .map_or_else(|| type_name.span().end, |ta| ta.span.end);

        Ok(TSType::TypeReference(TSTypeReference {
            type_name,
            type_arguments,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse import type: `import('module')` or `import('module', {with: {...}}).Foo<T>`
    fn parse_import_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        self.advance()?; // consume 'import'

        // Expect '('
        self.expect(&TokenKind::ParenOpen)?;

        // Parse the module specifier (string literal)
        if !matches!(self.current_kind(), TokenKind::String) {
            return Err(self.error_expected("string literal in import type"));
        }

        let (arg_start, arg_end) = self.current_pos();
        let (content, quote) = self.extract_string_literal();
        self.advance()?;

        let argument = Literal {
            value: LiteralValue::String { content, quote },
            span: Span::new(arg_start as u32, arg_end as u32),
        };

        // Optional options object: `import('module', {with: {type: 'json'}})`
        let options = if self.check(&TokenKind::Comma) {
            self.advance()?; // consume ','
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        // Expect ')'
        self.expect(&TokenKind::ParenClose)?;

        // Optional qualifier: .Foo or .Foo.Bar
        let qualifier = if self.check(&TokenKind::Dot) {
            self.advance()?; // consume '.'
            Some(self.parse_entity_name()?)
        } else {
            None
        };

        // Optional type arguments: <T, U>
        let type_arguments = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_arguments()?)
        } else {
            None
        };

        let end = type_arguments
            .as_ref()
            .map(|ta| ta.span.end)
            .or_else(|| qualifier.as_ref().map(|q| q.span().end))
            .unwrap_or(self.prev_end as u32);

        Ok(TSType::Import(TSImportType {
            argument,
            options,
            qualifier,
            type_arguments,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`
    fn parse_type_query(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        self.advance()?; // consume 'typeof'

        // Check for import type: typeof import("module")
        let expr_name = if self.check(&TokenKind::Keyword(KeywordKind::Import)) {
            // Parse the import type
            let import_start = self.current_pos().0;
            self.advance()?; // consume 'import'

            // Expect '('
            self.expect(&TokenKind::ParenOpen)?;

            // Parse the module specifier (string literal)
            if !matches!(self.current_kind(), TokenKind::String) {
                return Err(self.error_expected("string literal in import type"));
            }

            let (arg_start, arg_end) = self.current_pos();
            let (content, quote) = self.extract_string_literal();
            self.advance()?;

            let argument = Literal {
                value: LiteralValue::String { content, quote },
                span: Span::new(arg_start as u32, arg_end as u32),
            };

            // Optional options object: `typeof import('module', {with: {...}})`
            let options = if self.check(&TokenKind::Comma) {
                self.advance()?; // consume ','
                Some(Box::new(self.parse_expression()?))
            } else {
                None
            };

            // Expect ')'
            self.expect(&TokenKind::ParenClose)?;

            // Optional qualifier: .Foo or .Foo.Bar
            let qualifier = if self.check(&TokenKind::Dot) {
                self.advance()?; // consume '.'
                Some(self.parse_entity_name()?)
            } else {
                None
            };

            // Optional type arguments: <T, U>
            let type_arguments = if self.check(&TokenKind::LessThan) {
                Some(self.parse_type_arguments()?)
            } else {
                None
            };

            let import_end = type_arguments
                .as_ref()
                .map(|ta| ta.span.end)
                .or_else(|| qualifier.as_ref().map(|q| q.span().end))
                .unwrap_or(self.prev_end as u32);

            TSTypeQueryExprName::Import(Box::new(TSImportType {
                argument,
                options,
                qualifier,
                type_arguments,
                span: Span::new(import_start as u32, import_end),
            }))
        } else {
            // Parse entity name: identifier or qualified name
            let entity_name = self.parse_entity_name()?;
            TSTypeQueryExprName::EntityName(entity_name)
        };

        // Parse optional type arguments: typeof Array<string>
        let type_arguments = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_arguments()?)
        } else {
            None
        };

        let end = type_arguments
            .as_ref()
            .map_or_else(|| expr_name.span().end, |ta| ta.span.end);

        Ok(TSType::TypeQuery(TSTypeQuery {
            expr_name,
            type_arguments,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse entity name: `Foo` or `Foo.Bar.Baz`
    pub(crate) fn parse_entity_name(&mut self) -> Result<TSEntityName, ParseError> {
        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern_identifier();
        self.advance()?;

        let mut result = TSEntityName::Identifier(Identifier::simple(
            symbol,
            Span::new(id_start as u32, id_end as u32),
        ));

        while self.check(&TokenKind::Dot) {
            self.advance()?; // consume '.'

            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(self.error_expected_after("identifier", "."));
            }

            let (right_start, right_end) = self.current_pos();
            let right_symbol = self.intern_identifier();
            self.advance()?;

            let right = Identifier::simple(
                right_symbol,
                Span::new(right_start as u32, right_end as u32),
            );

            result = TSEntityName::QualifiedName(Box::new(TSQualifiedName {
                left: result,
                right,
                span: Span::new(id_start as u32, right_end as u32),
            }));
        }

        Ok(result)
    }

    /// Parse type arguments: `<T, U>`
    pub(in crate::parser) fn parse_type_arguments(
        &mut self,
    ) -> Result<TSTypeParameterInstantiation, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::LessThan)?;

        let mut params = Vec::new();
        if !self.check_greater_than_in_type() {
            params.push(self.parse_type()?);
            while self.eat(TokenKind::Comma) {
                params.push(self.parse_type()?);
            }
        }

        let (_, end) = self.current_pos();
        self.expect_greater_than_in_type()?;

        Ok(TSTypeParameterInstantiation {
            params,
            span: Span::new(start as u32, end as u32),
        })
    }

    /// Parse parenthesized type `(T)` or function type `(x: T) => U`
    fn parse_parenthesized_or_function_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::ParenOpen)?;

        // Check if this is definitely a parenthesized type (not function params)
        // Types that can't be parameter names: typeof, keyof, [, {, -, etc.
        if self.is_definitely_type_start() {
            let inner_type = self.parse_type()?;
            let end = self.current_pos().0;
            self.expect(&TokenKind::ParenClose)?;

            return Ok(TSType::Parenthesized(TSParenthesizedType {
                type_annotation: Box::new(inner_type),
                span: Span::new(start as u32, end as u32),
            }));
        }

        // Try to parse as function parameters
        let params = self.parse_function_type_params()?;
        self.expect(&TokenKind::ParenClose)?;

        // Check for arrow => to determine if it's a function type
        if self.check(&TokenKind::Arrow) {
            let arrow_start = self.current_pos().0 as u32;
            self.advance()?; // consume '=>'
            // Parse return type, which may be a type predicate (asserts x, x is T)
            let return_type = self.parse_return_type_inner(arrow_start)?;
            let end = return_type.span.end;

            Ok(TSType::Function(TSFunctionType {
                type_parameters: None,
                params,
                return_type: Box::new(return_type),
                span: Span::new(start as u32, end),
            }))
        } else if params.len() == 1 && !self.is_function_param(&params[0]) {
            // Single identifier without type annotation - could be parenthesized type
            // But for simplicity, we'll treat `()` as function type
            if let Expression::Identifier(id) = &params[0] {
                // Parse this as a type reference wrapped in parentheses
                let type_ref = TSType::TypeReference(TSTypeReference {
                    type_name: TSEntityName::Identifier(id.clone()),
                    type_arguments: None,
                    span: id.span,
                });
                let (_, end) = (self.current_pos().0, params[0].span().end);
                Ok(TSType::Parenthesized(TSParenthesizedType {
                    type_annotation: Box::new(type_ref),
                    span: Span::new(start as u32, end),
                }))
            } else {
                Err(self.error_msg("Invalid parenthesized type"))
            }
        } else {
            // Empty params or params with types - function type with implicit void return
            let end = self.current_pos().0 as u32;
            Ok(TSType::Function(TSFunctionType {
                type_parameters: None,
                params,
                return_type: Box::new(TSTypeAnnotation {
                    type_annotation: Box::new(TSType::Keyword(TSKeywordType::new(
                        TSKeywordKind::Void,
                        Span::new(end, end),
                    ))),
                    span: Span::new(end, end),
                }),
                span: Span::new(start as u32, end),
            }))
        }
    }

    /// Check if current token definitely starts a type (not a valid parameter name)
    fn is_definitely_type_start(&mut self) -> bool {
        match self.current_kind() {
            // Keywords that are types, not parameter names
            TokenKind::Keyword(KeywordKind::Typeof) => true,
            // Type keywords: string, number, boolean, any, void, never, unknown, object, symbol, bigint, null, undefined
            TokenKind::Keyword(kw) if kw.is_type_keyword() => true,
            // Constructor types: new () => T
            TokenKind::Keyword(KeywordKind::New) => true,
            // Non-identifier tokens that start types
            TokenKind::BracketOpen => true, // tuple types
            TokenKind::BraceOpen => true,   // object types
            TokenKind::LessThan => true,    // generic function types
            TokenKind::Minus => true,       // negative number literals
            TokenKind::ParenOpen => true,   // nested parenthesized types
            // String/number literals are types, not params
            TokenKind::String | TokenKind::Number => true,
            // Template literals
            TokenKind::NoSubstitutionTemplate | TokenKind::TemplateHead => true,
            // Type operators like keyof, readonly, unique, infer, and abstract (for constructor types)
            TokenKind::Identifier => {
                let val = self.current_value();
                if matches!(val, "keyof" | "unique" | "readonly" | "infer") {
                    return true;
                }
                // Abstract constructor types: abstract new () => T
                if val == "abstract" {
                    return matches!(self.peek_kind(), TokenKind::Keyword(KeywordKind::New));
                }
                // If an identifier is followed by these tokens, it's a type not a param:
                // (A | B) is a union type, not function params
                // (A & B) is an intersection type
                // (A[K]) is an indexed access type
                // (T extends U ? V : W) is a conditional type in parentheses
                // (ns.X) is a qualified type reference
                matches!(
                    self.peek_kind(),
                    TokenKind::Pipe
                        | TokenKind::Ampersand
                        | TokenKind::BracketOpen
                        | TokenKind::Keyword(KeywordKind::Extends)
                        | TokenKind::Dot
                )
            }
            _ => false,
        }
    }

    /// Parse generic function type: `<T>() => U`, `<T, U extends V>(x: T) => U`
    fn parse_generic_function_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;

        // Parse type parameters: <T, U extends V, ...>
        let type_parameters = self.parse_type_parameters()?;

        // Parse parameter list
        self.expect(&TokenKind::ParenOpen)?;
        let params = self.parse_function_type_params()?;
        self.expect(&TokenKind::ParenClose)?;

        // Expect arrow
        let arrow_start = self.current_pos().0 as u32;
        self.expect(&TokenKind::Arrow)?;

        // Parse return type (may be a type predicate)
        let return_type = self.parse_return_type_inner(arrow_start)?;
        let end = return_type.span.end;

        Ok(TSType::Function(TSFunctionType {
            type_parameters: Some(type_parameters),
            params,
            return_type: Box::new(return_type),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse constructor type: `new () => T`, `new <T>() => T`, `abstract new () => T`
    fn parse_constructor_type(&mut self, is_abstract: bool) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;

        // If abstract, consume 'abstract' keyword
        if is_abstract {
            self.advance()?; // consume 'abstract'
        }

        // Expect 'new' keyword
        self.expect(&TokenKind::Keyword(KeywordKind::New))?;

        // Parse optional type parameters: <T>
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Parse parameter list
        self.expect(&TokenKind::ParenOpen)?;
        let params = self.parse_function_type_params()?;
        self.expect(&TokenKind::ParenClose)?;

        // Expect arrow
        let arrow_start = self.current_pos().0 as u32;
        self.expect(&TokenKind::Arrow)?;

        // Parse return type (may be a type predicate)
        let return_type = self.parse_return_type_inner(arrow_start)?;
        let end = return_type.span.end;

        Ok(TSType::Constructor(TSConstructorType {
            abstract_: is_abstract,
            type_parameters,
            params,
            return_type: Box::new(return_type),
            span: Span::new(start as u32, end),
        }))
    }

    /// Check if an expression is a function parameter (has type annotation)
    fn is_function_param(&self, expr: &Expression) -> bool {
        match expr {
            Expression::Identifier(id) => id.type_annotation.is_some() || id.optional,
            _ => true,
        }
    }

    /// Parse function type parameters
    fn parse_function_type_params(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut params = Vec::new();

        if !self.check(&TokenKind::ParenClose) {
            params.push(self.parse_function_type_param()?);
            while self.eat(TokenKind::Comma) {
                // Handle trailing comma
                if self.check(&TokenKind::ParenClose) {
                    break;
                }
                params.push(self.parse_function_type_param()?);
            }
        }

        Ok(params)
    }

    /// Parse a single function type parameter
    fn parse_function_type_param(&mut self) -> Result<Expression, ParseError> {
        // Check for rest parameter: ...args
        if self.check(&TokenKind::DotDotDot) {
            let (start, _) = self.current_pos();
            self.advance()?;
            let arg = self.parse_function_type_param()?;
            let end = arg.span().end;
            return Ok(Expression::RestElement(RestElement {
                argument: Box::new(arg),
                span: Span::new(start as u32, end),
            }));
        }

        let (id_start, id_end) = self.current_pos();

        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected("parameter name"));
        }

        let symbol = self.intern_identifier();
        self.advance()?;

        // Check for optional: ?
        let optional = self.eat(TokenKind::Question);

        // Check for type annotation: : T
        let type_annotation = if self.check(&TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let end = type_annotation
            .as_ref()
            .map_or_else(|| id_end as u32, |ta| ta.span.end);

        Ok(Expression::Identifier(Identifier {
            name: symbol,
            optional,
            type_annotation,
            decorators: None,
            span: Span::new(id_start as u32, end),
        }))
    }

    /// Parse object type: `{ prop: T; method(): U }` or mapped type: `{ [K in T]: V }`
    fn parse_object_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::BraceOpen)?;

        // Check for unambiguous mapped type modifiers: +/- (with or without readonly)
        // +readonly, -readonly, +, - all indicate mapped type
        if self.check(&TokenKind::Minus) || self.check(&TokenKind::Plus) {
            return self.parse_mapped_type_body(start);
        }

        // For `readonly [` or bare `[`, we need to look ahead to distinguish:
        // - Mapped type: `{ readonly [K in keyof T]: V }` or `{ [K in keyof T]: V }`
        // - Index signature: `{ readonly [key: string]: T }` or `{ [key: string]: T }`
        // The distinction is `in` vs `:` after the identifier
        if self.check(&TokenKind::BracketOpen) {
            return self.parse_bracket_type_member_or_mapped(start, false);
        }

        // Check for `readonly [` - needs disambiguation
        if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "readonly"
            && matches!(self.peek_kind(), TokenKind::BracketOpen)
        {
            self.advance()?; // consume 'readonly'
            return self.parse_bracket_type_member_or_mapped(start, true);
        }

        let mut members = Vec::new();
        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            members.push(self.parse_type_element()?);
            // Consume separator (; or ,) if present
            if !self.eat(TokenKind::Semicolon) {
                self.eat(TokenKind::Comma);
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(TSType::TypeLiteral(TSTypeLiteral {
            members,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse `[...]` that could be either a mapped type or index signature.
    /// Consumes tokens to disambiguate, then continues with the appropriate parse.
    /// `readonly` indicates whether we already consumed a `readonly` keyword.
    fn parse_bracket_type_member_or_mapped(
        &mut self,
        start: usize,
        readonly: bool,
    ) -> Result<TSType, ParseError> {
        // At this point: current is `[`
        self.expect(&TokenKind::BracketOpen)?;

        // Parse the identifier (parameter name for mapped type, or key name for index sig)
        let param_start = self.current_pos().0;
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected_after("identifier", "["));
        }
        let param_name = self.current_value().to_string();
        let param_symbol = self.intern(&param_name);
        let (id_start, id_end) = self.current_pos();
        self.advance()?;

        // NOW we can distinguish: `in` means mapped type, `:` means index signature
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::In)) {
            // Mapped type: [K in keyof T]: V
            self.advance()?; // consume 'in'
            self.parse_mapped_type_body_after_in(start, param_start, param_name, readonly)
        } else if self.check(&TokenKind::Colon) {
            // Index signature: [key: string]: T
            // Parse it as a type element, then continue with remaining members
            self.parse_type_literal_starting_with_index_sig(
                start,
                param_symbol,
                id_start,
                id_end,
                readonly,
            )
        } else {
            Err(self.error_msg(&format!(
                "Expected 'in' or ':' after '[{}', found {}",
                param_name,
                self.current_kind()
            )))
        }
    }

    /// Continue parsing mapped type after `[K in` has been consumed
    fn parse_mapped_type_body_after_in(
        &mut self,
        start: usize,
        param_start: usize,
        param_name: String,
        readonly: bool,
    ) -> Result<TSType, ParseError> {
        // Parse constraint type (e.g., `keyof T`)
        let constraint = self.parse_type()?;
        let param_end = constraint.span().end;

        // Check for optional `as` clause: `as NewKey`
        let name_type = if self.eat(TokenKind::Keyword(KeywordKind::As)) {
            Some(Box::new(self.parse_type()?))
        } else {
            None
        };

        // Expect `]`
        self.expect(&TokenKind::BracketClose)?;

        // Parse optional modifier: `?`, `+?`, `-?`
        let optional = self.parse_mapped_type_optional_modifier();

        // Expect `:` and parse value type
        self.expect(&TokenKind::Colon)?;
        let type_annotation = Some(Box::new(self.parse_type()?));

        // Consume optional separator
        self.eat(TokenKind::Semicolon);

        // Expect `}`
        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(TSType::Mapped(TSMappedType {
            type_parameter: TSMappedTypeParameter {
                name: param_name,
                constraint: Box::new(constraint),
                span: Span::new(param_start as u32, param_end),
            },
            name_type,
            type_annotation,
            readonly: if readonly { Some(true) } else { None },
            optional,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse type literal that starts with an index signature
    /// Called after `[identifier` has been consumed and we saw `:`
    fn parse_type_literal_starting_with_index_sig(
        &mut self,
        start: usize,
        param_symbol: string_interner::DefaultSymbol,
        id_start: usize,
        _id_end: usize,
        readonly: bool,
    ) -> Result<TSType, ParseError> {
        // We're at `:` after `[identifier`
        // Parse: `: type]: value_type`
        self.expect(&TokenKind::Colon)?;
        let param_type = self.parse_type()?;

        let param = Identifier {
            name: param_symbol,
            optional: false,
            type_annotation: Some(TSTypeAnnotation {
                type_annotation: Box::new(param_type.clone()),
                span: param_type.span(),
            }),
            decorators: None,
            span: Span::new(id_start as u32, param_type.span().end),
        };

        self.expect(&TokenKind::BracketClose)?;
        self.expect(&TokenKind::Colon)?;

        let value_type = self.parse_type()?;
        let member_end = value_type.span().end;

        let index_sig = TSTypeElement::IndexSignature(TSIndexSignature {
            parameters: vec![param],
            type_annotation: TSTypeAnnotation {
                type_annotation: Box::new(value_type),
                span: Span::new(id_start as u32, member_end),
            },
            readonly,
            span: Span::new(start as u32, member_end),
        });

        // Now parse remaining members
        let mut members = vec![index_sig];

        // Consume separator (; or ,) if present
        if !self.eat(TokenKind::Semicolon) {
            self.eat(TokenKind::Comma);
        }

        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            members.push(self.parse_type_element()?);
            if !self.eat(TokenKind::Semicolon) {
                self.eat(TokenKind::Comma);
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(TSType::TypeLiteral(TSTypeLiteral {
            members,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse the body of a mapped type (after '{' has been consumed)
    fn parse_mapped_type_body(&mut self, start: usize) -> Result<TSType, ParseError> {
        // Parse optional readonly modifier: `readonly`, `+readonly`, `-readonly`
        let readonly = self.parse_mapped_type_readonly_modifier();

        // Expect `[`
        self.expect(&TokenKind::BracketOpen)?;

        // Parse type parameter name: `K`
        let param_start = self.current_pos().0;
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected("type parameter name in mapped type"));
        }
        let param_name = self.current_value().to_string();
        self.advance()?;

        // Expect `in`
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::In)) {
            return Err(self.error_expected("'in' in mapped type"));
        }
        self.advance()?; // consume 'in'

        // Parse constraint type (e.g., `keyof T`)
        let constraint = self.parse_type()?;
        let param_end = constraint.span().end;

        // Check for optional `as` clause: `as NewKey`
        let name_type = if self.eat(TokenKind::Keyword(KeywordKind::As)) {
            Some(Box::new(self.parse_type()?))
        } else {
            None
        };

        // Expect `]`
        self.expect(&TokenKind::BracketClose)?;

        // Parse optional modifier: `?`, `+?`, `-?`
        let optional = self.parse_mapped_type_optional_modifier();

        // Expect `:` and parse value type
        self.expect(&TokenKind::Colon)?;
        let type_annotation = Some(Box::new(self.parse_type()?));

        // Consume optional separator
        self.eat(TokenKind::Semicolon);

        // Expect `}`
        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(TSType::Mapped(TSMappedType {
            type_parameter: TSMappedTypeParameter {
                name: param_name,
                constraint: Box::new(constraint),
                span: Span::new(param_start as u32, param_end),
            },
            name_type,
            type_annotation,
            readonly,
            optional,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse readonly modifier for mapped type: `readonly`, `+readonly`, `-readonly`
    fn parse_mapped_type_readonly_modifier(&mut self) -> Option<bool> {
        if self.eat(TokenKind::Minus) {
            // `-readonly`
            if self.eat_contextual_keyword("readonly") {
                return Some(false); // false = minus modifier
            }
            // Unexpected - but we consumed `-`
        }

        if self.eat(TokenKind::Plus) {
            // `+readonly`
            if self.eat_contextual_keyword("readonly") {
                return Some(true); // true = plus modifier (same as just `readonly`)
            }
        }

        if self.eat_contextual_keyword("readonly") {
            return Some(true); // true = readonly present
        }

        None
    }

    /// Parse optional modifier for mapped type: `?`, `+?`, `-?`
    fn parse_mapped_type_optional_modifier(&mut self) -> Option<bool> {
        if self.eat(TokenKind::Minus) {
            // `-?`
            if self.eat(TokenKind::Question) {
                return Some(false); // false = minus modifier
            }
            // Unexpected - but we consumed `-`
        }

        if self.eat(TokenKind::Plus) {
            // `+?`
            if self.eat(TokenKind::Question) {
                return Some(true); // true = plus modifier
            }
        }

        if self.eat(TokenKind::Question) {
            return Some(true); // true = optional present
        }

        None
    }

    /// Parse tuple type: `[T, U, V]`, `[...T]`, `[label: T]`, `[T?]`, `[first: string, ...rest: T]`
    fn parse_tuple_type(&mut self) -> Result<TSType, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::BracketOpen)?;

        let mut element_types = Vec::new();
        if !self.check(&TokenKind::BracketClose) {
            element_types.push(self.parse_tuple_element()?);
            while self.eat(TokenKind::Comma) {
                if self.check(&TokenKind::BracketClose) {
                    break; // trailing comma
                }
                element_types.push(self.parse_tuple_element()?);
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BracketClose)?;

        Ok(TSType::Tuple(TSTupleType {
            element_types,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse a single tuple element: `T`, `T?`, `...T`, `label: T`, `label?: T`, `...label: T`
    fn parse_tuple_element(&mut self) -> Result<TSType, ParseError> {
        let elem_start = self.current_pos().0;

        // Check for rest element: `...T` or `...label: T`
        if self.check(&TokenKind::DotDotDot) {
            self.advance()?; // consume `...`
            let inner = self.parse_tuple_element_inner()?;
            let end = inner.span().end;
            return Ok(TSType::Rest(TSRestType {
                type_annotation: Box::new(inner),
                span: Span::new(elem_start as u32, end),
            }));
        }

        self.parse_tuple_element_inner()
    }

    /// Parse a tuple element (without leading `...`): `T`, `T?`, `label: T`, `label?: T`
    fn parse_tuple_element_inner(&mut self) -> Result<TSType, ParseError> {
        let elem_start = self.current_pos().0;

        // Check for named tuple member: `label: T` or `label?: T`
        // An identifier followed by `:` indicates a named tuple member
        // An identifier followed by `?:` indicates an optional named tuple member
        if matches!(self.current_kind(), TokenKind::Identifier)
            && matches!(self.peek_kind(), TokenKind::Colon | TokenKind::Question)
        {
            let (label_start, label_end) = self.current_pos();
            let label_symbol = self.intern_identifier();
            self.advance()?; // consume identifier

            // Check for optional marker `?` followed by `:`
            // This distinguishes `label?: T` (named optional) from `TypeRef?` (optional type ref)
            let optional = if self.check(&TokenKind::Question) {
                // Peek ahead: if next is `:`, this is `label?: T`
                // Otherwise, we misread - need to backtrack (but we can't easily)
                // Simpler approach: check for `:` after consuming `?`
                self.advance()?; // consume `?`
                if self.check(&TokenKind::Colon) {
                    true
                } else {
                    // This was actually `TypeRef?` - we need to create the type reference
                    // and wrap it in optional
                    let type_ref = TSType::TypeReference(TSTypeReference {
                        type_name: TSEntityName::Identifier(Identifier::simple(
                            label_symbol,
                            Span::new(label_start as u32, label_end as u32),
                        )),
                        type_arguments: None,
                        span: Span::new(label_start as u32, label_end as u32),
                    });
                    return Ok(TSType::Optional(TSOptionalType {
                        type_annotation: Box::new(type_ref),
                        span: Span::new(elem_start as u32, self.prev_end as u32),
                    }));
                }
            } else {
                false
            };

            // Expect `:` and parse the element type
            self.expect(&TokenKind::Colon)?;
            let element_type = self.parse_type()?;
            let end = element_type.span().end;

            return Ok(TSType::NamedTupleMember(TSNamedTupleMember {
                label: Identifier::simple(
                    label_symbol,
                    Span::new(label_start as u32, label_end as u32),
                ),
                element_type: Box::new(element_type),
                optional,
                span: Span::new(elem_start as u32, end),
            }));
        }

        // Parse as regular type, then check for trailing `?` (optional type)
        let inner_type = self.parse_type()?;

        // Check for optional suffix: `T?`
        if self.eat(TokenKind::Question) {
            let end = self.prev_end;
            Ok(TSType::Optional(TSOptionalType {
                type_annotation: Box::new(inner_type),
                span: Span::new(elem_start as u32, end as u32),
            }))
        } else {
            Ok(inner_type)
        }
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
                        return Err(self.error_expected("'}' after type in template literal"));
                    }

                    // Use lexer to continue template from }
                    let token = self
                        .lexer
                        .continue_template_from_brace(self.current_raw_end())?;
                    self.update_current(token);

                    match *self.current_kind() {
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
                            return Err(self.error_msg("Unexpected token in template literal type"));
                        }
                    }
                }
            }
            _ => Err(self.error_expected_found("template literal type")),
        }
    }

    pub(super) fn parse_type_alias_declaration(&mut self) -> Result<Statement, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'type' contextual keyword
        debug_assert!(self.current_value() == "type");
        self.advance()?;

        let decl = self.parse_type_alias_declaration_body(start)?;
        Ok(Statement::TSTypeAliasDeclaration(decl))
    }

    /// Parse type alias declaration inner - assumes 'type' keyword already consumed
    /// Used by export type X = T when 'type' is consumed to check for { vs identifier
    pub(super) fn parse_type_alias_declaration_inner(&mut self) -> Result<Statement, ParseError> {
        // Start position is before 'type' keyword, but we already consumed it
        // Use current position as a reasonable approximation
        let (start, _) = self.current_pos();
        let decl = self.parse_type_alias_declaration_body(start)?;
        Ok(Statement::TSTypeAliasDeclaration(decl))
    }

    /// Parse type alias body - the part after 'type' keyword
    fn parse_type_alias_declaration_body(
        &mut self,
        start: usize,
    ) -> Result<TSTypeAliasDeclaration, ParseError> {
        // Parse type name (identifier)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected_after("type name", "type"));
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern_identifier();
        self.advance()?;

        let id = Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32));

        // Parse optional type parameters: <T, U>
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Expect '='
        self.expect(&TokenKind::Equals)?;

        // Parse the type
        let type_annotation = self.parse_type()?;
        let type_end = type_annotation.span().end;
        self.semicolon()?;

        Ok(TSTypeAliasDeclaration {
            id,
            type_parameters,
            type_annotation,
            span: Span::new(start as u32, type_end),
        })
    }

    // ============================================================================
    // Interface Declaration
    // ============================================================================

    /// Parse interface declaration: `interface Foo { ... }` or `interface Foo extends Bar { ... }`
    pub(super) fn parse_interface_declaration(&mut self) -> Result<Statement, ParseError> {
        let start = self.current_pos().0;

        // Consume 'interface' contextual keyword
        debug_assert!(self.current_value() == "interface");
        self.advance()?;

        // Parse interface name
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected_after("interface name", "interface"));
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern_identifier();
        self.advance()?;

        let id = Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32));

        // Parse optional type parameters: <T, U>
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Parse optional extends clause
        let extends = if self.check(&TokenKind::Keyword(KeywordKind::Extends)) {
            self.advance()?;
            self.parse_interface_heritage_list()?
        } else {
            Vec::new()
        };

        // Parse interface body
        let body = self.parse_interface_body()?;
        let end = body.span.end;

        Ok(Statement::TSInterfaceDeclaration(TSInterfaceDeclaration {
            id,
            type_parameters,
            extends,
            body,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse interface heritage list: `Foo, Bar<T>`
    pub(in crate::parser) fn parse_interface_heritage_list(
        &mut self,
    ) -> Result<Vec<TSInterfaceHeritage>, ParseError> {
        let mut heritages = Vec::new();

        loop {
            let start = self.current_pos().0;

            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(self.error_expected("interface name in extends clause"));
            }

            let expression = self.parse_entity_name()?;

            // Check for type arguments
            let type_arguments = if self.check(&TokenKind::LessThan) {
                Some(self.parse_type_arguments()?)
            } else {
                None
            };

            let end = type_arguments
                .as_ref()
                .map_or_else(|| expression.span().end, |ta| ta.span.end);

            heritages.push(TSInterfaceHeritage {
                expression,
                type_arguments,
                span: Span::new(start as u32, end),
            });

            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        Ok(heritages)
    }

    /// Parse interface body: `{ members }`
    fn parse_interface_body(&mut self) -> Result<TSInterfaceBody, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::BraceOpen)?;

        let mut body = Vec::new();
        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            body.push(self.parse_type_element()?);
            // Consume separator (; or ,) if present
            if !self.eat(TokenKind::Semicolon) {
                self.eat(TokenKind::Comma);
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(TSInterfaceBody {
            body,
            span: Span::new(start as u32, end as u32),
        })
    }

    /// Parse type element: property, method, call signature, construct signature, or index signature
    fn parse_type_element(&mut self) -> Result<TSTypeElement, ParseError> {
        let start = self.current_pos().0;

        // Check for readonly modifier - only if followed by a property name or bracket
        // Otherwise `readonly` itself is the property name: `readonly: string` or `readonly?: boolean`
        let readonly = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "readonly"
        {
            // Peek ahead to see what follows
            match self.peek_kind() {
                TokenKind::Identifier
                | TokenKind::Keyword(_)
                | TokenKind::BracketOpen
                | TokenKind::ParenOpen
                | TokenKind::LessThan => {
                    // 'readonly' is a modifier - consume it
                    self.advance().ok();
                    true
                }
                _ => {
                    // 'readonly' is a property name - don't consume
                    false
                }
            }
        } else {
            false
        };

        // Check for call signature: `(): T` or `<T>(): T`
        if self.check(&TokenKind::ParenOpen) || self.check(&TokenKind::LessThan) {
            // Parse optional type parameters: <T>
            let type_parameters = if self.check(&TokenKind::LessThan) {
                Some(self.parse_type_parameters()?)
            } else {
                None
            };
            let params = self.parse_parameter_list()?;
            let return_type = if self.check(&TokenKind::Colon) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };
            let end = return_type
                .as_ref()
                .map_or_else(|| self.current_pos().0 as u32, |rt| rt.span.end);
            return Ok(TSTypeElement::CallSignature(TSCallSignatureDeclaration {
                type_parameters,
                params,
                return_type,
                span: Span::new(start as u32, end),
            }));
        }

        // Check for construct signature: `new (): T` or `new <T>(): T`
        // But NOT when `new` is used as a property name: `{ new: string }`
        if self.check(&TokenKind::Keyword(KeywordKind::New)) {
            // Peek ahead to distinguish construct signature from property named 'new'
            // Construct signature: new() or new<T>()
            // Property: new: or new?
            if matches!(self.peek_kind(), TokenKind::ParenOpen | TokenKind::LessThan) {
                self.advance()?;
                // Parse optional type parameters: <T>
                let type_parameters = if self.check(&TokenKind::LessThan) {
                    Some(self.parse_type_parameters()?)
                } else {
                    None
                };
                let params = self.parse_parameter_list()?;
                let return_type = if self.check(&TokenKind::Colon) {
                    Some(self.parse_type_annotation()?)
                } else {
                    None
                };
                let end = return_type
                    .as_ref()
                    .map_or_else(|| self.current_pos().0 as u32, |rt| rt.span.end);
                return Ok(TSTypeElement::ConstructSignature(
                    TSConstructSignatureDeclaration {
                        type_parameters,
                        params,
                        return_type,
                        span: Span::new(start as u32, end),
                    },
                ));
            }
            // Otherwise fall through - 'new' is a property name
        }

        // Check for index signature: `[key: string]: T` vs computed property: `[sym]: T`
        // Index signature has `:` after identifier inside brackets, computed property has `]`
        if self.check(&TokenKind::BracketOpen) {
            // Peek ahead to distinguish index signature from computed property
            // Index signature: [id: type]: T
            // Computed property: [expr]: T
            if self.is_index_signature_start() {
                self.advance()?; // consume '['

                // Parse index parameter
                let (param_start, _) = self.current_pos();
                let param_symbol = self.intern_identifier();
                self.advance()?;

                self.expect(&TokenKind::Colon)?;
                let param_type = self.parse_type()?;

                let param = Identifier {
                    name: param_symbol,
                    optional: false,
                    type_annotation: Some(TSTypeAnnotation {
                        type_annotation: Box::new(param_type.clone()),
                        span: param_type.span(),
                    }),
                    decorators: None,
                    span: Span::new(param_start as u32, param_type.span().end),
                };

                self.expect(&TokenKind::BracketClose)?;
                self.expect(&TokenKind::Colon)?;

                let value_type = self.parse_type()?;
                let end = value_type.span().end;

                return Ok(TSTypeElement::IndexSignature(TSIndexSignature {
                    parameters: vec![param],
                    type_annotation: TSTypeAnnotation {
                        type_annotation: Box::new(value_type),
                        span: Span::new(start as u32, end),
                    },
                    readonly,
                    span: Span::new(start as u32, end),
                }));
            }
            // If not an index signature, fall through to computed property handling below
        }

        // Check for accessor signatures: `get x(): T` or `set x(v: T)`
        // These are contextual keywords - only treated as get/set when followed by a property name
        let accessor_kind = if *self.current_kind() == TokenKind::Identifier {
            let is_get = self.current_value() == "get";
            let is_set = self.current_value() == "set";
            if (is_get || is_set) && self.peek_is_property_name() {
                let kind = if is_get {
                    MethodKind::Get
                } else {
                    MethodKind::Set
                };
                self.advance()?; // consume 'get' or 'set'
                Some(kind)
            } else {
                None
            }
        } else {
            None
        };

        // Parse property/method name
        // Property key: identifier, keyword, string literal, number literal, or computed [expr]
        // Keywords are valid property names in type literals: { class: string }
        let (computed, key) = if self.check(&TokenKind::BracketOpen) {
            self.advance()?;
            let expr = self.parse_expression()?;
            self.expect(&TokenKind::BracketClose)?;
            (true, expr)
        } else if self.current_is_identifier_or_keyword() {
            let (key_start, key_end) = self.current_pos();
            let symbol = self.intern(self.current_property_name());
            self.advance()?;
            (
                false,
                Expression::Identifier(Identifier::simple(
                    symbol,
                    Span::new(key_start as u32, key_end as u32),
                )),
            )
        } else if self.check(&TokenKind::String) {
            // String literal key: {'multi-word': number}
            let (key_start, key_end) = self.current_pos();
            let (content, quote) = self.extract_string_literal();
            self.advance()?;
            (
                false,
                Expression::Literal(Literal {
                    value: LiteralValue::String { content, quote },
                    span: Span::new(key_start as u32, key_end as u32),
                }),
            )
        } else if self.check(&TokenKind::Number) {
            // Number literal key: {0: string, 1: number}
            let (key_start, key_end) = self.current_pos();
            let value = self.current_value().parse::<f64>().unwrap_or(f64::NAN);
            self.advance()?;
            (
                false,
                Expression::Literal(Literal {
                    value: LiteralValue::Number(value),
                    span: Span::new(key_start as u32, key_end as u32),
                }),
            )
        } else {
            return Err(self.error_expected("property name"));
        };

        // Check for optional: ?
        let optional = self.eat(TokenKind::Question);

        // Check for method signature: `()` or `<T>()` or accessor signature
        // Also check for `<` to handle generic methods like `method<T>(x: T): T`
        if accessor_kind.is_some()
            || self.check(&TokenKind::ParenOpen)
            || self.check(&TokenKind::LessThan)
        {
            // Parse type parameters if present: `<T>` or `<T, U extends V>`
            let type_parameters = if self.check(&TokenKind::LessThan) {
                Some(self.parse_type_parameters()?)
            } else {
                None
            };

            let params = self.parse_parameter_list()?;
            let return_type = if self.check(&TokenKind::Colon) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };
            let end = return_type
                .as_ref()
                .map_or_else(|| self.current_pos().0 as u32, |rt| rt.span.end);

            return Ok(TSTypeElement::MethodSignature(TSMethodSignature {
                key,
                computed,
                optional,
                kind: accessor_kind.unwrap_or(MethodKind::Method),
                type_parameters,
                params,
                return_type,
                span: Span::new(start as u32, end),
            }));
        }

        // Property signature
        let type_annotation = if self.check(&TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let end = type_annotation
            .as_ref()
            .map_or_else(|| key.span().end, |ta| ta.span.end);

        Ok(TSTypeElement::PropertySignature(TSPropertySignature {
            key,
            computed,
            optional,
            readonly,
            type_annotation,
            span: Span::new(start as u32, end),
        }))
    }

    // ============================================================================
    // Declare Statement
    // ============================================================================

    /// Parse declare statement: `declare function`, `declare class`, `declare enum`, `declare const enum`, `declare namespace`, `declare global`, `declare var/let/const`
    pub(super) fn parse_declare_statement(&mut self) -> Result<Statement, ParseError> {
        let start = self.current_pos().0;

        // Consume 'declare' contextual keyword
        debug_assert!(self.current_value() == "declare");
        self.advance()?;

        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Function) => self.parse_declare_function(start),
            TokenKind::Keyword(KeywordKind::Class) => self.parse_declare_class(start),
            TokenKind::Keyword(KeywordKind::Enum) => {
                // declare enum
                self.parse_enum_declaration(false, true)
            }
            TokenKind::Keyword(KeywordKind::Const) => {
                // declare const enum OR declare const variable
                if self.peek_kind() == TokenKind::Keyword(KeywordKind::Enum) {
                    self.parse_enum_declaration(true, true)
                } else {
                    // declare const variable: `declare const x: T;`
                    self.parse_declare_variable(start)
                }
            }
            TokenKind::Keyword(KeywordKind::Let) | TokenKind::Keyword(KeywordKind::Var) => {
                // declare let/var variable: `declare var x: T;`
                self.parse_declare_variable(start)
            }
            TokenKind::Identifier
                if self.current_value() == "namespace" || self.current_value() == "module" =>
            {
                // declare namespace/module
                self.parse_module_declaration(true, false)
            }
            TokenKind::Identifier if self.current_value() == "global" => {
                // declare global { }
                self.parse_declare_global(start)
            }
            _ => Err(self.error_expected_after(
                "'function', 'class', 'enum', 'const', 'let', 'var', 'namespace', 'module', or 'global'",
                "declare",
            )),
        }
    }

    /// Parse declare variable: `declare const x: T;`, `declare let x: T;`, `declare var x: T;`
    fn parse_declare_variable(&mut self, start: usize) -> Result<Statement, ParseError> {
        // Parse as a variable declaration but mark as declare
        let mut decl = self.parse_variable_declaration()?;

        // Mark as declare
        if let Statement::VariableDeclaration(ref mut var_decl) = decl {
            var_decl.declare = true;
            var_decl.span = Span::new(start as u32, var_decl.span.end);
        }

        Ok(decl)
    }

    /// Parse declare function: `declare function foo(): void`
    ///
    /// Called from `parse_declare_statement` where `declare` keyword is already consumed.
    fn parse_declare_function(&mut self, start: usize) -> Result<Statement, ParseError> {
        self.parse_declare_function_inner(start, true)
    }

    /// Parse function declaration in ambient context (inside `declare namespace`)
    ///
    /// These functions don't have bodies: `function foo(x: number): void;`
    pub(super) fn parse_ambient_function_declaration(&mut self) -> Result<Statement, ParseError> {
        let start = self.current_pos().0;
        self.parse_declare_function_inner(start, false)
    }

    /// Inner helper for parsing declare/ambient functions
    ///
    /// - `start`: span start position
    /// - `print_declare`: whether to print `declare` keyword (true for top-level, false inside `declare namespace`)
    fn parse_declare_function_inner(
        &mut self,
        start: usize,
        print_declare: bool,
    ) -> Result<Statement, ParseError> {
        // Consume 'function' keyword
        self.advance()?;

        // Parse function name
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected("function name"));
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern_identifier();
        self.advance()?;

        let id = Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32));

        // Parse optional type parameters: <T, U>
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Parse parameters
        let params = self.parse_parameter_list()?;

        // Parse return type (may be a type predicate)
        let return_type = if self.check(&TokenKind::Colon) {
            Some(self.parse_return_type_annotation()?)
        } else {
            None
        };

        let end = return_type
            .as_ref()
            .map_or_else(|| self.current_pos().0 as u32, |rt| rt.span.end);

        self.semicolon()?;

        Ok(Statement::TSDeclareFunction(TSDeclareFunction {
            id,
            type_parameters,
            params,
            return_type,
            declare: print_declare,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse return type annotation, handling type predicates (`x is T`, `asserts x is T`)
    ///
    /// This expects the colon to NOT be consumed yet.
    pub(in crate::parser) fn parse_return_type_annotation(
        &mut self,
    ) -> Result<TSTypeAnnotation, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::Colon)?;
        self.parse_return_type_inner(start as u32)
    }

    /// Parse return type after colon/arrow, handling type predicates
    ///
    /// Called after the `:` or `=>` has been consumed.
    pub(in crate::parser) fn parse_return_type_inner(
        &mut self,
        start: u32,
    ) -> Result<TSTypeAnnotation, ParseError> {
        // Check for 'asserts' keyword
        let asserts = self.eat_contextual_keyword("asserts");

        // Check if this might be a type predicate: `identifier is type`
        if matches!(self.current_kind(), TokenKind::Identifier)
            && self.peek_is_contextual_keyword("is")
        {
            // Parse parameter name
            let (id_start, id_end) = self.current_pos();
            let param_name_str = self.current_value().to_string();
            let param_symbol = self.intern(&param_name_str);
            self.advance()?;

            let parameter_name =
                Identifier::simple(param_symbol, Span::new(id_start as u32, id_end as u32));

            // Consume 'is' keyword
            self.advance()?;

            // Parse the type
            let type_node = self.parse_type()?;
            let end = type_node.span().end;

            let predicate = TSTypePredicate {
                parameter_name,
                type_annotation: Some(Box::new(type_node)),
                asserts,
                span: Span::new(start, end),
            };

            Ok(TSTypeAnnotation {
                type_annotation: Box::new(TSType::TypePredicate(predicate)),
                span: Span::new(start, end),
            })
        } else if asserts {
            // `asserts x` without `is T` - just the parameter name
            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(self.error_expected_after("identifier", "asserts"));
            }

            let (id_start, id_end) = self.current_pos();
            let param_name_str = self.current_value().to_string();
            let param_symbol = self.intern(&param_name_str);
            self.advance()?;

            let parameter_name =
                Identifier::simple(param_symbol, Span::new(id_start as u32, id_end as u32));

            let predicate = TSTypePredicate {
                parameter_name,
                type_annotation: None,
                asserts: true,
                span: Span::new(start, id_end as u32),
            };

            Ok(TSTypeAnnotation {
                type_annotation: Box::new(TSType::TypePredicate(predicate)),
                span: Span::new(start, id_end as u32),
            })
        } else {
            // Regular type annotation
            let type_node = self.parse_type()?;
            let end = type_node.span().end;

            Ok(TSTypeAnnotation {
                type_annotation: Box::new(type_node),
                span: Span::new(start, end),
            })
        }
    }

    /// Parse declare class: `declare class Foo { ... }`
    fn parse_declare_class(&mut self, start: usize) -> Result<Statement, ParseError> {
        // Consume 'class' keyword
        self.advance()?;

        // Parse class name
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected_after("class name", "declare class"));
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern_identifier();
        self.advance()?;

        let id = Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32));

        // Parse type parameters if present (e.g., <T> in `class Foo<T>`)
        let type_parameters = if self.check(&TokenKind::LessThan) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Parse optional extends clause (identifier with optional type arguments)
        let (super_class, super_type_parameters) =
            if self.check(&TokenKind::Keyword(KeywordKind::Extends)) {
                self.advance()?;

                // Parse the superclass identifier
                if !matches!(self.current_kind(), TokenKind::Identifier) {
                    return Err(self.error_expected_after("class name", "extends"));
                }

                let (id_start, id_end) = self.current_pos();
                let super_symbol = self.intern_identifier();
                self.advance()?;

                let super_id = Expression::Identifier(Identifier::simple(
                    super_symbol,
                    Span::new(id_start as u32, id_end as u32),
                ));

                // Parse optional type arguments: <T, U>
                let type_args = if self.check(&TokenKind::LessThan) {
                    Some(self.parse_type_arguments()?)
                } else {
                    None
                };

                (Some(Box::new(super_id)), type_args)
            } else {
                (None, None)
            };

        // Parse optional implements clause
        let implements = if self.eat_contextual_keyword("implements") {
            self.parse_interface_heritage_list()?
        } else {
            Vec::new()
        };

        // Parse class body (for declare class, members are signatures only)
        let body = self.parse_declare_class_body()?;
        let end = body.span.end;

        Ok(Statement::ClassDeclaration(ClassDeclaration {
            decorators: None,
            id: Some(id),
            super_class,
            super_type_parameters,
            implements,
            body,
            declare: true,
            r#abstract: false,
            type_parameters,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse declare class body: `{ constructor(); method(): T; prop: T; }`
    fn parse_declare_class_body(&mut self) -> Result<ClassBody, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::BraceOpen)?;

        let mut body = Vec::new();
        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            body.push(self.parse_declare_class_member()?);
            // Consume separator if present
            self.eat(TokenKind::Semicolon);
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(ClassBody {
            body,
            span: Span::new(start as u32, end as u32),
        })
    }

    /// Parse declare class member (property or method signature)
    fn parse_declare_class_member(&mut self) -> Result<ClassMember, ParseError> {
        let start = self.current_pos().0;

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

        // Handle modifiers: static, readonly
        let is_static = self.eat_contextual_keyword("static");
        let readonly = self.eat_contextual_keyword("readonly");

        // Parse member name or constructor
        let (computed, key, is_constructor) = if self.check(&TokenKind::BracketOpen) {
            self.advance()?;
            let expr = self.parse_expression()?;
            self.expect(&TokenKind::BracketClose)?;
            (true, expr, false)
        } else if matches!(self.current_kind(), TokenKind::Identifier) {
            let name = self.current_value().to_string();
            let (key_start, key_end) = self.current_pos();
            let symbol = self.intern(&name);
            self.advance()?;
            (
                false,
                Expression::Identifier(Identifier::simple(
                    symbol,
                    Span::new(key_start as u32, key_end as u32),
                )),
                name == "constructor",
            )
        } else {
            return Err(self.error_expected("class member name"));
        };

        // Check if it's a method (has parentheses)
        if self.check(&TokenKind::ParenOpen) {
            // Capture paren position before parsing params (for comment detection)
            let (params_start, _) = self.current_pos();
            let params = self.parse_parameter_list()?;
            let return_type = if self.check(&TokenKind::Colon) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            let end = return_type
                .as_ref()
                .map_or_else(|| self.current_pos().0 as u32, |rt| rt.span.end);

            let kind = if is_constructor {
                MethodKind::Constructor
            } else {
                MethodKind::Method
            };

            // Create a FunctionExpression with empty body for declare class methods
            let value = FunctionExpression {
                id: None,
                type_parameters: None, // TODO: parse type parameters for declare class methods
                params,
                return_type,
                body: BlockStatement {
                    body: Vec::new(),
                    span: Span::new(end, end),
                },
                generator: false,
                r#async: false,
                params_start: params_start as u32,
                span: Span::new(start as u32, end),
            };

            Ok(ClassMember::MethodDefinition(MethodDefinition {
                decorators: None,
                key,
                value,
                kind,
                accessibility,
                is_static,
                r#override: false,
                r#abstract: false,
                computed,
                span: Span::new(start as u32, end),
            }))
        } else {
            // Property declaration

            // Check for optional marker (`?`) or definite assignment assertion (`!`)
            let modifier = if self.eat(TokenKind::Question) {
                PropertyModifier::Optional
            } else if self.eat(TokenKind::Bang) {
                PropertyModifier::Definite
            } else {
                PropertyModifier::None
            };

            let type_annotation = if self.check(&TokenKind::Colon) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            let end = type_annotation
                .as_ref()
                .map_or_else(|| key.span().end, |ta| ta.span.end);

            Ok(ClassMember::PropertyDefinition(PropertyDefinition {
                decorators: None,
                key,
                type_annotation,
                value: None,
                accessibility,
                is_static,
                r#abstract: false,
                readonly,
                computed,
                accessor: false,
                modifier,
                span: Span::new(start as u32, end),
            }))
        }
    }

    /// Parse type parameters: `<T, U extends V = W>`
    pub(in crate::parser) fn parse_type_parameters(
        &mut self,
    ) -> Result<TSTypeParameterDeclaration, ParseError> {
        let start = self.current_pos().0 as u32;
        self.expect(&TokenKind::LessThan)?;

        let mut params = Vec::new();
        loop {
            let param = self.parse_type_parameter()?;
            params.push(param);

            if !self.eat(TokenKind::Comma) {
                break;
            }
            // Handle trailing comma
            if self.check_greater_than_in_type() {
                break;
            }
        }

        let end = self.current_pos().1 as u32;
        self.expect_greater_than_in_type()?;

        Ok(TSTypeParameterDeclaration {
            params,
            span: Span::new(start, end),
        })
    }

    /// Parse a single type parameter: `T`, `T extends U`, or `T extends U = V`
    /// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
    fn parse_type_parameter(&mut self) -> Result<TSTypeParameter, ParseError> {
        let start = self.current_pos().0 as u32;

        // Parse optional modifiers: const, in, out
        let mut is_const = false;
        let mut is_in = false;
        let mut is_out = false;

        // Check for `const` modifier (TS 5.0)
        if self.check(&TokenKind::Keyword(KeywordKind::Const)) {
            is_const = true;
            self.advance()?;
        }

        // Check for `in` modifier (variance, TS 4.7)
        if self.check(&TokenKind::Keyword(KeywordKind::In)) {
            is_in = true;
            self.advance()?;
        }

        // Check for `out` modifier (variance, TS 4.7)
        // Note: `out` is a contextual keyword, check as identifier
        if matches!(self.current_kind(), TokenKind::Identifier) {
            let text = self.current_value();
            if text == "out" {
                is_out = true;
                self.advance()?;
            }
        }

        let (id_start, id_end) = self.current_pos();

        // Parse the type parameter name (must be an identifier)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected_found_at("type parameter name", id_start));
        }
        let symbol = self.intern_identifier();
        self.advance()?;
        let name = Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32));

        // Parse optional constraint: `extends U`
        let constraint = if self.check(&TokenKind::Keyword(KeywordKind::Extends)) {
            self.advance()?;
            Some(Box::new(self.parse_type()?))
        } else {
            None
        };

        // Parse optional default: `= V`
        let default = if self.eat(TokenKind::Equals) {
            Some(Box::new(self.parse_type()?))
        } else {
            None
        };

        let end = self.current_pos().0 as u32;

        Ok(TSTypeParameter {
            name,
            constraint,
            default,
            is_const,
            is_in,
            is_out,
            span: Span::new(start, end),
        })
    }

    /// Parse type argument instantiation: `<T, U>` (for instantiation expressions like `f<T>`)
    ///
    /// Unlike parse_type_parameters, this parses actual types, not type parameter declarations.
    /// Used for TSInstantiationExpression and other type argument contexts.
    pub(in crate::parser) fn parse_type_parameter_instantiation(
        &mut self,
    ) -> Result<TSTypeParameterInstantiation, ParseError> {
        let start = self.current_pos().0 as u32;
        self.expect(&TokenKind::LessThan)?;

        let mut params = Vec::new();
        loop {
            let ts_type = self.parse_type()?;
            params.push(ts_type);

            if !self.eat(TokenKind::Comma) {
                break;
            }
            // Handle trailing comma
            if self.check_greater_than_in_type() {
                break;
            }
        }

        let end = self.current_pos().1 as u32;
        self.expect_greater_than_in_type()?;

        Ok(TSTypeParameterInstantiation {
            params,
            span: Span::new(start, end),
        })
    }

    // ============================================================================
    // Enum Declaration
    // ============================================================================

    /// Parse enum declaration: `enum Foo { A, B }`, `const enum Foo { A = 1 }`, etc.
    ///
    /// This handles all enum variants:
    /// - Regular: `enum Foo { A, B }`
    /// - Const: `const enum Foo { A, B }`
    /// - Declare: `declare enum Foo { A, B }`
    /// - Declare const: `declare const enum Foo { A, B }`
    pub(super) fn parse_enum_declaration(
        &mut self,
        is_const: bool,
        is_declare: bool,
    ) -> Result<Statement, ParseError> {
        let start = self.current_pos().0;

        // Consume 'const' if present
        if is_const {
            self.expect(&TokenKind::Keyword(KeywordKind::Const))?;
        }

        // Consume 'enum' keyword
        self.expect(&TokenKind::Keyword(KeywordKind::Enum))?;

        // Parse enum name
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected_after("enum name", "enum"));
        }

        let (id_start, id_end) = self.current_pos();
        let symbol = self.intern_identifier();
        self.advance()?;

        let id = Identifier::simple(symbol, Span::new(id_start as u32, id_end as u32));

        // Parse enum body: { members }
        self.expect(&TokenKind::BraceOpen)?;

        let mut members = Vec::new();
        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            members.push(self.parse_enum_member()?);

            // Consume comma if present (trailing comma is allowed)
            if !self.eat(TokenKind::Comma) {
                // No comma, break if not at closing brace
                if !matches!(self.current_kind(), TokenKind::BraceClose) {
                    return Err(self.error_expected("',' or '}' in enum"));
                }
            }
        }

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(Statement::TSEnumDeclaration(TSEnumDeclaration {
            id,
            members,
            r#const: is_const,
            declare: is_declare,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse a single enum member: `A`, `A = 1`, `A = "value"`, `"computed" = 1`
    fn parse_enum_member(&mut self) -> Result<TSEnumMember, ParseError> {
        let start = self.current_pos().0;

        // Parse member id: can be identifier or string literal
        let id = match self.current_kind() {
            TokenKind::Identifier => {
                let (id_start, id_end) = self.current_pos();
                let symbol = self.intern_identifier();
                self.advance()?;
                TSEnumMemberId::Identifier(Identifier::simple(
                    symbol,
                    Span::new(id_start as u32, id_end as u32),
                ))
            }
            TokenKind::String => TSEnumMemberId::String(self.parse_string_literal()?),
            _ => {
                return Err(self.error_expected("enum member name (identifier or string)"));
            }
        };

        // Parse optional initializer: = value
        // Use assignment expression (not full expression) to stop at commas
        let (initializer, end) = if self.eat(TokenKind::Equals) {
            let expr = self.parse_assignment_expression()?;
            let end = expr.span().end;
            (Some(expr), end)
        } else {
            let id_end = match &id {
                TSEnumMemberId::Identifier(i) => i.span.end,
                TSEnumMemberId::String(l) => l.span.end,
            };
            (None, id_end)
        };

        Ok(TSEnumMember {
            id,
            initializer,
            span: Span::new(start as u32, end),
        })
    }

    /// Parse a namespace/module declaration: `namespace Utils { ... }` or `module Utils { ... }`
    ///
    /// Handles:
    /// - `namespace Name { statements }`
    /// - `namespace Outer.Inner { statements }` (nested)
    /// - `declare namespace Name { statements }` (ambient)
    /// - `declare module 'name' { statements }` (ambient module augmentation)
    /// - `declare module 'name';` (shorthand ambient module)
    /// - `module Name { statements }` (old syntax)
    pub(super) fn parse_module_declaration(
        &mut self,
        declare: bool,
        global: bool,
    ) -> Result<Statement, ParseError> {
        let start = self.current_pos().0;

        // Capture which keyword was used: 'namespace' or 'module'
        debug_assert!(
            matches!(self.current_kind(), TokenKind::Identifier)
                && (self.current_value() == "namespace" || self.current_value() == "module")
        );
        let kind = if self.current_value() == "module" {
            TSModuleDeclarationKind::Module
        } else {
            TSModuleDeclarationKind::Namespace
        };
        self.advance()?;

        // Parse module name - can be identifier or string literal (for ambient modules)
        let id = if matches!(self.current_kind(), TokenKind::String) {
            // Ambient module with string literal name: `declare module 'name' { }`
            let lit = self.parse_string_literal()?;
            TSModuleName::Literal(lit)
        } else if matches!(self.current_kind(), TokenKind::Identifier) {
            let (id_start, id_end) = self.current_pos();
            let name = self.intern_identifier();
            let ident = Identifier::simple(name, Span::new(id_start as u32, id_end as u32));
            self.advance()?;

            // Check for nested namespace: `namespace Outer.Inner { }`
            if matches!(self.current_kind(), TokenKind::Dot) {
                return self.parse_nested_module_declaration(start as u32, ident, declare, kind);
            }

            TSModuleName::Identifier(ident)
        } else {
            return Err(self.error_expected("identifier or string literal for module name"));
        };

        // Parse body or semicolon for shorthand
        let (body, end) = if matches!(self.current_kind(), TokenKind::Semicolon) {
            // Shorthand ambient module: `declare module 'name';`
            let end = self.current_pos().1 as u32;
            self.advance()?; // consume ';'
            (None, end)
        } else {
            // Full body: `{ statements }`
            let block = self.parse_module_block(declare)?;
            let end = match &block {
                TSModuleDeclarationBody::TSModuleBlock(b) => b.span.end,
                TSModuleDeclarationBody::TSModuleDeclaration(n) => n.span.end,
            };
            (Some(block), end)
        };

        Ok(Statement::TSModuleDeclaration(TSModuleDeclaration {
            id,
            body,
            declare,
            kind,
            global,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse declare global: `declare global { ... }`
    fn parse_declare_global(&mut self, start: usize) -> Result<Statement, ParseError> {
        // Consume 'global' keyword
        debug_assert!(self.current_value() == "global");
        let (global_start, global_end) = self.current_pos();
        let name = self.intern_identifier();
        self.advance()?;

        let id = TSModuleName::Identifier(Identifier::simple(
            name,
            Span::new(global_start as u32, global_end as u32),
        ));

        // Parse body
        let block = self.parse_module_block(true)?;
        let end = match &block {
            TSModuleDeclarationBody::TSModuleBlock(b) => b.span.end,
            TSModuleDeclarationBody::TSModuleDeclaration(n) => n.span.end,
        };

        Ok(Statement::TSModuleDeclaration(TSModuleDeclaration {
            id,
            body: Some(block),
            declare: true,
            kind: TSModuleDeclarationKind::Module, // TypeScript uses module kind for global
            global: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse nested module declaration: `namespace Outer.Inner { }`
    fn parse_nested_module_declaration(
        &mut self,
        start: u32,
        outer_id: Identifier,
        declare: bool,
        kind: TSModuleDeclarationKind,
    ) -> Result<Statement, ParseError> {
        self.advance()?; // consume '.'

        // Parse the inner declaration recursively
        let nested_start = self.current_pos().0;
        let nested = self.parse_module_declaration_inner(nested_start as u32, false, kind)?;
        let body = TSModuleDeclarationBody::TSModuleDeclaration(Box::new(nested));
        let end = match &body {
            TSModuleDeclarationBody::TSModuleBlock(b) => b.span.end,
            TSModuleDeclarationBody::TSModuleDeclaration(n) => n.span.end,
        };

        Ok(Statement::TSModuleDeclaration(TSModuleDeclaration {
            id: TSModuleName::Identifier(outer_id),
            body: Some(body),
            declare,
            kind,
            global: false,
            span: Span::new(start, end),
        }))
    }

    /// Inner helper for parsing nested module declarations
    fn parse_module_declaration_inner(
        &mut self,
        start: u32,
        declare: bool,
        kind: TSModuleDeclarationKind,
    ) -> Result<TSModuleDeclaration, ParseError> {
        // Parse namespace name (identifier for nested parts)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected("identifier for namespace name"));
        }
        let (id_start, id_end) = self.current_pos();
        let name = self.intern_identifier();
        let id = Identifier::simple(name, Span::new(id_start as u32, id_end as u32));
        self.advance()?;

        // Check for nested namespace: `namespace Outer.Inner { }`
        let body = if matches!(self.current_kind(), TokenKind::Dot) {
            self.advance()?; // consume '.'

            // Parse nested declaration (recursively)
            // Nested parts inherit the same kind (namespace vs module)
            let nested_start = self.current_pos().0;
            let nested = self.parse_module_declaration_inner(nested_start as u32, false, kind)?;
            TSModuleDeclarationBody::TSModuleDeclaration(Box::new(nested))
        } else {
            // Parse block body: `{ statements }`
            // For `declare namespace`, we're in ambient context
            self.parse_module_block(declare)?
        };

        // Calculate end position based on body
        let end = match &body {
            TSModuleDeclarationBody::TSModuleBlock(block) => block.span.end,
            TSModuleDeclarationBody::TSModuleDeclaration(nested) => nested.span.end,
        };

        Ok(TSModuleDeclaration {
            id: TSModuleName::Identifier(id),
            body: Some(body),
            declare,
            kind,
            global: false,
            span: Span::new(start, end),
        })
    }

    /// Parse a module block: `{ statements }`
    ///
    /// If `is_ambient` is true (for `declare namespace`), functions inside
    /// don't have bodies and are parsed as `TSDeclareFunction`.
    fn parse_module_block(
        &mut self,
        is_ambient: bool,
    ) -> Result<TSModuleDeclarationBody, ParseError> {
        // Expect opening brace
        if !matches!(self.current_kind(), TokenKind::BraceOpen) {
            return Err(self.error_expected("'{' to open namespace body"));
        }
        let (block_start, _) = self.current_pos();
        self.advance()?; // consume '{'

        // Set ambient context for declare namespace
        let saved_ambient = self.in_ambient_context;
        if is_ambient {
            self.in_ambient_context = true;
        }

        // Parse statements until '}'
        let mut body = Vec::new();
        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let stmt = self.parse_statement()?;
            body.push(stmt);
        }

        // Restore ambient context
        self.in_ambient_context = saved_ambient;

        // Expect closing brace
        if !matches!(self.current_kind(), TokenKind::BraceClose) {
            return Err(self.error_expected("'}' to close namespace body"));
        }
        let (_, block_end) = self.current_pos();
        self.advance()?; // consume '}'

        Ok(TSModuleDeclarationBody::TSModuleBlock(TSModuleBlock {
            body,
            span: Span::new(block_start as u32, block_end as u32),
        }))
    }
}
