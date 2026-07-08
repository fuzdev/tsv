// Class declaration parsing

use crate::ast::internal::{
    Accessibility, BlockStatement, ClassBody, ClassDeclaration, ClassExpression, ClassMember,
    Decorator, ExportDefaultDeclaration, ExportDefaultValue, ExportKind, ExportNamedDeclaration,
    Expression, FunctionExpression, Identifier, Literal, LiteralValue, MethodDefinition,
    MethodKind, PropertyDefinition, PropertyModifier, Statement, StaticBlock, TSIndexSignature,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation,
};
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

/// Everything parsed off the front of a class member before its body — the
/// modifier set, the (possibly computed) key, and the optional/type-param
/// markers. `parse_class_member` builds this, then dispatches to
/// `finish_method_member` / `finish_property_member`.
#[allow(clippy::struct_excessive_bools)] // independent modifier flags, not a state machine
struct ClassMemberHeader<'arena> {
    start: usize,
    decorators: &'arena [Decorator<'arena>],
    accessibility: Option<Accessibility>,
    is_static: bool,
    is_declare: bool,
    is_override: bool,
    is_abstract: bool,
    readonly: bool,
    accessor: bool,
    is_generator: bool,
    is_async: bool,
    accessor_kind: Option<MethodKind>,
    computed: bool,
    key: Expression<'arena>,
    /// Whether the (plain identifier or string-literal) key spells `constructor`
    /// — the one bit `finish_method_member` derives from the member name to pick
    /// `MethodKind::Constructor`. Computed/private/numeric keys are never the
    /// constructor, so they record `false`. The name itself isn't carried: it is
    /// recoverable from `key`, and nothing else consumes it.
    name_is_constructor: bool,
    modifier: PropertyModifier,
    type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
}

impl<'a, 'arena> Parser<'a, 'arena> {
    pub(super) fn parse_class_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        let class = self.parse_class_declaration_inner(true, false)?;
        Ok(Statement::ClassDeclaration(class))
    }

    /// Parse an abstract class declaration: `abstract class Foo { ... }`
    pub(super) fn parse_abstract_class(&mut self) -> Result<Statement<'arena>, ParseError> {
        // Capture position of 'abstract' before consuming it
        let abstract_start = self.current_pos().0;
        debug_assert!(self.current_value() == "abstract");
        self.advance()?;

        let class =
            self.parse_class_declaration_inner_with_start(true, true, abstract_start, false)?;
        Ok(Statement::ClassDeclaration(class))
    }

    /// Parse a decorated class: `@decorator class Foo { }`
    ///
    /// Decorators can be stacked: `@dec1 @dec2 class Foo { }`
    /// Decorator can be followed by `abstract class` or `export class`
    pub(super) fn parse_decorated_class(&mut self) -> Result<Statement<'arena>, ParseError> {
        let start = self.current_pos().0;

        // Parse one or more decorators
        let decorators = self.parse_decorators()?;

        // Check for `export` before class
        let is_export = *self.current_kind() == TokenKind::Keyword(KeywordKind::Export);
        let is_default = if is_export {
            self.advance()?; // consume 'export'
            if *self.current_kind() == TokenKind::Keyword(KeywordKind::Default) {
                self.advance()?; // consume 'default'
                true
            } else {
                false
            }
        } else {
            false
        };

        // Optional `abstract`, then the `class` declaration with the decorators
        // attached (name optional for `export default`).
        let class = self.finish_decorated_class(start, decorators, !is_default)?;

        // Wrap in export if needed
        if is_export {
            if is_default {
                let end = class.span.end;
                Ok(Statement::ExportDefaultDeclaration(
                    ExportDefaultDeclaration {
                        declaration: ExportDefaultValue::ClassDeclaration(class),
                        span: Span::new(start as u32, end),
                    },
                ))
            } else {
                let end = class.span.end;
                let class_decl = Statement::ClassDeclaration(class);
                Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
                    declaration: Some(self.alloc(class_decl)),
                    specifiers: &[],
                    source: None,
                    attributes: None,
                    export_kind: ExportKind::Value,
                    span: Span::new(start as u32, end),
                }))
            }
        } else {
            Ok(Statement::ClassDeclaration(class))
        }
    }

    /// With leading `decorators` (starting at `deco_start`) already parsed, consume the
    /// optional `abstract` modifier, expect and parse the `class` declaration, attach
    /// the decorators, and extend the class span back over them. Shared by
    /// `parse_decorated_class` (decorator-first `@dec [export] class`) and the
    /// `export @dec class` arm of `parse_export_declaration` (decorator-after-`export`).
    pub(super) fn finish_decorated_class(
        &mut self,
        deco_start: usize,
        decorators: bumpalo::collections::Vec<'arena, Decorator<'arena>>,
        name_required: bool,
    ) -> Result<ClassDeclaration<'arena>, ParseError> {
        let is_abstract = matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "abstract"
            && self.peek_kind() == TokenKind::Keyword(KeywordKind::Class);
        if is_abstract {
            self.advance()?; // consume 'abstract'
        }

        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Class)) {
            return Err(self.error_expected_after("'class'", "decorator"));
        }

        let mut class = self.parse_class_declaration_inner(name_required, is_abstract)?;
        class.decorators = if decorators.is_empty() {
            None
        } else {
            Some(decorators.into_bump_slice())
        };
        class.span = Span::new(deco_start as u32, class.span.end);
        Ok(class)
    }

    /// Parse a decorated class expression: `@dec class {}`.
    ///
    /// The expression-position counterpart of `parse_decorated_class` (which
    /// builds a class *declaration*): parse the decorator list, then a class
    /// expression, attaching the decorators to it. `export`/`abstract` are
    /// declaration-only, so neither appears here.
    pub(in crate::parser) fn parse_decorated_class_expression(
        &mut self,
    ) -> Result<Expression<'arena>, ParseError> {
        let start = self.current_pos().0;

        let decorators = self.parse_decorators()?;

        if *self.current_kind() != TokenKind::Keyword(KeywordKind::Class) {
            return Err(self.error_expected_after("'class'", "decorator"));
        }

        let mut expr = self.parse_class_expression()?;
        if let Expression::ClassExpression(class) = &mut expr {
            class.decorators = if decorators.is_empty() {
                None
            } else {
                Some(decorators.into_bump_slice())
            };
            // Extend the span to cover the leading decorators.
            class.span = Span::new(start as u32, class.span.end);
        }
        Ok(expr)
    }

    /// Parse a list of decorators: `@dec1 @dec2 ...`
    pub(in crate::parser) fn parse_decorators(
        &mut self,
    ) -> Result<bumpalo::collections::Vec<'arena, Decorator<'arena>>, ParseError> {
        let mut decorators = self.bvec();

        while *self.current_kind() == TokenKind::At {
            decorators.push(self.parse_decorator()?);
        }

        Ok(decorators)
    }

    /// Parse a single decorator: `@expression`, where the expression follows the
    /// restricted ES decorators grammar — see `parse_decorator_expression`.
    fn parse_decorator(&mut self) -> Result<Decorator<'arena>, ParseError> {
        let start = self.current_pos().0;

        // Consume '@'
        debug_assert!(*self.current_kind() == TokenKind::At);
        self.advance()?;

        // Parse the decorator expression under the restricted ES decorators
        // grammar (identifier / `.`-member chain / single call, or a
        // parenthesized full expression) — NOT a full `AssignmentExpression`, so
        // a trailing `*` or binary operator is left for the following construct
        // (e.g. a decorated generator method `@fn *a() {}`).
        let expression = self.parse_decorator_expression()?;

        // The decorator span covers a parenthesized expression's closing `)`
        // (`@(expr)`), which the paren-stripped expression span excludes
        let end = self.prev_token_end();

        Ok(Decorator {
            expression,
            span: Span::new(start as u32, end as u32),
        })
    }

    /// Parse an optional `extends <super_class>` heritage clause, shared by the
    /// class-declaration and class-expression parsers. The superclass is any
    /// `LeftHandSideExpression` (`parse_heritage_expression`); `extends Base<T>`
    /// splits into `Base` plus separate `super_type_parameters`. Returns
    /// `(None, None)` when there is no `extends`.
    fn parse_optional_extends_clause(
        &mut self,
    ) -> Result<
        (
            Option<&'arena Expression<'arena>>,
            Option<TSTypeParameterInstantiation<'arena>>,
        ),
        ParseError,
    > {
        if !matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Extends)
        ) {
            return Ok((None, None));
        }
        self.advance()?; // consume 'extends'
        let expr = self.parse_heritage_expression()?;
        let type_args = self.parse_optional_type_arguments()?;
        Ok((Some(expr), type_args))
    }

    /// Take an optional class name — a `BindingIdentifier`, so contextual type keywords
    /// (`class any {}`) are valid names and `await` is one only at Script `[~Await]` (both
    /// handled by `take_binding_identifier`). `implements` (a strict-mode reserved word, but
    /// a plain identifier token) can never be the name: directly after `class` it begins the
    /// extends-less `implements` clause of an anonymous class (`class implements Foo {}`), so
    /// it's excluded here (without advancing) and left for the heritage parser — where
    /// acorn-typescript rejects it. Returns `Ok(None)` for both `implements` and a
    /// non-binding token; the caller decides whether a missing name is an error (declaration)
    /// or fine (expression / `export default`). Shared by both class paths.
    fn take_class_name(&mut self) -> Result<Option<Identifier<'arena>>, ParseError> {
        if self.current_value() == "implements" {
            return Ok(None);
        }
        self.take_binding_identifier()
    }

    /// Parse a class expression: `class { }` or `class Foo<T> extends Bar { }`
    ///
    /// Class expressions are similar to class declarations but:
    /// - The name is always optional
    /// - They appear in expression position
    /// - No `declare` field
    pub fn parse_class_expression(&mut self) -> Result<Expression<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'class' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Class)
        ));
        self.advance()?;

        // Parse optional class name (`implements` excluded — it begins the heritage
        // clause of an anonymous class; see `take_class_name`).
        let id = self.take_class_name()?;

        // Parse type parameters (TypeScript generics): class Foo<T>
        let type_parameters = self.parse_optional_type_parameters()?;

        // Parse optional `extends` clause
        let (super_class, super_type_parameters) = self.parse_optional_extends_clause()?;

        // Parse optional `implements` clause
        let implements: &'arena [_] = if self.eat_contextual_keyword("implements") {
            self.parse_interface_heritage_list()?.into_bump_slice()
        } else {
            &[]
        };

        // Parse class body
        let body = self.parse_class_body()?;
        let end = body.span.end;

        Ok(Expression::ClassExpression(ClassExpression {
            decorators: None,
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
    ) -> Result<ClassDeclaration<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.parse_class_declaration_inner_with_start(name_required, is_abstract, start, false)
    }

    /// Parse a class declaration from the `class` keyword. `declare` marks an
    /// ambient (`declare class`) declaration: it sets the `declare` field. Members
    /// (bodies, decorators, static blocks) are parsed exactly like a concrete class
    /// (see `finish_method_member`); their ambient-context early-errors (a body's
    /// TS1183, a decorator's TS1206) are deferred to the diagnostics layer — the same
    /// permissive posture tsv takes on ambient field initializers (TS1039).
    pub(super) fn parse_class_declaration_inner_with_start(
        &mut self,
        name_required: bool,
        is_abstract: bool,
        start: usize,
        declare: bool,
    ) -> Result<ClassDeclaration<'arena>, ParseError> {
        // Consume 'class' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Class)
        ));
        self.advance()?;

        // Parse class name (required for declarations, optional for export default; see
        // `take_class_name`). `export default class implements Foo {}` (spec-valid, name
        // optional) parses where acorn-typescript rejects it (`implements` reserved); a
        // `name_required` declaration (`class implements Foo {}` / no name) still errors.
        let id = self.take_class_name()?;
        if name_required && id.is_none() {
            return Err(self.error_expected_after("class name", "class"));
        }

        // Parse type parameters (TypeScript generics): class Foo<T>()
        let type_parameters = self.parse_optional_type_parameters()?;

        // Parse optional `extends` clause
        let (super_class, super_type_parameters) = self.parse_optional_extends_clause()?;

        // Parse optional `implements` clause
        let implements: &'arena [_] = if self.eat_contextual_keyword("implements") {
            self.parse_interface_heritage_list()?.into_bump_slice()
        } else {
            &[]
        };

        // Parse class body (ambient members share the concrete grammar; see `parse_class_body`)
        let body = self.parse_class_body()?;
        let end = body.span.end;

        Ok(ClassDeclaration {
            decorators: None,
            id,
            super_class,
            super_type_parameters,
            implements,
            body,
            declare,
            r#abstract: is_abstract,
            type_parameters,
            span: Span::new(start as u32, end),
        })
    }

    /// Parse a class body. Concrete and ambient (`declare class`) bodies share one
    /// grammar — keys, modifiers, decorators, type parameters, return types, method
    /// bodies, index/accessor signatures, static blocks. An ambient member's
    /// forbidden constructs (a method body, a decorator, an initializer) parse
    /// structurally; their early-errors are deferred to diagnostics (see
    /// `parse_class_declaration_inner_with_start`).
    pub(super) fn parse_class_body(&mut self) -> Result<ClassBody<'arena>, ParseError> {
        let (start, _) = self.current_pos();
        self.expect(&TokenKind::BraceOpen)?;

        // A class body is a fresh `[+In]` context: computed member names
        // (`[+In]`), field initializers (`[+In]`), method/getter/setter bodies,
        // and static blocks all permit `in` even when the class expression sits
        // in a for-header init. A nested for-header inside re-disables it.
        let body = self.with_allow_in(|p| {
            let mut body = p.bvec();
            while !matches!(p.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
                // Stray semicolons are empty class members — acorn skips them,
                // producing no node (prettier strips them on format).
                if p.eat(TokenKind::Semicolon) {
                    continue;
                }
                let member = p.parse_class_member()?;
                body.push(member);
            }
            Ok(body)
        })?;

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(ClassBody {
            body: body.into_bump_slice(),
            span: Span::new(start as u32, end as u32),
        })
    }

    /// Parse the optional (`?`) / definite (`!`) marker that follows a class
    /// member key, before any type parameters: `m?<T>()`, `prop?: T`, `prop!: T`.
    /// The two are mutually exclusive (same syntactic slot). Shared by the method
    /// and property branches of `parse_class_member` (concrete and ambient alike).
    pub(in crate::parser) fn parse_property_modifier(&mut self) -> PropertyModifier {
        if self.eat(TokenKind::Question) {
            PropertyModifier::Optional
        } else if self.eat(TokenKind::Bang) {
            PropertyModifier::Definite
        } else {
            PropertyModifier::None
        }
    }

    /// Consume the contextual keyword `kw` as a class-member modifier iff the
    /// current token is the identifier `kw` and the next token begins a member
    /// name or a generator `*` — otherwise `kw` is itself the member's name
    /// (e.g. `readonly = 1`). Returns whether it was consumed. Shared by the
    /// `declare`/`override`/`abstract`/`readonly` detectors; `declare` is
    /// probed in three positions (before/after accessibility, after `static`).
    #[inline]
    fn eat_modifier_keyword(&mut self, kw: &str) -> bool {
        if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == kw
            && (self.peek_is_class_member_name() || self.peek_is(&TokenKind::Star))
        {
            self.advance().ok();
            true
        } else {
            false
        }
    }

    /// Parse a single class member. Concrete and ambient (`declare class`) members
    /// share this one parser: decorators, method bodies, and static blocks all parse
    /// structurally in either context, deferring their ambient-context early-errors
    /// (a decorator's TS1206, a body's TS1183) to diagnostics — see
    /// `parse_class_declaration_inner_with_start` and `finish_method_member`.
    fn parse_class_member(&mut self) -> Result<ClassMember<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Parse any decorators on this member. Ambient (`declare class`) members parse
        // decorators too — acorn accepts them and prettier formats the body form; the
        // TS1206 ("Decorators are not valid here") ambient early-error is deferred to
        // the diagnostics layer.
        let decorators = self.parse_decorators()?.into_bump_slice();

        // Handle 'declare' contextual keyword - only if followed by a class member name or another modifier
        // Otherwise `declare` itself is the property name: `declare = 1;`
        // Note: declare must be parsed BEFORE accessibility because `declare public a` is valid
        let is_declare = self.eat_modifier_keyword("declare");

        // Handle accessibility modifiers (public, private, protected). Like the
        // other shared detectors, each is consumed only if followed by a class
        // member name or `*` (generator); otherwise the keyword itself is the
        // property name (`private = 1;`).
        let accessibility = if self.eat_modifier_keyword("public") {
            Some(Accessibility::Public)
        } else if self.eat_modifier_keyword("private") {
            Some(Accessibility::Private)
        } else if self.eat_modifier_keyword("protected") {
            Some(Accessibility::Protected)
        } else {
            None
        };

        // Handle 'declare' after accessibility (for non-canonical order like `public declare a`)
        let is_declare = is_declare || self.eat_modifier_keyword("declare");

        // Handle 'static' contextual keyword - only if followed by a class member name or `{` (static block)
        // Otherwise `static` itself is the property name: `static = 2;`
        // Stays inline: unlike the shared detectors, `static {` (a static block) is also a valid follow.
        let is_static = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "static"
            && (self.peek_is_class_member_name()
                || self.peek_is(&TokenKind::BraceOpen)
                || self.peek_is(&TokenKind::Star))
        {
            self.advance().ok();
            true
        } else {
            false
        };

        // Handle 'declare' after static (for non-canonical order like `static declare a`)
        let is_declare = is_declare || self.eat_modifier_keyword("declare");

        // Check for static initialization block: `static { ... }` (ES2022).
        // Parsed in ambient (`declare class`) context too, as a `StaticBlock` — the
        // same defer-the-early-error posture as a `declare class` method body below
        // (tsc rejects an ambient implementation with TS1183; tsv defers it to
        // diagnostics rather than failing to parse/format).
        if is_static && matches!(self.current_kind(), TokenKind::BraceOpen) {
            // Parse the block body. A class static initialization block is a
            // `[+Await]` context (`await` is allowed inside it).
            let block = self.with_in_await(true, Self::parse_block_statement)?;
            let end = block.span.end;

            return Ok(ClassMember::StaticBlock(StaticBlock {
                body: block.body,
                span: Span::new(start as u32, end),
            }));
        }

        // Handle 'override' contextual keyword - only if followed by a class member name or `*`
        // Otherwise `override` itself is the property name: `override = 1;`
        let is_override = self.eat_modifier_keyword("override");

        // Handle 'abstract' contextual keyword - only if followed by a class member name or `*`
        // Otherwise `abstract` itself is the property name: `abstract = 1;`
        let is_abstract = self.eat_modifier_keyword("abstract");

        // Handle 'readonly' contextual keyword - only if followed by a class member name
        // Otherwise `readonly` itself is the property name: `readonly = 1;`
        let readonly = self.eat_modifier_keyword("readonly");

        // Handle 'accessor' contextual keyword (ES decorator proposal)
        // Only consume as modifier if followed by a class member name.
        // Stays inline: `accessor` may not prefix a generator, so it omits the `*` follow.
        let accessor = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "accessor"
            && self.peek_is_class_member_name()
        {
            self.advance().ok();
            true
        } else {
            false
        };

        // Handle 'async' keyword for async methods
        // async is only a modifier if followed by: identifier, [, #, or *
        // Stays inline: `async` is a reserved Keyword token, not an Identifier.
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

        // Check for index signature: [key: Type]: ValueType
        // Index signatures look like `[ident: Type]` followed by `: ValueType`
        if self.is_index_signature_start() {
            return self.parse_class_index_signature(start, is_static, readonly);
        }

        // Parse member name (key)
        let (computed, key, name_is_constructor) =
            if matches!(self.current_kind(), TokenKind::BracketOpen) {
                // Computed key: [expr] — never the constructor (`['constructor']` is a
                // normal computed method, unlike the bare/string `constructor`).
                self.advance()?;
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::BracketClose)?;
                (true, expr, false)
            } else if matches!(self.current_kind(), TokenKind::Hash) {
                // Private identifier key: #name
                let private_id = self.parse_private_identifier()?;
                (false, Expression::PrivateIdentifier(private_id), false)
            } else if self.current_is_identifier_or_keyword() {
                // Identifier or keyword as key - keywords are valid as class member names.
                // The constructor is matched by decoded StringValue, so an escaped
                // `constructor` is the constructor too (ecma262 ClassElementName) — acorn parity.
                let name_is_constructor = self
                    .current_decoded()
                    .unwrap_or_else(|| self.current_property_name())
                    == "constructor";
                let (key_start, key_end) = self.current_pos();
                // Member keys decode `\u` escapes (span-identity otherwise) — acorn parity.
                let key_name = self.current_ident_name();
                self.advance()?;
                (
                    false,
                    Expression::Identifier(Identifier::simple(
                        key_name,
                        Span::new(key_start as u32, key_end as u32),
                    )),
                    name_is_constructor,
                )
            } else if matches!(self.current_kind(), TokenKind::String) {
                // String literal key: `'a-b'() {}`, `'a' = 1;`. A string-keyed
                // `'constructor'` IS the constructor (kind detection reads the name),
                // unlike a computed `['constructor']`. The check goes through the
                // decoded value, so `'constructor'` is recognized too.
                let (key_start, key_end) = self.current_pos();
                let span = Span::new(key_start as u32, key_end as u32);
                let cooked = self.extract_string_cooked();
                self.advance()?;
                let name_is_constructor = self.resolve_cooked(&cooked, span) == "constructor";
                (
                    false,
                    Expression::Literal(Literal {
                        value: LiteralValue::String(cooked),
                        span,
                    }),
                    name_is_constructor,
                )
            } else if matches!(self.current_kind(), TokenKind::Number) {
                // Numeric key: `0() {}`, `0xb_b = 1;`, `1n;` — shares the full
                // numeric decode (radix, separators, bigint)
                let literal = self.parse_number_or_bigint_literal()?;
                self.advance()?;
                (false, Expression::Literal(literal), false)
            } else {
                return Err(self.error_expected("class member name"));
            };

        // Optional (`?`) / definite (`!`) marker, between the key and any type
        // parameters — methods read `?` as `optional`, properties keep the full modifier.
        let modifier = self.parse_property_modifier();

        // Parse type parameters (TypeScript generics): method<T>()
        let type_parameters = self.parse_optional_type_parameters()?;

        let header = ClassMemberHeader {
            start,
            decorators,
            accessibility,
            is_static,
            is_declare,
            is_override,
            is_abstract,
            readonly,
            accessor,
            is_generator,
            is_async,
            accessor_kind,
            computed,
            key,
            name_is_constructor,
            modifier,
            type_parameters,
        };

        // Detect if this is a method (has `(`) or property (has `=` or `;` or end of class)
        if matches!(self.current_kind(), TokenKind::ParenOpen) {
            self.finish_method_member(header)
        } else {
            self.finish_property_member(header)
        }
    }

    /// Finish a method member (the `(`-led branch of `parse_class_member`):
    /// parameter list, optional return type, and body — or a bodiless signature
    /// for abstract methods and overload signatures (`;`/ASI-terminated, including
    /// ambient signatures).
    fn finish_method_member(
        &mut self,
        header: ClassMemberHeader<'arena>,
    ) -> Result<ClassMember<'arena>, ParseError> {
        let ClassMemberHeader {
            start,
            decorators,
            accessibility,
            is_static,
            is_override,
            is_abstract,
            is_generator,
            is_async,
            accessor_kind,
            computed,
            key,
            name_is_constructor,
            modifier,
            type_parameters,
            ..
        } = header;

        // Method definition - use accessor_kind if set, otherwise check for
        // constructor. A `static` method named `constructor` is NOT the class
        // constructor (that name is only reserved on instance methods), so it
        // stays `MethodKind::Method` — matching acorn.
        let kind = accessor_kind.unwrap_or(if name_is_constructor && !is_static {
            MethodKind::Constructor
        } else {
            MethodKind::Method
        });

        // Capture paren position before parsing params (for comment detection)
        let (params_start, _) = self.current_pos();

        // Parse parameter list and block body (like a function), in the method's
        // own `[Await]` context (async method → `[+Await]`, else `[~Await]`).
        let params = self
            .with_in_await(is_async, Self::parse_parameter_list)?
            .into_bump_slice();

        // Check for return type annotation: (): type or type predicate
        let return_type = self.parse_optional_return_type()?;

        // Abstract methods and overload signatures have no body - just a semicolon
        // Method overloads: `parse(x: string): object;` followed by implementation
        // Note: ASI can insert semicolon on line terminator, but NOT if next token is `{`
        // (a method with body on next line: `fn()\n{` is valid)
        // Ambient (`declare class`) members are NOT forced bodiless: a `{` body parses
        // like a concrete method. tsc rejects an ambient implementation with the
        // grammar-level TS1183 ("An implementation cannot be declared in ambient
        // contexts"), but tsv *defers* that early-error to the future diagnostics layer
        // — the same posture it takes on ambient field initializers (TS1039) — so the
        // formatter stays complete on inputs prettier also formats. Only a `;`/ASI
        // member stays a bodiless signature, exactly as in a concrete class.
        let is_overload_or_abstract = is_abstract
            || self.check(&TokenKind::Semicolon)
            || (self.can_insert_semicolon() && !self.check(&TokenKind::BraceOpen));
        let (body_block, end) = if is_overload_or_abstract {
            // Without a return type the signature ends at the params' `)` —
            // the next token's start would overshoot past trailing comments
            // or onto the next line under ASI
            let body_end = return_type
                .as_ref()
                .map_or_else(|| self.prev_token_end() as u32, |rt| rt.span.end);
            let end = if self.eat(TokenKind::Semicolon) {
                self.prev_token_end() as u32
            } else {
                body_end
            };
            // Create empty body for abstract methods and overload signatures
            (
                BlockStatement {
                    body: &[],
                    span: Span::new(body_end, body_end),
                },
                end,
            )
        } else {
            let body_block = self.with_in_await(is_async, Self::parse_function_body)?;
            let end = body_block.span.end;
            (body_block, end)
        };

        // Create FunctionExpression for the method value
        // span starts at params_start (the `(`) to match acorn's FunctionExpression positioning
        let value = FunctionExpression {
            id: None,
            type_parameters,
            params,
            return_type,
            body: body_block,
            generator: is_generator,
            r#async: is_async,
            params_start: params_start as u32,
            span: Span::new(params_start as u32, end),
        };

        Ok(ClassMember::MethodDefinition(MethodDefinition {
            decorators: if decorators.is_empty() {
                None
            } else {
                Some(decorators)
            },
            key,
            value,
            kind,
            accessibility,
            is_static,
            r#override: is_override,
            r#abstract: is_abstract,
            computed,
            optional: matches!(modifier, PropertyModifier::Optional),
            span: Span::new(start as u32, end),
        }))
    }

    /// Finish a property member (the non-`(` branch of `parse_class_member`):
    /// optional type annotation, optional initializer, and trailing semicolon.
    fn finish_property_member(
        &mut self,
        header: ClassMemberHeader<'arena>,
    ) -> Result<ClassMember<'arena>, ParseError> {
        let ClassMemberHeader {
            start,
            decorators,
            accessibility,
            is_static,
            is_declare,
            is_override,
            is_abstract,
            readonly,
            accessor,
            computed,
            key,
            modifier,
            ..
        } = header;

        // Property definition: `name: type = value;` or `name: type;` or `name = value;` or `name;`
        // The optional/definite marker was already parsed above (shared with methods).

        // Check for type annotation: `name: type`
        let type_annotation = self.parse_optional_type_annotation()?;

        // Check for value: `= value`. A class field `Initializer` is `[~Await]`
        // (it does not inherit a `[+Await]` enclosing — `await` is not an
        // await-expression there, only an identifier under Script goal), unlike
        // a computed key, which inherits.
        let value: Option<Expression<'arena>> = if self.eat(TokenKind::Equals) {
            Some(self.with_in_await(false, Self::parse_assignment_expression)?)
        } else {
            None
        };

        let end = value.as_ref().map_or_else(
            || {
                type_annotation
                    .as_ref()
                    // No type annotation/value: the last consumed token is the key
                    // (its closing `]` for a computed key) or the `?`/`!` modifier.
                    // `key.span().end` would stop inside a computed key's brackets,
                    // so read the previous token's end instead.
                    .map_or_else(|| self.prev_token_end() as u32, |ta| ta.span.end)
            },
            // prev_token_end covers a parenthesized value's closing `)`,
            // which the paren-stripped value span excludes
            |_| self.prev_token_end() as u32,
        );

        // Consume optional semicolon (ASI applies), including it in the span
        let mut end = end;
        if self.eat(TokenKind::Semicolon) {
            end = self.prev_token_end() as u32;
        }

        Ok(ClassMember::PropertyDefinition(PropertyDefinition {
            decorators: if decorators.is_empty() {
                None
            } else {
                Some(decorators)
            },
            key,
            type_annotation,
            value,
            accessibility,
            is_static,
            declare: is_declare,
            r#abstract: is_abstract,
            r#override: is_override,
            readonly,
            computed,
            accessor,
            modifier,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse an index signature: `[key: KeyType]: ValueType` or `readonly [key: KeyType]: ValueType`
    pub(in crate::parser) fn parse_class_index_signature(
        &mut self,
        start: usize,
        is_static: bool,
        readonly: bool,
    ) -> Result<ClassMember<'arena>, ParseError> {
        // Consume `[`
        self.expect(&TokenKind::BracketOpen)?;

        // Parse parameter: `key: KeyType`
        let (id_start, id_end) = self.current_pos();
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected("index signature parameter name"));
        }
        let param_name = self.current_ident_name();
        self.advance()?;

        // Parse type annotation on parameter: `: KeyType`
        let param_type = if self.check(&TokenKind::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            return Err(self.error_expected("type annotation for index signature parameter"));
        };

        let param_end = param_type.as_ref().map_or(id_end, |t| t.span.end as usize);
        let extra = param_type.map(|ta| self.typed_extra(ta));
        let parameter = Identifier {
            escaped_name: param_name.escaped,
            name_len: param_name.raw_len,
            optional: false,
            extra,
            span: Span::new(id_start as u32, param_end as u32),
        };

        // Consume `]`
        self.expect(&TokenKind::BracketClose)?;

        // Parse value type annotation: `: ValueType`
        let value_type = if self.check(&TokenKind::Colon) {
            self.parse_type_annotation()?
        } else {
            return Err(self.error_expected("type annotation for index signature value"));
        };

        // Consume semicolon, including it in the span
        let mut end = value_type.span.end;
        if self.eat(TokenKind::Semicolon) {
            end = self.prev_token_end() as u32;
        }

        let mut parameters = self.bvec();
        parameters.push(parameter);
        Ok(ClassMember::IndexSignature(TSIndexSignature {
            parameters: parameters.into_bump_slice(),
            type_annotation: value_type,
            is_static,
            readonly,
            span: Span::new(start as u32, end),
        }))
    }
}
