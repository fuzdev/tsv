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

            if !matches!(self.current_kind(), TokenKind::Identifier) {
                return Err(self.error_expected("interface name in extends clause"));
            }

            let expression = self.parse_entity_name()?;

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
        match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Function) => self.parse_declare_function(start),
            TokenKind::Keyword(KeywordKind::Class) => self.parse_declare_class(start, false),
            TokenKind::Identifier if self.current_value() == "abstract" => {
                // declare abstract class
                self.advance()?;
                if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Class)) {
                    return Err(self.error_expected_after("'class'", "declare abstract"));
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
                // declare namespace/module
                self.parse_module_declaration_with_start(true, false, start)
            }
            TokenKind::Identifier if self.current_value() == "interface" => {
                // declare interface
                self.parse_interface_declaration_with_start(start)
            }
            TokenKind::Identifier if self.current_value() == "type" => {
                // declare type
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

    /// Parse declare function: `declare function foo(): void`
    ///
    /// Called from `parse_declare_statement` where `declare` keyword is already consumed.
    fn parse_declare_function(&mut self, start: usize) -> Result<Statement<'arena>, ParseError> {
        self.parse_declare_function_inner(start, true)
    }

    /// Parse function declaration in ambient context (inside `declare namespace`)
    ///
    /// These functions don't have bodies: `function foo(x: number): void;`
    pub(super) fn parse_ambient_function_declaration(
        &mut self,
    ) -> Result<Statement<'arena>, ParseError> {
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
    ) -> Result<Statement<'arena>, ParseError> {
        // Consume 'function' keyword
        self.advance()?;

        // Parse function name
        if !matches!(self.current_kind(), TokenKind::Identifier) {
            return Err(self.error_expected("function name"));
        }

        let (id_start, id_end) = self.current_pos();
        let name = self.current_ident_name();
        self.advance()?;

        let id = Identifier::simple(name, Span::new(id_start as u32, id_end as u32));

        // Parse optional type parameters: <T, U>
        let type_parameters = self.parse_optional_type_parameters()?;

        // Parse parameters
        let params = self.parse_parameter_list()?.into_bump_slice();

        // Parse return type (may be a type predicate)
        let return_type = self.parse_optional_return_type()?;

        let end = self.semicolon_end()?;

        Ok(Statement::TSDeclareFunction(TSDeclareFunction {
            id,
            type_parameters,
            params,
            return_type,
            declare: print_declare,
            r#async: false,   // declare async function is a separate feature
            generator: false, // generators not allowed in declare context
            span: Span::new(start as u32, end),
        }))
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

        // Check for 'asserts' keyword
        let asserts = self.eat_contextual_keyword("asserts");

        // Check if current token is an identifier or `this` (for type predicates and asserts)
        let param_name = self.try_ident_or_keyword_name().or_else(|| {
            // `this` keyword is also valid in type predicates: `this is T`, `asserts this`
            if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::This)) {
                Some(self.current_raw_ident_name())
            } else {
                None
            }
        });
        if let Some(param_name) = param_name {
            // Type predicate: `identifier is Type` or `asserts identifier is Type`.
            // The `is` must not be preceded by a line terminator (TS
            // `parameterName [no LineTerminator here] is Type`): a newline before it
            // makes it not a predicate, leaving `is` a stray token (acorn-typescript's
            // `hasPrecedingLineBreak` guard). Same rule as the arrow `=>` / conditional
            // `extends`.
            if self.peek_is_contextual_keyword("is") && !self.peek_preceded_by_line_terminator() {
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
            // `asserts` followed by non-identifier
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
    /// This handles all enum variants:
    /// - Regular: `enum Foo { A, B }`
    /// - Const: `const enum Foo { A, B }`
    /// - Declare: `declare enum Foo { A, B }`
    /// - Declare const: `declare const enum Foo { A, B }`
    pub(super) fn parse_enum_declaration(
        &mut self,
        is_const: bool,
        is_declare: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.parse_enum_declaration_with_start(is_const, is_declare, start)
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

        // Parse member id: can be identifier or string literal
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
    ) -> Result<Statement<'arena>, ParseError> {
        let start = self.current_pos().0;
        self.parse_module_declaration_with_start(declare, global, start)
    }

    fn parse_module_declaration_with_start(
        &mut self,
        declare: bool,
        global: bool,
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

        // Parse body or semicolon for shorthand
        let (body, end) = if matches!(self.current_kind(), TokenKind::Semicolon) {
            // Shorthand ambient module: `declare module 'name';`
            let end = self.current_pos().1 as u32;
            self.advance()?; // consume ';'
            (None, end)
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
            global,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse declare global: `declare global { ... }`
    /// Parse a global augmentation: `global { … }`.
    ///
    /// `declare` is `true` for `declare global { }` and `false` for a bare
    /// `global { }` (top-level, or implicitly-ambient inside a `declare module`,
    /// where acorn omits the `declare` field). `start` is the keyword position
    /// the span begins at (`declare` for the declared form, `global` for the bare
    /// form). The body is parsed in ambient context when `declare` is set; a bare
    /// `global` nested in an already-ambient module keeps that context via
    /// `parse_module_block`'s save/restore.
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

        // Parse body
        let block = self.parse_module_block(declare)?;
        let end = module_body_end(&block);

        Ok(Statement::TSModuleDeclaration(TSModuleDeclaration {
            id,
            body: Some(block),
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
        let nested = self.parse_module_declaration_inner(nested_start as u32, false, kind)?;
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
        declare: bool,
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
            let nested = self.parse_module_declaration_inner(nested_start as u32, false, kind)?;
            TSModuleDeclarationBody::TSModuleDeclaration(self.alloc(nested))
        } else {
            // Parse block body: `{ statements }`
            // For `declare namespace`, we're in ambient context
            self.parse_module_block(declare)?
        };

        // Calculate end position based on body
        let end = module_body_end(&body);

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
