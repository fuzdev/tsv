// TypeScript type declaration parsing: type aliases, interfaces, enums,
// namespaces/modules, and `declare` statements. Type *expression* syntax
// (annotations, unions, object types, type parameters) lives in
// `parser/types.rs`.

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;

/// End offset of a module declaration body (block or nested declaration).
fn module_body_end(body: &TSModuleDeclarationBody<'_>) -> u32 {
    match body {
        TSModuleDeclarationBody::TSModuleBlock(b) => b.span.end,
        TSModuleDeclarationBody::TSModuleDeclaration(n) => n.span.end,
    }
}

impl<'a, 'arena> Parser<'a, 'arena> {
    pub(super) fn parse_type_alias_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // Consume 'type' contextual keyword
        debug_assert!(self.current_value() == "type");
        self.advance()?;

        let decl = self.parse_type_alias_declaration_body(start, false)?;
        Ok(Statement::TSTypeAliasDeclaration(decl))
    }

    /// Parse type alias declaration with an external start position (for `declare type`)
    fn parse_type_alias_declaration_with_start(
        &mut self,
        start: usize,
    ) -> Result<Statement<'arena>, ParseError> {
        // Consume 'type' contextual keyword
        debug_assert!(self.current_value() == "type");
        self.advance()?;

        let decl = self.parse_type_alias_declaration_body(start, true)?;
        Ok(Statement::TSTypeAliasDeclaration(decl))
    }

    /// Parse type alias declaration inner - assumes 'type' keyword already consumed
    /// Used by export type X = T when 'type' is consumed to check for { vs identifier
    /// `type_start` is the position of the 'type' keyword (captured before advancing)
    pub(super) fn parse_type_alias_declaration_inner(
        &mut self,
        type_start: usize,
    ) -> Result<Statement<'arena>, ParseError> {
        let decl = self.parse_type_alias_declaration_body(type_start, false)?;
        Ok(Statement::TSTypeAliasDeclaration(decl))
    }

    /// Parse type alias body - the part after 'type' keyword
    fn parse_type_alias_declaration_body(
        &mut self,
        start: usize,
        declare: bool,
    ) -> Result<TSTypeAliasDeclaration<'arena>, ParseError> {
        // Parse type name — a `BindingIdentifier`, so contextual type keywords
        // (`type any = …`) are valid names, matching acorn/tsc.
        let Some(id) = self.take_binding_identifier()? else {
            return Err(self.error_expected_after("type name", "type"));
        };

        // Parse optional type parameters: <T, U>
        let type_parameters = self.parse_optional_type_parameters()?;

        // Expect '='
        self.expect(&TokenKind::Equals)?;

        // Parse the type
        let type_annotation = self.parse_type()?;
        let end = self.semicolon_end()?;

        Ok(TSTypeAliasDeclaration {
            id,
            type_parameters,
            type_annotation,
            declare,
            span: Span::new(start as u32, end),
        })
    }

    //
    // Interface Declaration
    //

    /// Parse interface declaration: `interface Foo { ... }` or `interface Foo extends Bar { ... }`
    pub(super) fn parse_interface_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.parse_interface_declaration_body(start, false)
    }

    /// Parse interface declaration with an external start position (for `declare interface`)
    fn parse_interface_declaration_with_start(
        &mut self,
        start: usize,
    ) -> Result<Statement<'arena>, ParseError> {
        self.parse_interface_declaration_body(start, true)
    }

    /// Parse interface declaration body - assumes start position is set, consumes from `interface` keyword
    fn parse_interface_declaration_body(
        &mut self,
        start: usize,
        declare: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        Ok(Statement::TSInterfaceDeclaration(
            self.parse_interface_declaration_struct(start, declare)?,
        ))
    }

    /// Parse an interface declaration into its struct, without wrapping in
    /// `Statement` — reused by `export default interface Foo {}`, where the
    /// interface is an `ExportDefaultValue` rather than a top-level statement.
    pub(super) fn parse_interface_declaration_struct(
        &mut self,
        start: usize,
        declare: bool,
    ) -> Result<TSInterfaceDeclaration<'arena>, ParseError> {
        // Consume 'interface' contextual keyword
        debug_assert!(self.current_value() == "interface");
        self.advance()?;

        // Parse interface name — a `BindingIdentifier`, so contextual type keywords
        // (`interface string {}`) are valid names, matching acorn/tsc.
        let Some(id) = self.take_binding_identifier()? else {
            return Err(self.error_expected_after("interface name", "interface"));
        };

        // Parse optional type parameters: <T, U>
        let type_parameters = self.parse_optional_type_parameters()?;

        // Parse optional extends clause
        let extends: &'arena [TSInterfaceHeritage<'arena>] =
            if self.check(&TokenKind::Keyword(KeywordKind::Extends)) {
                self.advance()?;
                self.parse_interface_heritage_list()?.into_bump_slice()
            } else {
                &[]
            };

        // Parse interface body
        let body = self.parse_interface_body()?;
        let end = body.span.end;

        Ok(TSInterfaceDeclaration {
            id,
            type_parameters,
            extends,
            body,
            declare,
            span: Span::new(start as u32, end),
        })
    }

    /// Parse interface heritage list: `Foo, Bar<T>`
    pub(in crate::parser) fn parse_interface_heritage_list(
        &mut self,
    ) -> Result<bumpalo::collections::Vec<'arena, TSInterfaceHeritage<'arena>>, ParseError> {
        let mut heritages = self.bvec();

        loop {
            let start = self.current_pos().0;

            // A heritage element is a type REFERENCE, whose head is an
            // `IdentifierReference` — so a contextual type keyword is an ordinary
            // name here (`interface A extends number {}`, `class C implements string {}`),
            // and primitive-ness is the checker's business, not the parser's. A bare
            // `TokenKind::Identifier` test saw only the words the lexer never made a
            // `Keyword`, rejecting every predefined-type name.
            //
            // The reserved words stay out: prettier states the same rule outright
            // ("can only extend an identifier/qualified name with optional type
            // arguments") and refuses `null` / `true` / `this`, and tsc rejects `void`
            // at parse (TS1109) — see the `heritage_reserved_keyword_svelte_divergence`
            // sibling, which pins the `void` line against acorn, who accepts all four.
            //
            // `at_reference_name`, not `at_binding_name`: this head is an
            // `IdentifierReference`, so `yield`/`await` keep the `[~Yield]`/`[~Await]`
            // guards their production carries rather than the deferral the binding
            // channel gets. tsc lands in the same place by a different route — it
            // parses heritage with its *expression* parser, where both words are the
            // operator — so `function* g() { interface A extends yield {} }` and
            // `async function h() { interface A extends await {} }` are TS1109 for it
            // and rejections here, while at the top level both names are fine.
            if !self.at_reference_name() {
                return Err(self.error_expected("a type name in the heritage clause"));
            }

            let expression = self.parse_type_entity_name()?;

            // Check for type arguments
            let type_arguments = self.parse_optional_type_arguments()?;

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
    fn parse_interface_body(&mut self) -> Result<TSInterfaceBody<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.expect(&TokenKind::BraceOpen)?;

        let body = self.parse_type_members()?;

        let (_, end) = self.current_pos();
        self.expect(&TokenKind::BraceClose)?;

        Ok(TSInterfaceBody {
            body: body.into_bump_slice(),
            span: Span::new(start as u32, end as u32),
        })
    }

    //
    // Declare Statement
    //

    /// Parse declare statement: `declare function`, `declare class`, `declare enum`, `declare const enum`, `declare namespace`, `declare global`, `declare var/let/const`
    pub(super) fn parse_declare_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        let start = self.current_pos().0;

        // Consume 'declare' contextual keyword
        debug_assert!(self.current_value() == "declare");
        self.advance()?;

        // Everything under `declare` parses in ambient context (acorn/babel
        // `inAmbientContext`) — notably a single trailing comma after a rest
        // parameter is tolerated anywhere in the subtree (parameter lists,
        // function types, interface/type-literal members); see the rest-comma
        // checks in `parameters.rs`/`types.rs`.
        self.with_context_flag(
            |p| &mut p.in_ambient_context,
            true,
            |p| p.parse_declare_statement_kind(start),
        )
    }

    /// The post-`declare` dispatch, run inside ambient context.
    fn parse_declare_statement_kind(
        &mut self,
        start: usize,
    ) -> Result<Statement<'arena>, ParseError> {
        if self.had_line_terminator {
            // `declare [no LineTerminator here] <declaration>`, uniform across all eight
            // heads below. The statement path decides this one token earlier
            // (`peek_starts_ambient_declaration`), where a break DEMOTES `declare` to an
            // expression statement and the declaration stands alone — so this arm is
            // unreachable from it. The `export` path dispatches straight here, and there a
            // break has no valid reading: tsc rejects every `export declare⏎<kind>` with
            // TS1128, and so does prettier. acorn instead welds across the break, which is
            // the divergence this rejection accepts deliberately — the drop-in oracle is
            // for AST *shape*, not for a verdict tsc and prettier both refuse.
            return Err(self.error_msg("declaration must be on the same line as 'declare'"));
        }
        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Function) => self.parse_declare_function(start, false),
            TokenKind::Keyword(KeywordKind::Async) => {
                // `declare async function f(): Promise<void>;` — tsc's parser builds
                // one `FunctionDeclaration` with `[DeclareKeyword, AsyncKeyword]`
                // modifiers and no `parseDiagnostics`; the prohibition is TS1040
                // ("'async' modifier cannot be used in an ambient context"), a
                // checker grammar error of the ambient-context family tsv defers.
                // acorn accepts this only behind `export` and rejects it bare — an
                // inconsistency, not a judgement — so tsc's verdict wins here.
                self.advance()?;
                if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Function)) {
                    return Err(self.error_expected_after("'function'", "declare async"));
                }
                if self.had_line_terminator {
                    // `async [no LineTerminator here] function`. The bare `declare` path
                    // never arrives here across a break — `peek_starts_ambient_declaration`
                    // already declined the ambient reading — but `export declare`
                    // dispatches without that gate, so the rule is re-asked rather than
                    // assumed. tsc rejects this spelling too (TS1128).
                    return Err(
                        self.error_msg("'function' must be on the same line as 'async'")
                    );
                }
                self.parse_declare_function(start, true)
            }
            TokenKind::Keyword(KeywordKind::Class) => self.parse_declare_class(start, false),
            TokenKind::Identifier if self.current_value() == "abstract" => {
                // declare abstract class
                self.advance()?;
                if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Class)) {
                    return Err(self.error_expected_after("'class'", "declare abstract"));
                }
                if self.had_line_terminator {
                    // `abstract [no LineTerminator here] class`, the rule every other
                    // modifier gap here already honors. Without it `declare abstract⏎class
                    // B {}` welded into ONE ambient class — a reading NO oracle endorses
                    // (acorn rejects; tsc splits into three statements), and a silent one:
                    // the merged output is stable and reparses, so only a prettier
                    // comparison could ever have seen it.
                    //
                    // tsv rejects rather than reproducing tsc's split, because that split
                    // rests on a semicolon ECMAScript does not insert — `declare` and
                    // `abstract` sit on ONE line, and none of the three ASI conditions
                    // hold (ecma262 §sec-rules-of-automatic-semicolon-insertion: a
                    // preceding LineTerminator, a `}`, or the do-while `)`). tsc grants it
                    // to `declare` ALONE — `abstract`/`async`/`public`/`export`/`readonly`
                    // all reject the identical shape — so it is an oracle slip rather than
                    // a rule, and following it would make tsv's grammar, not merely its
                    // early-error policy, diverge from the spec. acorn agrees with the
                    // rejection. See docs/conformance_prettier_ts.md §tsv rejects what
                    // prettier formats.
                    return Err(self.error_msg("'class' must be on the same line as 'abstract'"));
                }
                self.parse_declare_class(start, true)
            }
            TokenKind::Keyword(KeywordKind::Enum) => {
                // declare enum
                self.parse_enum_declaration_with_start(false, true, start)
            }
            TokenKind::Keyword(KeywordKind::Const) => {
                // declare const enum OR declare const variable
                if self.peek_kind() == TokenKind::Keyword(KeywordKind::Enum) {
                    self.parse_enum_declaration_with_start(true, true, start)
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
                // declare namespace/module — the name must be on the SAME line; see
                // `require_same_line_declaration_name`, which the `interface`/`type` arm
                // below shares so the four contextual heads cannot drift apart.
                self.require_same_line_declaration_name(self.current_value())?;
                self.parse_module_declaration_with_start(true, start)
            }
            TokenKind::Identifier if self.current_value() == "interface" => {
                // declare interface — the same rule the `namespace`/`module` arm above
                // asks, for another of the four contextual heads.
                self.require_same_line_declaration_name("interface")?;
                self.parse_interface_declaration_with_start(start)
            }
            TokenKind::Identifier if self.current_value() == "type" => {
                // declare type — see the `interface` arm above.
                self.require_same_line_declaration_name("type alias")?;
                self.parse_type_alias_declaration_with_start(start)
            }
            TokenKind::Identifier if self.current_value() == "global" => {
                // declare global { }
                self.parse_global_declaration(start, true)
            }
            _ => Err(self.error_expected_after(
                "'function', 'class', 'enum', 'const', 'let', 'var', 'namespace', 'module', 'interface', 'type', or 'global'",
                "declare",
            )),
        }
    }

    /// Parse declare variable: `declare const x: T;`, `declare let x: T;`, `declare var x: T;`
    fn parse_declare_variable(&mut self, start: usize) -> Result<Statement<'arena>, ParseError> {
        // Parse as a variable declaration but mark as declare
        let mut decl = self.parse_variable_declaration()?;

        // Mark as declare
        if let Statement::VariableDeclaration(ref mut var_decl) = decl {
            var_decl.declare = true;
            var_decl.span = Span::new(start as u32, var_decl.span.end);
        }

        Ok(decl)
    }

    /// Parse a top-level `declare function` — always a **bodiless** signature
    /// (`declare function foo(x: number): void;`). Called from `parse_declare_statement`
    /// where the `declare` keyword (and an `async` modifier, if any) is already
    /// consumed. The `declare` keyword
    /// grammatically forbids a body (tsc/prettier reject one), so `semicolon_end`
    /// requires `;`/ASI. A `function` *inside* a `declare namespace` body is NOT parsed
    /// here — it has no `declare` keyword of its own and goes through the ordinary
    /// function-statement path (`parse_statement`), which allows a body (deferring the
    /// ambient TS1183).
    ///
    /// A generator `*` and an `async` modifier are both **accepted and deferred**: tsc
    /// bars them from an ambient context with TS1221 / TS1040, but both are grammar
    /// errors its *checker* raises (`grammarErrorOnNode`), not parse errors — its parser
    /// builds the signature with `asteriskToken` / `AsyncKeyword` set and reports no
    /// `parseDiagnostics`. They join the ambient-context early errors tsv already defers,
    /// which is also what makes this position agree with every other one: the same
    /// bodiless `function*` signature already parses inside a `declare namespace`, a
    /// `declare global` and an overload set.
    fn parse_declare_function(
        &mut self,
        start: usize,
        is_async: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        // Consume 'function' keyword
        self.advance()?;

        // Check for generator: `declare function* g(): Iterator<T>;`
        let is_generator = self.eat(TokenKind::Star);

        // Parse function name. The shared `BindingIdentifier` channel, so an ambient
        // declaration takes the same names the concrete `function string() {}` form
        // already does — a bare `TokenKind::Identifier` test sees only the words the
        // lexer never made a `Keyword`, which is why `declare function get()` worked
        // while `declare function string()` did not.
        let Some(name) = self.try_function_name() else {
            return Err(self.error_expected("function name"));
        };

        let (id_start, id_end) = self.current_pos();
        self.advance()?;

        let id = Identifier::simple(name, Span::new(id_start as u32, id_end as u32));

        // Parse optional type parameters: <T, U>
        let type_parameters = self.parse_optional_type_parameters()?;

        // Parse parameters in the signature's own `[Await]`/`[Yield]` context, as the
        // overload path (`parse_function_or_overload`) does for the same node type.
        let params = self
            .with_fn_context(is_async, is_generator, Self::parse_parameter_list)?
            .into_bump_slice();

        // Parse return type (may be a type predicate)
        let return_type = self.parse_optional_return_type()?;

        let end = self.semicolon_end()?;

        Ok(Statement::TSDeclareFunction(TSDeclareFunction {
            id,
            type_parameters,
            params,
            return_type,
            declare: true,
            r#async: is_async,
            generator: is_generator,
            span: Span::new(start as u32, end),
        }))
    }

    /// The current token read as a type predicate's SUBJECT — an identifier or
    /// contextual keyword name, or `this` (`x is T`, `this is T`, `asserts x`).
    /// Asked twice per return type, once to decide whether an ordinary predicate
    /// outranks a leading `asserts` and once to take the name, so the set they agree on
    /// is spelled here rather than at each.
    fn try_type_predicate_subject(&self) -> Option<IdentName<'arena>> {
        self.try_ident_or_contextual_name()
            .or_else(|| self.this_as_name())
    }

    /// Parse a `: ReturnType` annotation when the next token is a `:`, else
    /// `None` — the optional-guard for function/method/signature return types
    /// (type predicates included via `parse_return_type_annotation`).
    pub(in crate::parser) fn parse_optional_return_type(
        &mut self,
    ) -> Result<Option<TSTypeAnnotation<'arena>>, ParseError> {
        if self.check(&TokenKind::Colon) {
            Ok(Some(self.parse_return_type_annotation()?))
        } else {
            Ok(None)
        }
    }

    /// Parse return type annotation, handling type predicates (`x is T`, `asserts x is T`)
    ///
    /// This expects the colon to NOT be consumed yet.
    pub(in crate::parser) fn parse_return_type_annotation(
        &mut self,
    ) -> Result<TSTypeAnnotation<'arena>, ParseError> {
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
    ) -> Result<TSTypeAnnotation<'arena>, ParseError> {
        // The predicate itself starts at the first token after `:`, not at `:`
        let predicate_start = self.current_pos().0 as u32;

        // An ORDINARY type predicate outranks the `asserts` modifier: a parameter may
        // itself be named `asserts` (`(asserts: unknown): asserts is string`), and only
        // the token after the name tells `asserts x` from `<name> is T`. Read that first,
        // so the modifier is never eaten out from under a predicate subject of the same
        // spelling.
        let plain_predicate =
            self.peek_predicate_is_ahead() && self.try_type_predicate_subject().is_some();

        // Check for a leading `asserts` assertion-predicate keyword. Eaten only
        // when an identifier or keyword follows (see `eat_type_predicate_asserts`);
        // a bare `asserts`, or one heading a regular type (`asserts[]`,
        // `asserts<T>`, `asserts.Foo`), stays unconsumed and is parsed below as an
        // ordinary type reference.
        let asserts = !plain_predicate && self.eat_type_predicate_asserts();

        // The predicate subject is an identifier/keyword name or `this`
        // (`x is T`, `this is T`, `asserts x`, `asserts this`).
        if let Some(param_name) = self.try_type_predicate_subject() {
            // Type predicate: `identifier is Type` or `asserts identifier is Type`.
            // The `is` must not be preceded by a line terminator
            // (`parameterName [no LineTerminator here] is Type`); see
            // `peek_predicate_is_ahead`.
            if self.peek_predicate_is_ahead() {
                let (id_start, id_end) = self.current_pos();
                self.advance()?;

                let parameter_name =
                    Identifier::simple(param_name, Span::new(id_start as u32, id_end as u32));

                // Consume 'is' keyword
                self.advance()?;

                // Parse the type
                let type_node = self.parse_type()?;
                let end = type_node.span().end;

                let predicate = TSTypePredicate {
                    parameter_name,
                    type_annotation: Some(self.alloc(type_node)),
                    asserts,
                    span: Span::new(predicate_start, end),
                };

                return Ok(TSTypeAnnotation {
                    type_annotation: self.alloc(TSType::TypePredicate(predicate)),
                    span: Span::new(start, end),
                });
            }

            // Asserts predicate: `asserts identifier`
            if asserts {
                let (id_start, id_end) = self.current_pos();
                self.advance()?;

                let parameter_name =
                    Identifier::simple(param_name, Span::new(id_start as u32, id_end as u32));

                let predicate = TSTypePredicate {
                    parameter_name,
                    type_annotation: None,
                    asserts: true,
                    span: Span::new(predicate_start, id_end as u32),
                };

                return Ok(TSTypeAnnotation {
                    type_annotation: self.alloc(TSType::TypePredicate(predicate)),
                    span: Span::new(start, id_end as u32),
                });
            }
        } else if asserts {
            // `asserts` committed (a keyword followed it) but the keyword is not a
            // valid parameter name (`asserts extends …`): reject, as tsc does when
            // its `parseIdentifier` hits a reserved word. A committed `asserts` is
            // only reached when an identifier/keyword follows, so any non-keyword
            // (punctuation/literal) case is instead handled as a plain type below.
            return Err(self.error_expected_after("identifier", "asserts"));
        }

        // Regular type annotation
        let type_node = self.parse_type()?;
        let end = type_node.span().end;

        Ok(TSTypeAnnotation {
            type_annotation: self.alloc(type_node),
            span: Span::new(start, end),
        })
    }

    /// Parse declare class: `declare class Foo { ... }` or `declare abstract class Foo { ... }`
    ///
    /// Parses through the shared `parse_class_declaration_inner_with_start` with
    /// the `declare` flag set, so the header (name, type parameters, heritage,
    /// `implements`) and the ambient body are handled by the same code as a
    /// concrete class — no parallel parser to drift. The caller has already
    /// consumed `declare` (and `abstract`); the current token is `class`.
    fn parse_declare_class(
        &mut self,
        start: usize,
        is_abstract: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        let class =
            self.parse_class_declaration_inner_with_start(true, is_abstract, start, true)?;
        Ok(Statement::ClassDeclaration(class))
    }

    //
    // Enum Declaration
    //

    /// Parse enum declaration: `enum Foo { A, B }`, `const enum Foo { A = 1 }`, etc.
    ///
    /// This wrapper handles the non-ambient forms (`enum`, `const enum`); the
    /// `declare` forms are parsed via `parse_enum_declaration_with_start` from
    /// `parse_declare_statement_kind`.
    pub(super) fn parse_enum_declaration(
        &mut self,
        is_const: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.parse_enum_declaration_with_start(is_const, false, start)
    }

    fn parse_enum_declaration_with_start(
        &mut self,
        is_const: bool,
        is_declare: bool,
        start: usize,
    ) -> Result<Statement<'arena>, ParseError> {
        // Consume 'const' if present
        if is_const {
            self.expect(&TokenKind::Keyword(KeywordKind::Const))?;
        }

        // Consume 'enum' keyword
        self.expect(&TokenKind::Keyword(KeywordKind::Enum))?;

        // Parse enum name — a `BindingIdentifier`, so contextual type keywords
        // (`enum string {}`) are valid names, matching acorn/tsc.
        let Some(id) = self.take_binding_identifier()? else {
            return Err(self.error_expected_after("enum name", "enum"));
        };

        // Parse enum body: { members }
        self.expect(&TokenKind::BraceOpen)?;

        let mut members = self.bvec();
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
            members: members.into_bump_slice(),
            r#const: is_const,
            declare: is_declare,
            span: Span::new(start as u32, end as u32),
        }))
    }

    /// Parse a single enum member: `A`, `A = 1`, `A = "value"`, `"computed" = 1`
    fn parse_enum_member(&mut self) -> Result<TSEnumMember<'arena>, ParseError> {
        let start = self.current_pos().0;

        // Parse member id: a `PropertyName` — identifier, keyword, or string
        // literal. Reserved words (`class`, `enum`, `function`, `default`, …) are
        // valid member names, same as object property keys (see
        // `expression_literals.rs`); the keyword token is never escaped, so its
        // name comes from the raw source (`current_raw_ident_name`).
        let id = match self.current_kind() {
            TokenKind::Identifier => {
                let (id_start, id_end) = self.current_pos();
                let name = self.current_ident_name();
                self.advance()?;
                TSEnumMemberId::Identifier(Identifier::simple(
                    name,
                    Span::new(id_start as u32, id_end as u32),
                ))
            }
            TokenKind::Keyword(_) => {
                let (id_start, id_end) = self.current_pos();
                let name = self.current_raw_ident_name();
                self.advance()?;
                TSEnumMemberId::Identifier(Identifier::simple(
                    name,
                    Span::new(id_start as u32, id_end as u32),
                ))
            }
            TokenKind::String => TSEnumMemberId::String(self.parse_string_literal()?),
            _ => {
                return Err(
                    self.error_expected("enum member name (identifier, keyword, or string)")
                );
            }
        };

        // Parse optional initializer: = value
        // Use assignment expression (not full expression) to stop at commas
        let (initializer, end) = if self.eat(TokenKind::Equals) {
            let expr = self.parse_assignment_expression()?;
            // The member ends at the last token consumed, NOT at the initializer's
            // span: tsv drops a grouping paren rather than building a node for it,
            // so a parenthesized initializer (`A = (a, b)`) leaves the expression
            // span stopping inside the shell while the member runs through the `)`.
            let end = self.prev_token_end() as u32;
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
    /// Handles the non-ambient forms:
    /// - `namespace Name { statements }`
    /// - `namespace Outer.Inner { statements }` (nested)
    /// - `module Name { statements }` (old syntax)
    ///
    /// The `declare` forms are parsed via `parse_module_declaration_with_start`
    /// from `parse_declare_statement_kind`.
    pub(super) fn parse_module_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.parse_module_declaration_with_start(false, start)
    }

    fn parse_module_declaration_with_start(
        &mut self,
        declare: bool,
        start: usize,
    ) -> Result<Statement<'arena>, ParseError> {
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

        // Parse module name — an identifier, or (module keyword only) a string
        // literal: `module 'name' { }` / `declare module 'name';`. acorn rejects
        // a string name after `namespace`.
        let id = if kind == TSModuleDeclarationKind::Module
            && matches!(self.current_kind(), TokenKind::String)
        {
            let lit = self.parse_string_literal()?;
            TSModuleName::Literal(lit)
        } else if let Some(ident) = self.take_binding_identifier()? {
            // The name is a `BindingIdentifier`, so contextual type keywords are
            // valid (`declare namespace string { … }`).
            // Check for nested namespace: `namespace Outer.Inner { }`
            if matches!(self.current_kind(), TokenKind::Dot) {
                return self.parse_nested_module_declaration(start as u32, ident, declare, kind);
            }

            TSModuleName::Identifier(ident)
        } else {
            return Err(self.error_expected("identifier or string literal for module name"));
        };

        // Parse body, or the shorthand (bodyless) form.
        //
        // Only an **external** module — one named by a string literal — has a
        // bodyless form (`declare module 'name';`), and its terminator follows ASI,
        // not a literal `;`: tsc's `parseAmbientExternalModuleDeclaration` ends it
        // with `parseSemicolon()` and acorn-typescript's
        // `tsParseAmbientExternalModuleDeclaration` with `semicolon()`, so a line
        // break, `}` or EOF closes it (`declare module 'jquery'` on its own line is
        // ordinary `.d.ts` authoring). An identifier-named namespace/module has no
        // shorthand in either oracle and always takes a block, so it falls through
        // to `parse_module_block` and its `'{'` error.
        let (body, end) = if matches!(id, TSModuleName::Literal(_))
            && !matches!(self.current_kind(), TokenKind::BraceOpen)
        {
            (None, self.semicolon_end()?)
        } else {
            // Full body: `{ statements }`
            let block = self.parse_module_block(declare)?;
            let end = module_body_end(&block);
            (Some(block), end)
        };

        Ok(Statement::TSModuleDeclaration(TSModuleDeclaration {
            id,
            body,
            declare,
            kind,
            global: false,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse a global augmentation: `declare global { … }` / `global { … }`, or the
    /// bodyless `declare global;`.
    ///
    /// `declare` is `true` for `declare global { }` and `false` for a bare
    /// `global { }` (top-level, or implicitly-ambient inside a `declare module`,
    /// where acorn omits the `declare` field). `start` is the keyword position
    /// the span begins at (`declare` for the declared form, `global` for the bare
    /// form). The body is parsed in ambient context when `declare` is set; a bare
    /// `global` nested in an already-ambient module keeps that context via
    /// `parse_module_block`'s save/restore.
    ///
    /// `global` is the identifier-named arm of the same ambient-module production the
    /// string-literal arm takes above, **including its bodyless form** — acorn's
    /// `tsParseAmbientExternalModuleDeclaration` reaches `if (braceL) body else
    /// semicolon()` for both names, and tsc's `parseAmbientExternalModuleDeclaration`
    /// is written identically. So a `declare global` closed by ASI is one bodyless
    /// `TSModuleDeclaration`, not two identifier expression statements — prettier's
    /// reading, which its `typescript` parser reaches only by *routing* around the
    /// production (tsc's `isDeclaration` requires `{`, an identifier or `export` after
    /// `global`) and which would need a semicolon inserted between `declare` and
    /// `global`, two words with no `LineTerminator` between them, where no ASI rule
    /// admits one. See `docs/conformance_prettier_ts.md` §TypeScript.
    ///
    /// **The bodyless arm is reachable only under `declare`**, matching both oracles:
    /// a bare `global` is a declaration head only when `{` follows it (acorn's
    /// `tsParseExpressionStatement`, mirrored by this parser's `peek_kind` gate in
    /// `parse_statement`), so `global;` stays an ordinary expression statement — at the
    /// top level and inside an ambient module alike. That is the caller's gate, not
    /// this function's, and the two must stay in step.
    pub(super) fn parse_global_declaration(
        &mut self,
        start: usize,
        declare: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        // Consume 'global' keyword
        debug_assert!(self.current_value() == "global");
        let (global_start, global_end) = self.current_pos();
        let name = self.current_ident_name();
        self.advance()?;

        let id = TSModuleName::Identifier(Identifier::simple(
            name,
            Span::new(global_start as u32, global_end as u32),
        ));

        // Parse body, or the shorthand (bodyless) form — the same split the
        // string-literal arm takes, terminated by ASI rather than a literal `;`.
        let (body, end) = if matches!(self.current_kind(), TokenKind::BraceOpen) {
            let block = self.parse_module_block(declare)?;
            let end = module_body_end(&block);
            (Some(block), end)
        } else {
            (None, self.semicolon_end()?)
        };

        Ok(Statement::TSModuleDeclaration(TSModuleDeclaration {
            id,
            body,
            declare,
            kind: TSModuleDeclarationKind::Module, // TypeScript uses module kind for global
            global: true,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse nested module declaration: `namespace Outer.Inner { }`
    fn parse_nested_module_declaration(
        &mut self,
        start: u32,
        outer_id: Identifier<'arena>,
        declare: bool,
        kind: TSModuleDeclarationKind,
    ) -> Result<Statement<'arena>, ParseError> {
        self.advance()?; // consume '.'

        // Parse the inner declaration recursively
        let nested_start = self.current_pos().0;
        let nested = self.parse_module_declaration_inner(nested_start as u32, kind)?;
        let body = TSModuleDeclarationBody::TSModuleDeclaration(self.alloc(nested));
        let end = module_body_end(&body);

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
        kind: TSModuleDeclarationKind,
    ) -> Result<TSModuleDeclaration<'arena>, ParseError> {
        // Parse namespace name (identifier or contextual type keyword for nested
        // parts, e.g. the `number` in `namespace a.number {}`).
        let Some(id) = self.take_binding_identifier()? else {
            return Err(self.error_expected("identifier for namespace name"));
        };

        // Check for nested namespace: `namespace Outer.Inner { }`
        let body = if matches!(self.current_kind(), TokenKind::Dot) {
            self.advance()?; // consume '.'

            // Parse nested declaration (recursively)
            // Nested parts inherit the same kind (namespace vs module)
            let nested_start = self.current_pos().0;
            let nested = self.parse_module_declaration_inner(nested_start as u32, kind)?;
            TSModuleDeclarationBody::TSModuleDeclaration(self.alloc(nested))
        } else {
            // Parse block body: `{ statements }`
            // A nested part is never itself `declare`; it inherits any enclosing
            // ambient context via `parse_module_block`.
            self.parse_module_block(false)?
        };

        // Calculate end position based on body
        let end = module_body_end(&body);

        Ok(TSModuleDeclaration {
            id: TSModuleName::Identifier(id),
            body: Some(body),
            declare: false,
            kind,
            global: false,
            span: Span::new(start, end),
        })
    }

    /// Parse a module block: `{ statements }`
    ///
    /// When `is_ambient` (a `declare namespace`/`module`), the body parses with
    /// `in_ambient_context` set; a non-ambient nested block instead inherits any
    /// enclosing ambient context (never clears it). This relaxes ambient-only grammar
    /// (e.g. rest-parameter trailing commas) but does NOT force functions bodiless — a
    /// plain `function f() {}` here is an ordinary `FunctionDeclaration`, while a
    /// `;`-terminated `function f();` is a bodiless `TSDeclareFunction` overload.
    fn parse_module_block(
        &mut self,
        is_ambient: bool,
    ) -> Result<TSModuleDeclarationBody<'arena>, ParseError> {
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

        // Parse module items until '}'. A namespace/module body is a module-item
        // context, so `import`/`export` declarations are valid here (unlike an
        // ordinary block).
        let mut body = self.bvec();
        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let stmt = self.parse_module_item()?;
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
            body: body.into_bump_slice(),
            span: Span::new(block_start as u32, block_end as u32),
        }))
    }
}
