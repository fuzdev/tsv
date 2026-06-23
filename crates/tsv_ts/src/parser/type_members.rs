// TypeScript type-literal / interface-body member grammar: the run of type
// elements shared by type literals (`{ … }`) and interface bodies — property,
// method, call/construct/index signatures. Type *expression* syntax lives in
// `parser/types.rs`.

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use string_interner::DefaultSymbol;
use tsv_lang::{ParseError, Span};

use super::Parser;

impl<'a> Parser<'a> {
    /// Parse the body of an index signature after `[identifier` has been consumed.
    /// Expects current token to be `:` (the colon after the parameter name).
    /// Returns the completed `TSTypeElement::IndexSignature`.
    fn parse_index_signature_body(
        &mut self,
        sig_start: usize,
        param_symbol: DefaultSymbol,
        id_start: usize,
        readonly: bool,
    ) -> Result<TSTypeElement, ParseError> {
        let param_colon_start = self.current_pos().0;
        self.expect(&TokenKind::Colon)?;
        let param_type = self.parse_type()?;
        let param_type_end = param_type.span().end;

        let param = Identifier {
            name: param_symbol,
            optional: false,
            type_annotation: Some(TSTypeAnnotation {
                type_annotation: Box::new(param_type),
                span: Span::new(param_colon_start as u32, param_type_end),
            }),
            decorators: None,
            span: Span::new(id_start as u32, param_type_end),
        };

        self.expect(&TokenKind::BracketClose)?;
        let value_colon_start = self.current_pos().0;
        self.expect(&TokenKind::Colon)?;

        let value_type = self.parse_type()?;
        let member_end = value_type.span().end;

        Ok(TSTypeElement::IndexSignature(TSIndexSignature {
            parameters: vec![param],
            type_annotation: TSTypeAnnotation {
                type_annotation: Box::new(value_type),
                span: Span::new(value_colon_start as u32, member_end),
            },
            is_static: false,
            readonly,
            span: Span::new(sig_start as u32, member_end),
        }))
    }

    /// Parse a run of type members up to (but not consuming) the closing `}`.
    /// Each member's span is extended over a trailing `;`/`,` separator when one
    /// is present. Shared by type literals (`{ … }`) and interface bodies.
    pub(in crate::parser) fn parse_type_members(
        &mut self,
    ) -> Result<Vec<TSTypeElement>, ParseError> {
        let mut members = Vec::new();
        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let mut element = self.parse_type_element()?;
            // Consume separator (; or ,) if present, extending the element span to include it
            if self.eat(TokenKind::Semicolon) || self.eat(TokenKind::Comma) {
                element.extend_span_to(self.prev_token_end() as u32);
            }
            members.push(element);
        }
        Ok(members)
    }

    /// Parse type element: property, method, call signature, construct signature, or index signature
    pub(in crate::parser) fn parse_type_element(&mut self) -> Result<TSTypeElement, ParseError> {
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
            let type_parameters = self.parse_optional_type_parameters()?;
            let params = self.parse_parameter_list()?;
            let (return_type, end) = self.parse_signature_return_type(true)?;
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
            // Construct signature: new() or new<T>() — skipping comments, so
            // `new /* c */ (): T` stays a construct signature
            // Property: new: or new?
            if matches!(self.peek_kind(), TokenKind::ParenOpen | TokenKind::LessThan) {
                self.advance()?;
                // Parse optional type parameters: <T>
                let type_parameters = self.parse_optional_type_parameters()?;
                let params = self.parse_parameter_list()?;
                let (return_type, end) = self.parse_signature_return_type(false)?;
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

                let (param_start, _) = self.current_pos();
                let param_symbol = self.intern_identifier();
                self.advance()?;

                return self.parse_index_signature_body(start, param_symbol, param_start, readonly);
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
            let type_parameters = self.parse_optional_type_parameters()?;

            let params = self.parse_parameter_list()?;
            let (return_type, end) = self.parse_signature_return_type(true)?;

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
        let type_annotation = self.parse_optional_type_annotation()?;

        // Without a type annotation, prev_token_end() extends past the key when an
        // optional `?` was consumed (`interface I { a? }`), matching acorn's span.
        let end = type_annotation.as_ref().map_or_else(
            || key.span().end.max(self.prev_token_end() as u32),
            |ta| ta.span.end,
        );

        Ok(TSTypeElement::PropertySignature(TSPropertySignature {
            key,
            computed,
            optional,
            readonly,
            type_annotation,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse an optional `: ReturnType` for a signature member, returning the
    /// return type (when a `:` is present) and the member's end offset (the
    /// return type's end, or the current position when there's no annotation).
    ///
    /// Call and method signatures allow type predicates (`x is T`, `asserts x`);
    /// construct signatures pass `allow_type_predicate = false` since `new` can't
    /// assert.
    fn parse_signature_return_type(
        &mut self,
        allow_type_predicate: bool,
    ) -> Result<(Option<TSTypeAnnotation>, u32), ParseError> {
        let return_type = if self.check(&TokenKind::Colon) {
            Some(if allow_type_predicate {
                self.parse_return_type_annotation()?
            } else {
                self.parse_type_annotation()?
            })
        } else {
            None
        };
        // Without a return type the signature ends at the params' `)` — the next
        // token's start would overshoot past trailing comments or onto the next
        // line when no separator follows (`bar(/* c */) // c2`)
        let end = return_type
            .as_ref()
            .map_or_else(|| self.prev_token_end() as u32, |rt| rt.span.end);
        Ok((return_type, end))
    }
}
