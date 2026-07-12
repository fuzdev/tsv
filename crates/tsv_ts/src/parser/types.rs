// TypeScript type-syntax parsing: type annotations and type expressions
// (unions, intersections, object/mapped/tuple/template-literal types, function
// and constructor types, type queries, import types) plus type parameters. The
// type-element grammar shared by type literals and interface bodies lives in
// `parser/type_members.rs`; declarations that *introduce* types (alias,
// interface, enum, namespace, declare) live in `statement/type_declarations.rs`.

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::Parser;
use super::expression_lookahead::{paren_pattern_then_type_operator, scan_parens_then_arrow};
use super::scan::{
    is_identifier_continue, is_identifier_start, skip_identifier, skip_whitespace_and_comments,
};

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Parse a `: Type` annotation when the next token is a `:`, else `None` —
    /// the optional-annotation guard shared by variable declarations, class
    /// properties, and type members. Sites whose missing `:` is an error (e.g.
    /// an index-signature parameter) keep their own inline `else`.
    pub(in crate::parser) fn parse_optional_type_annotation(
        &mut self,
    ) -> Result<Option<TSTypeAnnotation<'arena>>, ParseError> {
        if self.check(&TokenKind::Colon) {
            Ok(Some(self.parse_type_annotation()?))
        } else {
            Ok(None)
        }
    }

    pub(in crate::parser) fn parse_type_annotation(
        &mut self,
    ) -> Result<TSTypeAnnotation<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::Colon)?;

        let type_node = self.parse_type()?;
        let end = type_node.span().end;

        Ok(TSTypeAnnotation {
            type_annotation: self.alloc(type_node),
            span: Span::new(start as u32, end),
        })
    }

    /// Parse a complete type expression (handles unions and conditional types at
    /// top level). This is the full-type entry, where function/constructor types
    /// are grammatically allowed — clear both type-context restrictions
    /// (`with_full_type_context`) so nested full-type positions (type arguments,
    /// tuple members, object-type members, conditional branches, parenthesized
    /// inners) parse them greedily even when reached from a constituent/operand
    /// parse. The postfix-`[` ASI rule (a newline before `[` ends the type — see
    /// `parse_array_type`) applies uniformly, so the `as`/`satisfies` expression
    /// context needs no separate entry.
    pub(in crate::parser) fn parse_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        debug_assert!(self.pending_conditional_extends.is_none());
        self.with_full_type_context(|p| {
            let start = p.current_pos().0;
            let check_type = p.parse_union_type()?;

            // A constrained infer inside `check_type` hit `?` directly after its
            // constraint at this allow-conditional position: the constraint
            // re-binds as this conditional's extends clause and the current token
            // is the `?` (see `pending_conditional_extends`). Otherwise check for
            // `T extends U ? V : W`. The `extends` must not be preceded by a line
            // terminator (TS `CheckType [no LineTerminator here] extends
            // ExtendsType`): a newline before it ends the type, leaving `extends`
            // a stray token (acorn-typescript's `hasPrecedingLineBreak` guard).
            // Same restricted-production rule as the arrow `=>` (see
            // `expect_arrow`).
            let extends_type = if let Some(rebound) = p.pending_conditional_extends.take() {
                debug_assert!(p.check(&TokenKind::Question));
                Some(rebound)
            } else if p.check(&TokenKind::Keyword(KeywordKind::Extends)) && !p.had_line_terminator {
                p.advance()?; // consume 'extends'

                // The extends clause cannot itself be a conditional — a nested
                // `? T : F` binds to this conditional; parens re-enable it.
                Some(p.with_conditional_type_disallowed(true, Self::parse_union_type)?)
            } else {
                None
            };

            if let Some(extends_type) = extends_type {
                // Expect '?'
                p.expect(&TokenKind::Question)?;

                let true_type = p.parse_type()?;

                // Expect ':'
                p.expect(&TokenKind::Colon)?;

                let false_type = p.parse_type()?;
                let end = false_type.span().end;

                Ok(TSType::Conditional(TSConditionalType {
                    check_type: p.alloc(check_type),
                    extends_type: p.alloc(extends_type),
                    true_type: p.alloc(true_type),
                    false_type: p.alloc(false_type),
                    span: Span::new(start as u32, end),
                }))
            } else {
                Ok(check_type)
            }
        })
    }

    /// Parse union type: `A | B | C` or `| A | B | C`
    fn parse_union_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;

        // Handle leading pipe: `| A | B`
        let has_leading_pipe = self.check(&TokenKind::Pipe);
        if has_leading_pipe {
            self.advance()?; // consume leading '|'
        }

        // A union constituent is not a full-type position: function/constructor
        // types are disallowed there (`fn_type_disallowed`), so a constituent's
        // `(` is a parenthesized type. Without a leading `|` the first parse is
        // still the full-type position itself (a function type here is greedy:
        // `([a]) => x | B` is a function type returning `x | B`).
        let first = if has_leading_pipe {
            self.with_fn_type_disallowed(true, Self::parse_intersection_type)?
        } else {
            self.parse_intersection_type()?
        };

        if !has_leading_pipe && !self.check(&TokenKind::Pipe) {
            return Ok(first);
        }

        let mut types = self.bvec();
        types.push(first);
        while self.eat(TokenKind::Pipe) {
            types.push(self.with_fn_type_disallowed(true, Self::parse_intersection_type)?);
        }

        let end = types.last().map_or_else(|| start as u32, |t| t.span().end);
        Ok(TSType::Union(TSUnionType {
            types: types.into_bump_slice(),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse intersection type: `A & B & C` or `& A & B & C`
    fn parse_intersection_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;

        // Handle leading ampersand: `& A & B`
        let has_leading_amp = self.check(&TokenKind::Ampersand);
        if has_leading_amp {
            self.advance()?; // consume leading '&'
        }

        // An intersection constituent is not a full-type position — see the
        // union counterpart above. The no-leading-`&` first parse inherits the
        // caller's position (full-type, or a union constituent's).
        let first = if has_leading_amp {
            self.with_fn_type_disallowed(true, Self::parse_array_type)?
        } else {
            self.parse_array_type()?
        };

        if !has_leading_amp && !self.check(&TokenKind::Ampersand) {
            return Ok(first);
        }

        let mut types = self.bvec();
        types.push(first);
        while self.eat(TokenKind::Ampersand) {
            types.push(self.with_fn_type_disallowed(true, Self::parse_array_type)?);
        }

        let end = types.last().map_or_else(|| start as u32, |t| t.span().end);
        Ok(TSType::Intersection(TSIntersectionType {
            types: types.into_bump_slice(),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse array type suffix `T[]` or indexed access type `T[K]`.
    ///
    /// The postfix `[` is a `[no LineTerminator here]` position (acorn's
    /// `tsParseArrayTypeOrHigher`: `while (!hasPrecedingLineBreak() && eat('['))`):
    /// a newline before `[` ends the type, so the `[` starts a new statement (ASI
    /// inserts the `;`), never an array/indexed-access suffix. `T⏎[K]` is thus `T`
    /// then a fresh `[K]`, not `T[K]`. Checked per bracket in the loop condition —
    /// every context, not just the `as`/`satisfies` expression position.
    fn parse_array_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;
        let mut result = self.parse_primary_type()?;

        // Array type suffix [] or indexed access T[K], stopping at a newline-`[`.
        while !self.had_line_terminator && self.check(&TokenKind::BracketOpen) {
            if matches!(self.peek_kind(), TokenKind::BracketClose) {
                // Array type: T[]
                self.advance()?; // consume '['
                let (_, arr_end) = self.current_pos();
                self.expect(&TokenKind::BracketClose)?;

                result = TSType::Array(TSArrayType {
                    element_type: self.alloc(result),
                    span: Span::new(start as u32, arr_end as u32),
                });
            } else {
                // Indexed access type: T[K]
                self.advance()?; // consume '['
                let index_type = self.parse_type()?;
                let (_, end) = self.current_pos();
                self.expect(&TokenKind::BracketClose)?;

                result = TSType::IndexedAccess(TSIndexedAccessType {
                    object_type: self.alloc(result),
                    index_type: self.alloc(index_type),
                    span: Span::new(start as u32, end as u32),
                });
            }
        }

        Ok(result)
    }

    /// Parse primary type (highest precedence)
    fn parse_primary_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        let (start, end) = self.current_pos();
        let span = Span::new(start as u32, end as u32);

        // Keyword types: string, number, boolean, true, false, etc.
        if let TokenKind::Keyword(kw) = self.current_kind()
            && let Some(ts_kind) = TSKeywordKind::from_lexer_keyword(*kw)
        {
            // A *contextual* type keyword (`string`, `number`, `any`, `undefined`, …)
            // immediately followed by `.` is the HEAD of a qualified type name
            // (`string.X`, `number.A.B`), not the primitive keyword — tsc, prettier,
            // and acorn all read it as a `TSTypeReference` / `TSQualifiedName`. Gated on
            // `can_be_binding_name` so only the non-reserved contextual keywords
            // qualify: `true`/`false`/`void`/`null` are reserved (never entity-name
            // heads for tsc/prettier), so `true.X` stays primitive and rejects the
            // trailing `.X`, matching the formatter oracle.
            if kw.can_be_binding_name() && matches!(self.peek_kind(), TokenKind::Dot) {
                return self.parse_type_reference();
            }
            self.advance()?;
            return Ok(TSType::Keyword(TSKeywordType::new(ts_kind, span)));
        }

        match self.current_kind() {
            // `this` keyword in type context (for `this` type, e.g., `this is T`)
            TokenKind::Keyword(KeywordKind::This) => {
                let (start, end) = self.current_pos();
                self.advance()?;
                Ok(TSType::ThisType(TSThisType {
                    span: Span::new(start as u32, end as u32),
                }))
            }
            // `const` keyword in type context (for `as const`)
            // Treated as a type reference with name "const"
            TokenKind::Keyword(KeywordKind::Const) => {
                let (start, end) = self.current_pos();
                let name = self.current_raw_ident_name();
                self.advance()?;

                Ok(TSType::TypeReference(TSTypeReference {
                    type_name: TSEntityName::Identifier(Identifier::simple(
                        name,
                        Span::new(start as u32, end as u32),
                    )),
                    type_arguments: None,
                    span: Span::new(start as u32, end as u32),
                }))
            }
            // Numeric literal types: `1`, `42.5`, `1n`
            TokenKind::Number => {
                let literal = self.parse_number_or_bigint_literal()?;
                let is_bigint = matches!(literal.value, LiteralValue::BigInt);
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
                let cooked = self.extract_string_cooked();
                self.advance()?;

                Ok(TSType::Literal(TSLiteralType::String(Literal {
                    value: LiteralValue::String(cooked),
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

                let argument = self.parse_number_or_bigint_literal()?;
                let num_end = self.current_pos().1;
                // TODO should this be used?
                let _is_bigint = matches!(argument.value, LiteralValue::BigInt);
                self.advance()?;

                let unary = UnaryExpression {
                    operator: UnaryOperator::Minus,
                    prefix: true,
                    argument: self.alloc(Expression::Literal(argument)),
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
            // Any remaining reserved keyword is a type-reference NAME: TS's type
            // space is a separate `IdentifierName` namespace, so reserved statement
            // keywords (`break`, `default`, `function`, `case`, …) are valid type
            // names (tsc + prettier accept; acorn-typescript is over-strict here).
            // The primitive keywords, `this`/`const`/`import`/`typeof`/`new`, and
            // the contextual type operators (which lex as `Identifier`) all match
            // above, so only the reserved keywords reach this arm — the entity-name
            // head reads a keyword token via `current_raw_ident_name` (keywords are
            // never escaped), mirroring the `Const` arm.
            TokenKind::Keyword(_) => self.parse_type_reference(),
            _ => Err(self.error_expected_found("type")),
        }
    }

    /// Parse type operator: `keyof T`, `unique symbol`, `readonly T[]`
    ///
    /// Type operators bind looser than array `[]` and indexed access `[K]`, so:
    /// - `keyof A[B]` parses as `keyof (A[B])`, not `(keyof A)[B]`
    /// - `keyof A[]` parses as `keyof (A[])`, not `(keyof A)[]`
    /// - `readonly A[B][]` parses as `readonly ((A[B])[])`
    fn parse_type_operator(
        &mut self,
        operator: TSTypeOperatorKind,
    ) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.advance()?; // consume the operator keyword (keyof, unique, readonly)

        // Parse the type being operated on (including array/indexed access suffixes).
        // A type-operator operand is not a full-type position: function/constructor
        // types are disallowed (`keyof () => T` must be `keyof (() => T)`), so a
        // `(` here is a parenthesized type (`keyof ([a])[]` stays valid).
        let type_annotation = self.with_fn_type_disallowed(true, Self::parse_array_type)?;
        let end = type_annotation.span().end;

        Ok(TSType::TypeOperator(TSTypeOperator {
            operator,
            type_annotation: self.alloc(type_annotation),
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse infer type: `infer U` (in conditional type extends clause)
    fn parse_infer_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.advance()?; // consume 'infer'

        // Parse the type parameter name (must be an identifier)
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected("type parameter name after 'infer'"));
        }

        let (id_start, id_end) = self.current_pos();
        let ident_name = self.current_ident_name();
        self.advance()?;

        let name = Identifier::simple(ident_name, Span::new(id_start as u32, id_end as u32));

        // Optional constraint: `infer U extends C`. The constraint is parsed
        // with conditionals disallowed and function types re-enabled (acorn
        // parses it as a full type in the disallow-conditional context), so a
        // nested `? T : F` binds outward rather than being swallowed as a
        // nested conditional. Parens re-enable conditionals
        // (`infer U extends (A ? B : C)`), since the parenthesized-type parser
        // recurses into the full type grammar. Mirrors how a conditional's
        // `extends_type` is parsed (`parse_type`).
        //
        // At an allow-conditional position, a `?` directly after the
        // constraint means it was never a constraint at all:
        // `infer U extends C ? T : F` is a conditional whose check is the bare
        // `infer U` and whose extends clause is `C` (acorn rolls back its
        // constraint tryParse and re-parses at the conditional level; the
        // already-parsed type is handed to `parse_type` via
        // `pending_conditional_extends` instead of re-lexing). The re-bound
        // `extends` honors the conditional's no-line-terminator rule: after a
        // newline the constraint is kept, and the enclosing context then
        // rejects the stray `?` — matching acorn, whose rolled-back re-parse
        // refuses the `extends` and rejects on the stray token.
        let mut end = id_end as u32;
        let mut constraint = None;
        if self.check(&TokenKind::Keyword(KeywordKind::Extends)) {
            let extends_preceded_by_lt = self.had_line_terminator;
            self.advance()?; // consume 'extends'
            let constraint_type = self.with_conditional_type_disallowed(true, |p| {
                p.with_fn_type_disallowed(false, Self::parse_union_type)
            })?;
            if !self.conditional_type_disallowed
                && !extends_preceded_by_lt
                && self.check(&TokenKind::Question)
            {
                self.pending_conditional_extends = Some(constraint_type);
            } else {
                end = constraint_type.span().end;
                constraint = Some(self.alloc(constraint_type));
            }
        }

        Ok(TSType::Infer(TSInferType {
            type_parameter: TSTypeParameter {
                name,
                constraint,
                default: None,
                is_const: false,
                is_in: false,
                is_out: false,
                span: Span::new(id_start as u32, end),
            },
            span: Span::new(start as u32, end),
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
        if !matches!(self.current.kind, TokenKind::BracketOpen) {
            return false;
        }

        // Lookahead: check if pattern is `[identifier:`
        // We need to look past the '[', then the identifier, then check for ':'
        let bytes = self.source.as_bytes();
        let pos = skip_whitespace_and_comments(bytes, self.current.start as usize + 1); // skip '[' and whitespace/comments

        // Must be followed by an identifier
        if pos >= bytes.len() || !is_identifier_start(bytes[pos]) {
            return false;
        }

        // Skip the identifier and trailing whitespace/comments
        let pos = skip_whitespace_and_comments(bytes, skip_identifier(bytes, pos));

        // Check for ':'
        pos < bytes.len() && bytes[pos] == b':'
    }

    /// Check if the current position starts a mapped type: `[K in T]`, optionally
    /// prefixed by `readonly` (`readonly [K in T]`). A mapped type is the sole
    /// member of its type literal; the `in` keyword after the single
    /// type-parameter name is what distinguishes it from an index signature
    /// (`[k: T]`) or a computed-key member (`[expr]`), both of which fall through
    /// to the general member loop. `+readonly` / `-readonly` mapped types are
    /// detected earlier by their unambiguous `+`/`-` prefix, so this only handles
    /// the bare and `readonly` forms.
    ///
    /// acorn-typescript reads `[Ident in …]` as a mapped type unconditionally
    /// (even `[a in b]`), so a computed key that wants the `in` operator must
    /// parenthesize (`[(a in b)]`) or use a non-identifier head (`[a.b in c]`) —
    /// both fail the `[Ident in` shape here and parse as computed keys, matching
    /// acorn.
    fn is_mapped_type_start(&self) -> bool {
        let bytes = self.source.as_bytes();
        let mut pos = self.current.start as usize;

        // Optional leading `readonly` modifier (bare; `+readonly` / `-readonly`
        // are handled by the caller's `+`/`-` check).
        if matches!(self.current.kind, TokenKind::Identifier) && self.current_value() == "readonly"
        {
            pos = skip_whitespace_and_comments(bytes, skip_identifier(bytes, pos));
        }

        // Must be `[`
        if pos >= bytes.len() || bytes[pos] != b'[' {
            return false;
        }
        pos = skip_whitespace_and_comments(bytes, pos + 1);

        // Followed by the type-parameter name (an identifier)
        if pos >= bytes.len() || !is_identifier_start(bytes[pos]) {
            return false;
        }
        pos = skip_whitespace_and_comments(bytes, skip_identifier(bytes, pos));

        // Then the `in` keyword, at a word boundary so `[index]` and `[inK in K]`
        // don't false-match on a leading `in`.
        pos + 2 <= bytes.len()
            && &bytes[pos..pos + 2] == b"in"
            && (pos + 2 == bytes.len() || !is_identifier_continue(bytes[pos + 2]))
    }

    /// Parse type reference: `Foo` or `Foo.Bar` or `Foo<T>`
    fn parse_type_reference(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;
        let type_name = self.parse_entity_name()?;

        // Check for type arguments: <T, U>. A line break before `<` ends the type
        // at `type_name` — the `<` is not consumed as type arguments (see
        // `parse_optional_type_arguments_same_line`).
        let type_arguments = self.parse_optional_type_arguments_same_line()?;

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
    fn parse_import_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.advance()?; // consume 'import'
        let import = self.parse_import_type_body(start)?;
        Ok(TSType::Import(import))
    }

    /// Parse import type body after `import` keyword has been consumed.
    /// Parses: `('module')`, `('module', {options}).Qualifier<TypeArgs>`
    fn parse_import_type_body(&mut self, start: usize) -> Result<TSImportType<'arena>, ParseError> {
        // Expect '('
        self.expect(&TokenKind::ParenOpen)?;

        // Parse the module specifier (string literal)
        if !matches!(self.current_kind(), TokenKind::String) {
            return Err(self.error_expected("string literal in import type"));
        }

        let (arg_start, arg_end) = self.current_pos();
        let cooked = self.extract_string_cooked();
        self.advance()?;

        let argument = Literal {
            value: LiteralValue::String(cooked),
            span: Span::new(arg_start as u32, arg_end as u32),
        };

        // Optional options object: `import('module', {with: {type: 'json'}})`
        let options: Option<&'arena Expression<'arena>> = if self.eat(TokenKind::Comma) {
            Some(self.parse_expression_ref()?)
        } else {
            None
        };

        // Expect ')'
        self.expect(&TokenKind::ParenClose)?;

        // Optional qualifier: .Foo or .Foo.Bar
        let qualifier = if self.eat(TokenKind::Dot) {
            Some(self.parse_entity_name()?)
        } else {
            None
        };

        // Optional type arguments: <T, U>
        let type_arguments = self.parse_optional_type_arguments()?;

        let end = type_arguments
            .as_ref()
            .map(|ta| ta.span.end)
            .or_else(|| qualifier.as_ref().map(|q| q.span().end))
            .unwrap_or_else(|| self.prev_token_end() as u32);

        Ok(TSImportType {
            argument,
            options,
            qualifier,
            type_arguments,
            span: Span::new(start as u32, end),
        })
    }

    /// Parse type query: `typeof x`, `typeof Foo.bar`, `typeof import("module")`
    fn parse_type_query(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.advance()?; // consume 'typeof'

        // Check for import type: typeof import("module")
        let expr_name = if self.check(&TokenKind::Keyword(KeywordKind::Import)) {
            let import_start = self.current_pos().0;
            self.advance()?; // consume 'import'
            let import = self.parse_import_type_body(import_start)?;
            TSTypeQueryExprName::Import(self.alloc(import))
        } else {
            // Parse entity name: identifier or qualified name
            let entity_name = self.parse_entity_name()?;
            TSTypeQueryExprName::EntityName(entity_name)
        };

        // Parse optional type arguments: typeof Array<string>. A line break
        // before `<` ends the query — `typeof a` ⏎ `<T>(): void` in an interface
        // is two members, not an instantiation (see
        // `parse_optional_type_arguments_same_line`).
        let type_arguments = self.parse_optional_type_arguments_same_line()?;

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
    pub(crate) fn parse_entity_name(&mut self) -> Result<TSEntityName<'arena>, ParseError> {
        let (id_start, id_end) = self.current_pos();
        let name = self.current_ident_name();
        self.advance()?;

        let mut result = TSEntityName::Identifier(Identifier::simple(
            name,
            Span::new(id_start as u32, id_end as u32),
        ));

        while self.eat(TokenKind::Dot) {
            let right_name = self
                .try_ident_or_keyword_name()
                .ok_or_else(|| self.error_expected_after("identifier", "."))?;

            let (right_start, right_end) = self.current_pos();
            self.advance()?;

            let right =
                Identifier::simple(right_name, Span::new(right_start as u32, right_end as u32));

            result = TSEntityName::QualifiedName(self.alloc(TSQualifiedName {
                left: result,
                right,
                span: Span::new(id_start as u32, right_end as u32),
            }));
        }

        Ok(result)
    }

    /// Parse a `<T, U>` type-argument list when the next token opens one (`<`),
    /// else `None` — the optional-type-arguments guard shared by
    /// expression-with-type-arguments, `import(...)` types, and `extends`-clause
    /// heritage, which consume type arguments *across* a line break (matching
    /// acorn). Callers that must instead stop at a line break — type references
    /// and `typeof` queries — use `parse_optional_type_arguments_same_line`.
    pub(in crate::parser) fn parse_optional_type_arguments(
        &mut self,
    ) -> Result<Option<TSTypeParameterInstantiation<'arena>>, ParseError> {
        if self.check_less_than_in_type() {
            Ok(Some(self.parse_type_arguments()?))
        } else {
            Ok(None)
        }
    }

    /// Like `parse_optional_type_arguments`, but a preceding line terminator ends
    /// the type: a `<` after a line break is NOT consumed as type arguments
    /// (tsc's `!scanner.hasPrecedingLineBreak()` guard on
    /// `parseTypeArgumentsOfTypeReference` / `tsParseTypeQuery`). So `B` ⏎ `<T>`
    /// is the type `B` followed by a separate `<T>`, not `B<T>` — in a type-member
    /// list this ASI-splits `a: B` ⏎ `<T>(): C` into two members. This mirrors the
    /// `!had_line_terminator` guard the postfix-`[]` and `extends` sites also apply.
    pub(in crate::parser) fn parse_optional_type_arguments_same_line(
        &mut self,
    ) -> Result<Option<TSTypeParameterInstantiation<'arena>>, ParseError> {
        if self.had_line_terminator {
            Ok(None)
        } else {
            self.parse_optional_type_arguments()
        }
    }

    /// Parse type arguments: `<T, U>` (trailing comma allowed, but cannot be empty)
    pub(in crate::parser) fn parse_type_arguments(
        &mut self,
    ) -> Result<TSTypeParameterInstantiation<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.expect_less_than_in_type()?;

        let mut params = self.bvec();
        if !self.check_greater_than_in_type() {
            params.push(self.parse_type()?);
            while self.eat(TokenKind::Comma) {
                // Allow trailing comma - check for closing > before parsing another type
                if self.check_greater_than_in_type() {
                    break;
                }
                params.push(self.parse_type()?);
            }
        }

        // Type argument list cannot be empty
        if params.is_empty() {
            return Err(self.error_msg("Type argument list cannot be empty"));
        }

        let end = self.greater_than_end_in_type()?;

        Ok(TSTypeParameterInstantiation {
            params: params.into_bump_slice(),
            span: Span::new(start as u32, end),
        })
    }

    /// Whether the just-opened `(` (the `ParenOpen` already consumed; `paren_offset`
    /// is its raw byte offset into `self.source`, not a span coordinate) begins a
    /// function type's parameter list rather than a parenthesized type — the two
    /// cases where a leading token that [`is_definitely_type_start`] would otherwise
    /// claim as a type is actually an (ambiguous) parameter. Only meaningful at
    /// full-type positions; at operand positions (`fn_type_disallowed`) the caller
    /// short-circuits to a parenthesized type before consulting this.
    ///
    /// - a **destructuring pattern** `([a]) => …` / `({ a }) => …`: a leading `[`/`{`
    ///   with a matching `)` then `=>` (`scan_parens_then_arrow`).
    ///   `paren_pattern_then_type_operator` additionally rules a leading `{…}`/`[…]`
    ///   directly followed by `|`/`&` back out (a union/intersection type, never a
    ///   parameter), e.g. `({ b: B } | C)` / `([B] | C)`.
    /// - a **contextual type keyword** `(number) => …` (every type keyword except the
    ///   reserved `void`/`null`, which can't be a parameter name): a parameter when an
    ///   immediate `:`/`?`/`,` follows (none can appear there in a parenthesized type)
    ///   or, for a bare single parameter, a `)` then `=>`.
    ///
    /// This mirrors acorn-typescript's `tsIsUnambiguouslyStartOfFunctionType`
    /// lookahead, which likewise runs only at full-type positions.
    ///
    /// [`is_definitely_type_start`]: Self::is_definitely_type_start
    fn paren_starts_function_type_params(&mut self, paren_offset: usize) -> bool {
        let source_bytes = self.source.as_bytes();

        // Destructuring-pattern parameter: `([a]) => U`, `({ a }) => U`.
        let pattern_param = matches!(
            self.current_kind(),
            TokenKind::BracketOpen | TokenKind::BraceOpen
        ) && scan_parens_then_arrow(source_bytes, paren_offset)
            && !paren_pattern_then_type_operator(source_bytes, paren_offset);

        // Contextual type keyword as a parameter name: `(number) => U`.
        let type_keyword_param = matches!(
            self.current_kind(),
            TokenKind::Keyword(kw)
                if kw.is_type_keyword() && !matches!(kw, KeywordKind::Void | KeywordKind::Null)
        ) && match self.peek_kind() {
            TokenKind::Colon | TokenKind::Question | TokenKind::Comma => true,
            TokenKind::ParenClose => scan_parens_then_arrow(source_bytes, paren_offset),
            _ => false,
        };

        pattern_param || type_keyword_param
    }

    /// Parse parenthesized type `(T)` or function type `(x: T) => U`
    fn parse_parenthesized_or_function_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        // Raw `(` offset into `self.source` for the byte scan below (NOT
        // `current_pos()`, which adds `base_offset` for span coordinates and would
        // mis-index the embedded `<script>` slice).
        let paren_offset = self.current.start as usize;
        let start = self.current_pos().0;
        self.expect(&TokenKind::ParenOpen)?;

        // Whether this `(` opens a function type's parameter list. At an operand
        // position (`fn_type_disallowed`: a union/intersection constituent or
        // type-operator operand) the grammar has no function types, so it never
        // does — a following `=>` belongs to an enclosing construct, e.g. the
        // enclosing arrow function's own `=>` after its return-type annotation
        // (`(): A & (B) => x`). At full-type positions the ambiguous pattern /
        // type-keyword cases consult the lookahead helper.
        let starts_function_params =
            !self.fn_type_disallowed && self.paren_starts_function_type_params(paren_offset);

        // Parenthesized type: any operand-position `(`, or a full-type-position
        // `(` on a token that definitely starts a type, not a parameter name
        // (keywords like typeof/new/import, type operators, brackets, literals).
        // The inner parse is a full-type position again (`parse_type` clears the
        // flag), so `A & (() => B)` works.
        if self.fn_type_disallowed || (!starts_function_params && self.is_definitely_type_start()) {
            let inner_type = self.parse_type()?;
            self.expect(&TokenKind::ParenClose)?;
            let end = self.prev_token_end();

            return Ok(TSType::Parenthesized(TSParenthesizedType {
                type_annotation: self.alloc(inner_type),
                span: Span::new(start as u32, end as u32),
            }));
        }

        // Try to parse as function parameters
        let (params, saw_comma) = self.parse_function_type_params()?;
        let params: &'arena [Expression<'arena>] = params.into_bump_slice();
        self.expect(&TokenKind::ParenClose)?;

        // A single identifier without type annotation, optional marker, comma,
        // or a following `=>` is a parenthesized type: a type reference `(T)`,
        // or the this-type `(this)` — `this` is a valid parameter name
        // (`parse_function_type_param`), so it lands here too.
        if !self.check(&TokenKind::Arrow)
            && !saw_comma
            && let [Expression::Identifier(id)] = params
            && id.type_annotation().is_none()
            && !id.optional
        {
            let inner = if self.ident_name_is(id, "this") {
                TSType::ThisType(TSThisType { span: id.span })
            } else {
                TSType::TypeReference(TSTypeReference {
                    type_name: TSEntityName::Identifier(id.clone()),
                    type_arguments: None,
                    span: id.span,
                })
            };
            // Use end of closing paren, not end of inner type
            let end = self.prev_token_end() as u32;
            return Ok(TSType::Parenthesized(TSParenthesizedType {
                type_annotation: self.alloc(inner),
                span: Span::new(start as u32, end),
            }));
        }

        // Anything else committed to a function-type parameter list (annotated
        // — including `this: T` — optional, empty, rest, pattern, trailing
        // comma, or multiple params), so the `=>` and return type are required
        // — there is no implicit-void function type (acorn rejects `(x: T)` /
        // `()` without `=>`). The return type may be a type predicate
        // (asserts x, x is T).
        let arrow_start = self.current_pos().0 as u32;
        self.expect(&TokenKind::Arrow)?;
        let return_type = self.parse_return_type_inner(arrow_start)?;
        let end = return_type.span.end;

        Ok(TSType::Function(TSFunctionType {
            type_parameters: None,
            params,
            return_type,
            span: Span::new(start as u32, end),
        }))
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
            // Import types: import("./a").B
            TokenKind::Keyword(KeywordKind::Import) => true,
            // Non-identifier tokens that start types
            TokenKind::BracketOpen => true, // tuple types
            TokenKind::BraceOpen => true,   // object types
            TokenKind::LessThan => true,    // generic function types
            TokenKind::Minus => true,       // negative number literals
            TokenKind::ParenOpen => true,   // nested parenthesized types
            TokenKind::Pipe => true,        // leading pipe in union: (| A | B)
            TokenKind::Ampersand => true,   // leading ampersand in intersection: (& A & B)
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
                // (A | B) union, (A & B) intersection, (A<B>) generic,
                // (A[K]) indexed access, (T extends U ? V : W) conditional,
                // (ns.X) qualified type reference
                matches!(
                    self.peek_kind(),
                    TokenKind::Pipe
                        | TokenKind::Ampersand
                        | TokenKind::LessThan
                        | TokenKind::BracketOpen
                        | TokenKind::Keyword(KeywordKind::Extends)
                        | TokenKind::Dot
                )
            }
            _ => false,
        }
    }

    /// Parse generic function type: `<T>() => U`, `<T, U extends V>(x: T) => U`
    fn parse_generic_function_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        // Function types are not allowed at operand positions (`A & <T>() => U`
        // must be `A & (<T>() => U)`) — see `fn_type_disallowed`.
        if self.fn_type_disallowed {
            return Err(self.error_expected_found("type"));
        }

        let start = self.current_pos().0;

        // Parse type parameters: <T, U extends V, ...>
        let type_parameters = self.parse_type_parameters()?;

        // Parse parameter list
        self.expect(&TokenKind::ParenOpen)?;
        let (params, _saw_comma) = self.parse_function_type_params()?;
        let params = params.into_bump_slice();
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
            return_type,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse constructor type: `new () => T`, `new <T>() => T`, `abstract new () => T`
    fn parse_constructor_type(&mut self, is_abstract: bool) -> Result<TSType<'arena>, ParseError> {
        // Constructor types are not allowed at operand positions (`A & new () => T`
        // must be `A & (new () => T)`) — see `fn_type_disallowed`.
        if self.fn_type_disallowed {
            return Err(self.error_expected_found("type"));
        }

        let start = self.current_pos().0;

        // If abstract, consume 'abstract' keyword
        if is_abstract {
            self.advance()?; // consume 'abstract'
        }

        // Expect 'new' keyword
        self.expect(&TokenKind::Keyword(KeywordKind::New))?;

        // Parse optional type parameters: <T>
        let type_parameters = self.parse_optional_type_parameters()?;

        // Parse parameter list
        self.expect(&TokenKind::ParenOpen)?;
        let (params, _saw_comma) = self.parse_function_type_params()?;
        let params = params.into_bump_slice();
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
            return_type,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse function type parameters. The returned flag is whether any comma
    /// was consumed — a `(T,)` list commits to a function type even though it
    /// holds a single bare identifier, so the parenthesized-type
    /// reinterpretation in `parse_parenthesized_or_function_type` needs it.
    fn parse_function_type_params(
        &mut self,
    ) -> Result<(bumpalo::collections::Vec<'arena, Expression<'arena>>, bool), ParseError> {
        let mut params = self.bvec();
        let mut saw_comma = false;

        if !self.check(&TokenKind::ParenClose) {
            let first = self.parse_function_type_param()?;
            let mut rest_seen = matches!(&first, Expression::RestElement(_));
            params.push(first);
            while self.eat(TokenKind::Comma) {
                saw_comma = true;
                // A rest parameter must be last: nothing — not even a trailing
                // comma — may follow it (see `parse_parameter_list`). In an
                // ambient (`declare`) context acorn tolerates a single trailing
                // comma; the `)` check below permits exactly that shape.
                if rest_seen {
                    if self.in_ambient_context && self.check(&TokenKind::ParenClose) {
                        break;
                    }
                    return Err(self.error_msg("A rest parameter must be last in a parameter list"));
                }
                // Handle trailing comma
                if self.check(&TokenKind::ParenClose) {
                    break;
                }
                let param = self.parse_function_type_param()?;
                rest_seen = matches!(&param, Expression::RestElement(_));
                params.push(param);
            }
        }

        Ok((params, saw_comma))
    }

    /// Parse a single function type parameter: an identifier (`x: T`), the `this`
    /// parameter (`this: T`), a destructuring pattern (`[a]?: T`, `{ a }: T`), or a
    /// rest element wrapping any of those (`...a: T[]`, `...[a]: T[]`, `...{ a }: T`).
    fn parse_function_type_param(&mut self) -> Result<Expression<'arena>, ParseError> {
        // Rest parameter: `...a`, `...[a]`, `...{ a }` (with an optional annotation).
        if self.check(&TokenKind::DotDotDot) {
            let (start, _) = self.current_pos();
            self.advance()?;
            // A rest argument is a bare binding — an identifier or a destructuring
            // pattern. Its optional `?` and `: T` bind to the `RestElement` (acorn's
            // shape), never to the inner binding, so parse them here rather than
            // through the shared param parser (which attaches them to the binding,
            // as for a non-rest `([a]?: T) => U`). The `?` is a deferred
            // grammar-check error (TS1047; see `RestElement`), preserved just as
            // `parameters.rs` preserves it for a value signature.
            let argument = if matches!(
                self.current_kind(),
                TokenKind::BracketOpen | TokenKind::BraceOpen
            ) {
                self.parse_binding_pattern()?
            } else {
                let (id_start, id_end) = self.current_pos();
                let name = self
                    .try_param_name()
                    .ok_or_else(|| self.error_expected("parameter name"))?;
                self.advance()?;
                Expression::Identifier(Identifier::simple(
                    name,
                    Span::new(id_start as u32, id_end as u32),
                ))
            };
            let (optional, type_annotation, arg_end) =
                self.parse_rest_param_tail(argument.span().end)?;
            return Ok(Expression::RestElement(RestElement {
                argument: self.alloc(argument),
                optional,
                type_annotation,
                span: Span::new(start as u32, arg_end),
            }));
        }

        // Destructuring-pattern parameter: `[a, b]?: T`, `{ a }: T`. Shares the
        // parameter-list pattern parser (bare pattern, optional `?`, `: Type`),
        // so `optional`/span handling matches a real signature parameter.
        if matches!(
            self.current_kind(),
            TokenKind::BracketOpen | TokenKind::BraceOpen
        ) {
            return self.parse_destructured_binding(true);
        }

        let (id_start, id_end) = self.current_pos();

        // Accept identifiers and contextual keywords (e.g., `from`, `as`) as parameter
        // names, plus the `this` keyword (TypeScript `this` parameter: `(this: T) => U`).
        let name = self
            .try_param_name()
            .ok_or_else(|| self.error_expected("parameter name"))?;
        self.advance()?;

        // Check for optional: param? — a bare `?` extends the span.
        let (optional, id_end) = self.eat_optional_marker(id_end);

        // Check for type annotation: : T
        let type_annotation = self.parse_optional_type_annotation()?;

        let end = type_annotation
            .as_ref()
            .map_or_else(|| id_end as u32, |ta| ta.span.end);

        let extra = type_annotation.map(|ta| self.typed_extra(ta));

        Ok(Expression::Identifier(Identifier {
            escaped_name: name.escaped,
            name_len: name.raw_len,
            optional,
            extra,
            span: Span::new(id_start as u32, end),
        }))
    }

    /// Parse object type: `{ prop: T; method(): U }` or mapped type: `{ [K in T]: V }`
    fn parse_object_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::BraceOpen)?;

        // Check for unambiguous mapped type modifiers: +/- (with or without readonly)
        // +readonly, -readonly, +, - all indicate mapped type
        if self.check(&TokenKind::Minus) || self.check(&TokenKind::Plus) {
            return self.parse_mapped_type_body(start);
        }

        // A mapped type (`{ [K in T]: V }`, optionally `readonly`) is the sole
        // member and is parsed specially. Index signatures (`{ [k: T]: V }`) and
        // computed-key members (`{ [expr]: V }`) both flow through the general
        // member loop below, where `parse_type_element` handles the `readonly`
        // modifier, index signatures, and arbitrary computed-key expressions.
        if self.is_mapped_type_start() {
            return self.parse_mapped_type_body(start);
        }

        let members = self.parse_type_members()?;

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(TSType::TypeLiteral(TSTypeLiteral {
            members: members.into_bump_slice(),
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse the body of a mapped type (after '{' has been consumed)
    fn parse_mapped_type_body(&mut self, start: usize) -> Result<TSType<'arena>, ParseError> {
        // Parse optional readonly modifier: `readonly`, `+readonly`, `-readonly`
        let readonly = self.parse_mapped_type_readonly_modifier()?;

        // Expect `[`
        self.expect(&TokenKind::BracketOpen)?;

        // Parse type parameter name: `K` (identifier or contextual keyword; the
        // `IdentName` is `Copy`, so no owned `String` is threaded through to
        // `finish_mapped_type`).
        let param_start = self.current_pos().0;
        let param_name = self
            .try_ident_or_keyword_name()
            .ok_or_else(|| self.error_expected("type parameter name in mapped type"))?;
        self.advance()?;

        // Expect `in`
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::In)) {
            return Err(self.error_expected("'in' in mapped type"));
        }
        self.advance()?; // consume 'in'

        self.finish_mapped_type(start, param_start, param_name, readonly)
    }

    /// Shared tail for mapped type parsing after `[K in` has been consumed.
    /// Parses: constraint, optional `as` clause, `]`, optional modifier, `:`, value type, `}`
    fn finish_mapped_type(
        &mut self,
        start: usize,
        param_start: usize,
        param_name: IdentName,
        readonly: Option<TSMappedTypeModifier>,
    ) -> Result<TSType<'arena>, ParseError> {
        let arena = self.arena;

        // Parse constraint type (e.g., `keyof T`)
        let constraint = self.parse_type()?;
        let param_end = constraint.span().end;

        // Check for optional `as` clause: `as NewKey`
        let name_type = if self.eat(TokenKind::Keyword(KeywordKind::As)) {
            Some(&*arena.alloc(self.parse_type()?))
        } else {
            None
        };

        // Expect `]`
        self.expect(&TokenKind::BracketClose)?;

        // Parse optional modifier: `?`, `+?`, `-?`
        let optional = self.parse_mapped_type_optional_modifier()?;

        // Parse optional `:` and value type — the annotation may be absent
        // entirely (`{ [K in T] }`, `{ [K in T]+? }`), matching acorn.
        let type_annotation = if self.eat(TokenKind::Colon) {
            Some(&*arena.alloc(self.parse_type()?))
        } else {
            None
        };

        // Consume optional separator
        self.eat(TokenKind::Semicolon);

        // Expect `}`
        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(TSType::Mapped(TSMappedType {
            type_parameter: TSMappedTypeParameter {
                name: param_name,
                constraint: self.alloc(constraint),
                span: Span::new(param_start as u32, param_end),
            },
            name_type,
            type_annotation,
            readonly,
            optional,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse readonly modifier for mapped type: `readonly`, `+readonly`, `-readonly`.
    /// A `+`/`-` is a single optional sign that *must* be followed by `readonly`;
    /// a stray sign (`+[K in T]`, `-+readonly`) is a syntax error, not a silently
    /// dropped token (matches acorn-typescript).
    fn parse_mapped_type_readonly_modifier(
        &mut self,
    ) -> Result<Option<TSMappedTypeModifier>, ParseError> {
        if self.eat(TokenKind::Minus) {
            // `-readonly`
            if self.eat_contextual_keyword("readonly") {
                return Ok(Some(TSMappedTypeModifier::Minus));
            }
            return Err(self.error_expected("'readonly' after '-' in mapped type"));
        }

        if self.eat(TokenKind::Plus) {
            // `+readonly`
            if self.eat_contextual_keyword("readonly") {
                return Ok(Some(TSMappedTypeModifier::Plus));
            }
            return Err(self.error_expected("'readonly' after '+' in mapped type"));
        }

        if self.eat_contextual_keyword("readonly") {
            return Ok(Some(TSMappedTypeModifier::True));
        }

        Ok(None)
    }

    /// Parse optional modifier for mapped type: `?`, `+?`, `-?`. A `+`/`-` is a
    /// single optional sign that *must* be followed by `?`; a stray sign
    /// (`[K in T]+:`) is a syntax error, not a silently dropped token (matches
    /// acorn-typescript).
    fn parse_mapped_type_optional_modifier(
        &mut self,
    ) -> Result<Option<TSMappedTypeModifier>, ParseError> {
        if self.eat(TokenKind::Minus) {
            // `-?`
            if self.eat(TokenKind::Question) {
                return Ok(Some(TSMappedTypeModifier::Minus));
            }
            return Err(self.error_expected("'?' after '-' in mapped type"));
        }

        if self.eat(TokenKind::Plus) {
            // `+?`
            if self.eat(TokenKind::Question) {
                return Ok(Some(TSMappedTypeModifier::Plus));
            }
            return Err(self.error_expected("'?' after '+' in mapped type"));
        }

        if self.eat(TokenKind::Question) {
            return Ok(Some(TSMappedTypeModifier::True));
        }

        Ok(None)
    }

    /// Parse tuple type: `[T, U, V]`, `[...T]`, `[label: T]`, `[T?]`, `[first: string, ...rest: T]`
    fn parse_tuple_type(&mut self) -> Result<TSType<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::BracketOpen)?;

        let mut element_types = self.bvec();
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
            element_types: element_types.into_bump_slice(),
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse a single tuple element: `T`, `T?`, `...T`, `label: T`, `label?: T`, `...label: T`
    fn parse_tuple_element(&mut self) -> Result<TSType<'arena>, ParseError> {
        let elem_start = self.current_pos().0;

        // Check for rest element: `...T` or `...label: T`
        if self.eat(TokenKind::DotDotDot) {
            let inner = self.parse_tuple_element_inner()?;
            let end = inner.span().end;
            return Ok(TSType::Rest(TSRestType {
                type_annotation: self.alloc(inner),
                span: Span::new(elem_start as u32, end),
            }));
        }

        self.parse_tuple_element_inner()
    }

    /// Parse a tuple element (without leading `...`): `T`, `T?`, `label: T`, `label?: T`
    fn parse_tuple_element_inner(&mut self) -> Result<TSType<'arena>, ParseError> {
        let elem_start = self.current_pos().0;

        // Check for named tuple member: `label: T` or `label?: T`
        // An identifier followed by `:` indicates a named tuple member
        // An identifier followed by `?:` indicates an optional named tuple member
        if matches!(self.current_kind(), TokenKind::Identifier)
            && matches!(self.peek_kind(), TokenKind::Colon | TokenKind::Question)
        {
            let (label_start, label_end) = self.current_pos();
            let label_name = self.current_ident_name();
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
                            label_name,
                            Span::new(label_start as u32, label_end as u32),
                        )),
                        type_arguments: None,
                        span: Span::new(label_start as u32, label_end as u32),
                    });
                    return Ok(TSType::Optional(TSOptionalType {
                        type_annotation: self.alloc(type_ref),
                        span: Span::new(elem_start as u32, self.prev_token_end() as u32),
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
                    label_name,
                    Span::new(label_start as u32, label_end as u32),
                ),
                element_type: self.alloc(element_type),
                optional,
                span: Span::new(elem_start as u32, end),
            }));
        }

        // Parse as regular type, then check for trailing `?` (optional type)
        let inner_type = self.parse_type()?;

        // Check for optional suffix: `T?`
        if self.eat(TokenKind::Question) {
            let end = self.prev_token_end();
            Ok(TSType::Optional(TSOptionalType {
                type_annotation: self.alloc(inner_type),
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
    fn parse_template_literal_type(&mut self) -> Result<TemplateLiteralType<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        let mut quasis = self.bvec();
        let mut types = self.bvec();

        match self.current_kind() {
            TokenKind::NoSubstitutionTemplate => {
                // Simple template with no interpolation: `hello world`
                let (elem_start, elem_end) = self.current_pos();
                let raw = self.current_value();

                // Extract content between backticks
                let (content, raw_span) = if raw.len() >= 2 {
                    (
                        &raw[1..raw.len() - 1],
                        Span::new(elem_start as u32 + 1, elem_end as u32 - 1),
                    )
                } else {
                    ("", Span::new(elem_start as u32, elem_start as u32))
                };
                let has_newline = content.contains('\n');

                let cooked = self.template_cooked(content, false)?;

                self.advance()?;

                quasis.push(TemplateElement {
                    raw_span,
                    cooked,
                    has_newline,
                    tail: true,
                    span: Span::new(elem_start as u32, elem_end as u32),
                });

                Ok(TemplateLiteralType {
                    quasis: quasis.into_bump_slice(),
                    types: types.into_bump_slice(),
                    span: Span::new(start as u32, elem_end as u32),
                })
            }
            TokenKind::TemplateHead => {
                // Template with interpolation: `hello ${string}...`
                let (elem_start, elem_end) = self.current_pos();
                let raw = self.current_value();

                // Extract content: remove leading ` and trailing ${
                let (content, raw_span) = if raw.len() >= 3 {
                    (
                        &raw[1..raw.len() - 2],
                        Span::new(elem_start as u32 + 1, elem_end as u32 - 2),
                    )
                } else {
                    ("", Span::new(elem_start as u32, elem_start as u32))
                };
                let has_newline = content.contains('\n');

                let cooked = self.template_cooked(content, false)?;

                self.advance()?;

                quasis.push(TemplateElement {
                    raw_span,
                    cooked,
                    has_newline,
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
                            let (tail_start, tail_end) = self.current_pos();
                            let tail_raw = self.current_value();

                            // Extract content: remove leading } and trailing `.
                            // The node span starts at the prior `}` (brace_start); the
                            // raw content span uses the token's own start (tail_start).
                            let (tail_content, raw_span) = if tail_raw.len() >= 2 {
                                (
                                    &tail_raw[1..tail_raw.len() - 1],
                                    Span::new(tail_start as u32 + 1, tail_end as u32 - 1),
                                )
                            } else {
                                ("", Span::new(tail_start as u32, tail_start as u32))
                            };
                            let has_newline = tail_content.contains('\n');

                            let tail_cooked = self.template_cooked(tail_content, false)?;

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw_span,
                                cooked: tail_cooked,
                                has_newline,
                                tail: true,
                                span: Span::new(brace_start as u32, tail_end as u32),
                            });

                            return Ok(TemplateLiteralType {
                                quasis: quasis.into_bump_slice(),
                                types: types.into_bump_slice(),
                                span: Span::new(start as u32, tail_end as u32),
                            });
                        }
                        TokenKind::TemplateMiddle => {
                            // Middle part: }content${
                            let (mid_start, mid_end) = self.current_pos();
                            let mid_raw = self.current_value();

                            // Extract content: remove leading } and trailing ${.
                            // The node span starts at the prior `}` (brace_start); the
                            // raw content span uses the token's own start (mid_start).
                            let (mid_content, raw_span) = if mid_raw.len() >= 3 {
                                (
                                    &mid_raw[1..mid_raw.len() - 2],
                                    Span::new(mid_start as u32 + 1, mid_end as u32 - 2),
                                )
                            } else {
                                ("", Span::new(mid_start as u32, mid_start as u32))
                            };
                            let has_newline = mid_content.contains('\n');

                            let mid_cooked = self.template_cooked(mid_content, false)?;

                            self.advance()?;

                            quasis.push(TemplateElement {
                                raw_span,
                                cooked: mid_cooked,
                                has_newline,
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

    /// Parse type parameters: `<T, U extends V = W>`
    /// Parse a `<T, U>` type-parameter declaration list when the next token
    /// opens one (`<`), else `None` — the optional-generics guard shared by every
    /// declaration site (functions, classes, interfaces, type aliases, and call /
    /// method / construct signatures).
    pub(in crate::parser) fn parse_optional_type_parameters(
        &mut self,
    ) -> Result<Option<TSTypeParameterDeclaration<'arena>>, ParseError> {
        if self.check(&TokenKind::LessThan) {
            Ok(Some(self.parse_type_parameters()?))
        } else {
            Ok(None)
        }
    }

    pub(in crate::parser) fn parse_type_parameters(
        &mut self,
    ) -> Result<TSTypeParameterDeclaration<'arena>, ParseError> {
        let start = self.current_pos().0 as u32;
        self.expect(&TokenKind::LessThan)?;

        let mut params = self.bvec();
        let mut trailing_comma = None;
        loop {
            let param = self.parse_type_parameter()?;
            params.push(param);

            if !self.eat(TokenKind::Comma) {
                break;
            }
            // Handle trailing comma
            if self.check_greater_than_in_type() {
                trailing_comma = Some(self.prev_token_end() as u32 - 1);
                break;
            }
        }

        let end = self.greater_than_end_in_type()?;

        Ok(TSTypeParameterDeclaration {
            params: params.into_bump_slice(),
            trailing_comma,
            span: Span::new(start, end),
        })
    }

    /// Parse a single type parameter: `T`, `T extends U`, or `T extends U = V`
    /// With optional modifiers: `const T`, `in T`, `out T`, `in out T`
    fn parse_type_parameter(&mut self) -> Result<TSTypeParameter<'arena>, ParseError> {
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
        let ident_name = self.current_ident_name();
        self.advance()?;
        let name = Identifier::simple(ident_name, Span::new(id_start as u32, id_end as u32));

        // Track the end position as we parse optional parts
        let mut end = id_end as u32;

        // Parse optional constraint: `extends U`
        let constraint = if self.check(&TokenKind::Keyword(KeywordKind::Extends)) {
            self.advance()?;
            let constraint_type = self.parse_type()?;
            end = constraint_type.span().end;
            Some(self.alloc(constraint_type))
        } else {
            None
        };

        // Parse optional default: `= V`
        let default = if self.eat(TokenKind::Equals) {
            let default_type = self.parse_type()?;
            end = default_type.span().end;
            Some(self.alloc(default_type))
        } else {
            None
        };

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
    ) -> Result<TSTypeParameterInstantiation<'arena>, ParseError> {
        let start = self.current_pos().0 as u32;
        self.expect_less_than_in_type()?;

        let mut params = self.bvec();
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

        let end = self.greater_than_end_in_type()?;

        Ok(TSTypeParameterInstantiation {
            params: params.into_bump_slice(),
            span: Span::new(start, end),
        })
    }
}
