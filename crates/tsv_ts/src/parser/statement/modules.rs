// Import and export declaration parsing

use crate::ast::internal::*;
use crate::lexer::{KeywordKind, TokenKind};
use tsv_lang::{ParseError, Span};

use super::super::Parser;
use super::class::DecoratedClassExport;

/// The parsed pieces of a module specifier that begins with the contextual
/// `type` keyword, after acorn-typescript's type-only disambiguation has run.
/// `left` is the imported/local name, `right` the optional rename target
/// (local/exported), and `has_type_specifier` whether the leading `type` was
/// the type-only modifier rather than the name itself. `end` is the end of the
/// last token the specifier consumed (its span end).
struct TypeSpecifierParts<'arena> {
    left: Identifier<'arena>,
    right: Option<ModuleExportName<'arena>>,
    has_type_specifier: bool,
    end: usize,
}

impl<'a, 'arena> Parser<'a, 'arena> {
    /// An **exported** `global` augmentation must carry its body: `export global { }`
    /// and `export declare global { }` are declarations, `export global;` and
    /// `export declare global;` are not.
    ///
    /// On the `export` route tsv follows **tsc**, whose `isDeclaration` demands `{`, an
    /// identifier or `export` after `global` — with no body the augmentation is not a
    /// declaration, so `export` is left with nothing to attach to and prettier throws
    /// where it formats the bodied form. acorn rejects *all four* spellings, because
    /// `tokenIsTSDeclarationStart` (which gates its `shouldParseExportStatement`)
    /// enumerates every sibling ambient head — `abstract`, `declare`, `enum`, `module`,
    /// `namespace`, `interface`, `type` — and omits exactly this one, while its own
    /// statement path parses `global { }` happily. A verdict reached for every sibling
    /// and not for this one is an oracle slip rather than a judgement, so tsv keeps the
    /// two bodied spellings (`docs/conformance_svelte.md` §TypeScript Corrections).
    ///
    /// The restriction belongs to the `global` **name**, not to bodylessness: the
    /// shorthand's string arm stays exportable (`export declare module 'c';`, accepted
    /// by all three oracles). Asked at both export arms so the `declare` and bare
    /// spellings cannot drift apart — which they had, tsv accepting `export declare
    /// global { }` while rejecting `export global { }`, a split neither oracle makes.
    fn require_exported_global_body(
        &self,
        declaration: &Statement<'arena>,
    ) -> Result<(), ParseError> {
        if let Statement::TSModuleDeclaration(module) = declaration
            && module.global
            && module.body.is_none()
        {
            return Err(self.error_msg_at(
                "an exported 'global' augmentation must have a body",
                module.span.start as usize,
            ));
        }
        Ok(())
    }

    /// Wrap a declaration statement in an `ExportNamedDeclaration` with no
    /// specifiers or source (`export <declaration>`).
    fn export_named(
        &self,
        start: usize,
        declaration: Statement<'arena>,
        export_kind: ExportKind,
    ) -> Statement<'arena> {
        let end = declaration.span().end;
        Statement::ExportNamedDeclaration(ExportNamedDeclaration {
            declaration: Some(self.alloc(declaration)),
            specifiers: &[],
            source: None,
            attributes: None,
            export_kind,
            span: Span::new(start as u32, end),
        })
    }

    pub(super) fn parse_export_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // `export` declarations are reachable only via `ModuleItem` — a Script
        // goal has no export declarations.
        if self.goal != crate::Goal::Module {
            return Err(self.error_msg("'export' is only allowed in a module"));
        }

        // Consume 'export' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Export)
        ));
        self.advance()?;

        match self.current_kind() {
            // export = expr; (TypeScript CommonJS-style export)
            TokenKind::Equals => {
                self.advance()?; // consume '='
                let expression = self.parse_expression()?;
                let end = self.semicolon_end()?;
                Ok(Statement::TSExportAssignment(TSExportAssignment {
                    expression,
                    span: Span::new(start as u32, end),
                }))
            }
            // export import X = ... (TypeScript import-equals re-export). The only
            // valid `export import` form is import-equals, so the binding (after an
            // optional `type` modifier) must be followed by `=`. `export import X
            // from` / `export import { … }` are rejected. `export import type X =
            // require('m')` is valid; the entity-name form (`export import type X =
            // A.B`) is rejected in `parse_import_equals_declaration`, exactly as for
            // a plain `import type X = …`.
            TokenKind::Keyword(KeywordKind::Import) => {
                self.advance()?; // consume 'import'

                // Optional `type` modifier: `type` is the modifier only when a
                // binding name follows (the alias); a bare `export import type =
                // require('m')` is a *value* re-export of a binding named `type` —
                // the same name-vs-modifier disambiguation the plain import path uses.
                let import_kind = if matches!(self.current_kind(), TokenKind::Identifier)
                    && self.current_value() == "type"
                    && self.peek_kind().is_binding_name_word()
                {
                    self.advance()?; // consume `type`
                    ImportKind::Type
                } else {
                    ImportKind::Value
                };

                // The binding is a `BindingIdentifier`, so a contextual type keyword
                // is a valid name (`export import string = N.M`), matching the plain
                // import-equals path.
                let Some(name) = self.try_binding_name() else {
                    return Err(self.error_expected_after("an identifier", "export import"));
                };
                let (id_start, id_end) = self.current_pos();
                self.advance()?;
                if !matches!(self.current_kind(), TokenKind::Equals) {
                    return Err(self.error_expected("'=' in import-equals declaration"));
                }
                self.parse_import_equals_declaration(
                    start,
                    id_start,
                    id_end,
                    name,
                    import_kind,
                    true, // is_export
                )
            }
            // export as namespace Foo; (TypeScript UMD global export declaration)
            TokenKind::Keyword(KeywordKind::As) => {
                self.advance()?; // consume 'as'
                if !matches!(self.current_kind(), TokenKind::Identifier)
                    || self.current_value() != "namespace"
                {
                    return Err(self.error_expected_after("'namespace'", "export as"));
                }
                self.advance()?; // consume 'namespace'
                if !matches!(self.current_kind(), TokenKind::Identifier) {
                    return Err(self.error_expected_after("an identifier", "export as namespace"));
                }
                let (id_start, id_end) = self.current_pos();
                let name = self.current_ident_name();
                self.advance()?;
                let id = Identifier::simple(name, Span::new(id_start as u32, id_end as u32));
                let end = self.semicolon_end()?;
                Ok(Statement::TSNamespaceExportDeclaration(
                    TSNamespaceExportDeclaration {
                        id,
                        span: Span::new(start as u32, end),
                    },
                ))
            }
            // export default ...
            TokenKind::Keyword(KeywordKind::Default) => {
                self.parse_export_default_declaration(start as u32)
            }
            // export * from "y" or export * as ns from "y"
            TokenKind::Star => self.parse_export_all_declaration(start as u32, ExportKind::Value),
            // export { x, y as z } or export { x } from "y"
            TokenKind::BraceOpen => self.parse_export_specifiers(start as u32, ExportKind::Value),
            // export const/let/var
            TokenKind::Keyword(KeywordKind::Let | KeywordKind::Var) => {
                let decl = self.parse_variable_declaration()?;
                Ok(self.export_named(start, decl, ExportKind::Value))
            }
            // export const ... or export const enum ...
            TokenKind::Keyword(KeywordKind::Const) => {
                // Check for `export const enum` declaration
                let declaration = if self.peek_kind() == TokenKind::Keyword(KeywordKind::Enum) {
                    self.parse_enum_declaration(true)?
                } else {
                    self.parse_variable_declaration()?
                };
                Ok(self.export_named(start, declaration, ExportKind::Value))
            }
            // export enum ...
            TokenKind::Keyword(KeywordKind::Enum) => {
                let decl = self.parse_enum_declaration(false)?;
                Ok(self.export_named(start, decl, ExportKind::Value))
            }
            TokenKind::Keyword(KeywordKind::Function) => {
                let decl = self.parse_function_declaration()?;
                Ok(self.export_named(start, decl, ExportKind::Value))
            }
            // export async function foo() {}
            //
            // `async` is only a declaration keyword here when `function` follows on the
            // SAME LINE — there is no `export async` arrow form. Without this check a
            // stray `export async foo()` would reach `parse_function_or_overload` on a
            // non-`function` token, violating its precondition. `peek_kind` skips
            // comments, matching the statement-position dispatch in `statement/mod.rs`.
            //
            // The line-terminator half is `AsyncFunctionDeclaration : async [no
            // LineTerminator here] function` (ecma262) again, and unlike its
            // statement-position and `export default` siblings the break leaves nothing
            // valid behind: an export needs a Declaration, and a bare `async` is not one.
            // So `export async⏎function b() {}` is a syntax error rather than two
            // statements — acorn and Svelte both reject it; welding the two halves into
            // one async function nobody wrote would be an over-acceptance.
            TokenKind::Keyword(KeywordKind::Async) => {
                if self.peek_kind() != TokenKind::Keyword(KeywordKind::Function)
                    || self.peek_preceded_by_line_terminator()
                {
                    return Err(self.error_expected_after("'function'", "export async"));
                }
                let decl = self.parse_async_function_declaration()?;
                Ok(self.export_named(start, decl, ExportKind::Value))
            }
            TokenKind::Keyword(KeywordKind::Class) => {
                let decl = self.parse_class_declaration()?;
                Ok(self.export_named(start, decl, ExportKind::Value))
            }
            // export @dec class C {} — decorators positioned *after* `export`. Only a
            // class (optionally `abstract`) can follow. The decorator-first orderings
            // (`@dec export class`, `@dec export default class`) go through
            // `parse_decorated_class`; here `export` is already consumed, so this arm
            // mirrors its decorator+abstract+class handling. acorn emits
            // `ExportNamedDeclaration → ClassDeclaration` with the decorators on the
            // class and the class span covering them (`export default @dec class` is a
            // separate shape — a decorated class *expression* — handled by the fall-
            // through expression arm of `parse_export_default_declaration`).
            TokenKind::At => {
                let deco_start = self.current_pos().0;
                let decorators = self.parse_decorators()?;
                // Optional `declare`/`abstract` + `class`, decorators attached and the
                // span extended over them — shared with `parse_decorated_class`. The
                // `export` here *precedes* the decorators, so they stay the declaration
                // head even for an ambient class, and acorn keeps `exportKind: value`.
                let class = self.finish_decorated_class(
                    deco_start,
                    decorators,
                    DecoratedClassExport::BeforeOrAbsent,
                )?;
                Ok(self.export_named(start, Statement::ClassDeclaration(class), ExportKind::Value))
            }
            // export type X = T or export interface X { } or export declare function/class
            TokenKind::Identifier => {
                // `&'a str` (source-bound) — no `.to_string()` needed to hold it
                // across the `self.advance()` calls in the arms below.
                let value = self.current_value();
                match value {
                    "type" => {
                        // Could be:
                        // - export type { Name } from "..." - type-only re-export
                        // - export type * from "..." - type-only re-export all
                        // - export type * as ns from "..." - type-only namespace re-export
                        // - export type X = T - type alias declaration
                        let type_start = self.current_pos().0;
                        // Asked BEFORE `type` is consumed — the predicate reads the token
                        // after `current`, and only the alias arm below consults it.
                        let name_on_same_line = self.peek_is_same_line_declaration_name();
                        self.advance()?; // consume 'type'

                        if matches!(self.current_kind(), TokenKind::BraceOpen) {
                            // export type { Name } from "..." - type-only re-export
                            self.parse_export_specifiers(start as u32, ExportKind::Type)
                        } else if matches!(self.current_kind(), TokenKind::Star) {
                            // export type * from "..." or export type * as ns from "..."
                            self.parse_export_all_declaration(start as u32, ExportKind::Type)
                        } else {
                            // export type X = T - type alias declaration. The name must
                            // be on the SAME line; see `require_same_line_declaration_name`.
                            // Only this arm asks — the `{` and `*` re-export forms above
                            // take the break in every oracle, which is exactly the pair
                            // tsc's own `canFollowExportModifier` exempts.
                            if !name_on_same_line {
                                return Err(self.same_line_declaration_name_error("type alias"));
                            }
                            let decl = self.parse_type_alias_declaration_inner(type_start)?;
                            Ok(self.export_named(start, decl, ExportKind::Type))
                        }
                    }
                    // export interface X { }
                    "interface" => {
                        self.require_same_line_declaration_name("interface")?;
                        let decl = self.parse_interface_declaration()?;
                        Ok(self.export_named(start, decl, ExportKind::Type))
                    }
                    // export declare function/class — ambient declarations are type-level
                    "declare" => {
                        let decl = self.parse_declare_statement()?;
                        self.require_exported_global_body(&decl)?;
                        Ok(self.export_named(start, decl, ExportKind::Type))
                    }
                    // export abstract class Foo {}
                    "abstract" => {
                        let decl = self.parse_abstract_class()?;
                        Ok(self.export_named(start, decl, ExportKind::Value))
                    }
                    // export namespace/module
                    "namespace" | "module" => {
                        self.require_same_line_declaration_name(value)?;
                        let decl = self.parse_module_declaration()?;
                        Ok(self.export_named(start, decl, ExportKind::Value))
                    }
                    // export global { } — the augmentation's bare spelling, which takes
                    // the same `export` route its `declare` twin does. See
                    // `require_exported_global_body` for why both are here.
                    "global" => {
                        // The inner declaration's span starts at `global`, not at
                        // `export` — the shape every sibling arm produces, since
                        // `export_named` is what carries the `export` keyword.
                        let (global_start, _) = self.current_pos();
                        let decl = self.parse_global_declaration(global_start, false)?;
                        self.require_exported_global_body(&decl)?;
                        Ok(self.export_named(start, decl, ExportKind::Value))
                    }
                    _ => {
                        Err(self
                            .error_expected_after("declaration, '{', '*', or 'default'", "export"))
                    }
                }
            }
            _ => Err(self.error_expected_after("declaration, '{', '*', or 'default'", "export")),
        }
    }

    /// Parse export default declaration:
    /// - `export default x`
    /// - `export default function() {}`
    /// - `export default function foo() {}`
    /// - `export default class {}`
    /// - `export default class Foo {}`
    fn parse_export_default_declaration(
        &mut self,
        start: u32,
    ) -> Result<Statement<'arena>, ParseError> {
        // Consume 'default' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Default)
        ));
        self.advance()?;

        // `export default interface Foo {}` — detected before the match so the
        // same-line peek (a `&mut self` borrow) doesn't conflict with the match
        // scrutinee's `&self` borrow of `current_kind()`. Mirrors the statement-level
        // interface dispatch: acorn's `parseExportDefaultDeclaration` routes the
        // `interface` keyword to `tsParseInterfaceDeclaration`, which bails on a line
        // break before the name (then `interface` is an expression). The `&&`
        // short-circuits, so the peek runs only when the keyword is actually present.
        let is_default_interface =
            self.current_value() == "interface" && self.peek_is_same_line_name_word();

        // `export default async⏎function () {}` — the `async` function keyword requires
        // `function` on the *same line* (ECMAScript `async [no LineTerminator here] function`);
        // a line break demotes `async` to the default-exported expression (then the following
        // `function () {}` is a nameless declaration → rejected), matching acorn. Computed before
        // the match for the same borrow reason as `is_default_interface`.
        let is_default_async_function =
            matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Async))
                && self.peek_is(&TokenKind::Keyword(KeywordKind::Function))
                && !self.peek_preceded_by_line_terminator();

        // `export default abstract⏎class Base {}` — the same rule one keyword over:
        // `abstract` binds to `class` only on its own line, so a break demotes it to the
        // default-exported *expression* and the class becomes its own statement (tsc and
        // prettier both read `export default abstract;` then `class Base {}`). acorn
        // instead welds across the break into one exported abstract class; tsv followed it
        // there, which silently deleted the line terminator. Falling through to the
        // expression arm also makes the bare `export default abstract;` parse, which an
        // unconditional `class` demand would reject. Computed before the match for the
        // same borrow reason as `is_default_interface`.
        let is_default_abstract_class = self.current_value() == "abstract"
            && self.peek_is(&TokenKind::Keyword(KeywordKind::Class))
            && !self.peek_preceded_by_line_terminator();

        let (declaration, end) = match self.current_kind() {
            TokenKind::Keyword(KeywordKind::Async) if is_default_async_function => {
                // export default async function() {}
                let async_start = self.current_pos().0 as u32;
                self.advance()?; // consume 'async'

                if !matches!(
                    self.current_kind(),
                    TokenKind::Keyword(KeywordKind::Function)
                ) {
                    return Err(self.error_expected_after("'function'", "async"));
                }

                let result = self.parse_function_declaration_or_declare(true)?;
                match result {
                    ExportFunctionDeclaration::Declaration(mut func) => {
                        // Update span to include 'async' keyword
                        func.span = Span::new(async_start, func.span.end);
                        let end = func.span.end;
                        (ExportDefaultValue::FunctionDeclaration(func), end)
                    }
                    ExportFunctionDeclaration::Declare(mut func) => {
                        func.span = Span::new(async_start, func.span.end);
                        let end = func.span.end;
                        (ExportDefaultValue::TSDeclareFunction(func), end)
                    }
                }
            }
            TokenKind::Keyword(KeywordKind::Function) => {
                // Name is optional for export default function() {}
                let result = self.parse_function_declaration_or_declare(false)?;
                match result {
                    ExportFunctionDeclaration::Declaration(func) => {
                        let end = func.span.end;
                        (ExportDefaultValue::FunctionDeclaration(func), end)
                    }
                    ExportFunctionDeclaration::Declare(func) => {
                        let end = func.span.end;
                        (ExportDefaultValue::TSDeclareFunction(func), end)
                    }
                }
            }
            TokenKind::Keyword(KeywordKind::Class) => {
                // Name is optional for export default class {}
                let class = self.parse_class_declaration_inner(false, false)?;
                let end = class.span.end;
                (ExportDefaultValue::ClassDeclaration(class), end)
            }
            TokenKind::Identifier if is_default_abstract_class => {
                // export default abstract class {}
                let abstract_start = self.current_pos().0 as u32;
                self.advance()?; // consume 'abstract'

                debug_assert!(matches!(
                    self.current_kind(),
                    TokenKind::Keyword(KeywordKind::Class)
                ));

                let mut class = self.parse_class_declaration_inner(false, true)?;
                // Update span to include 'abstract' keyword
                class.span = Span::new(abstract_start, class.span.end);
                let end = class.span.end;
                (ExportDefaultValue::ClassDeclaration(class), end)
            }
            TokenKind::Identifier if is_default_interface => {
                // export default interface Foo {}
                let iface_start = self.current_pos().0;
                let iface = self.parse_interface_declaration_struct(iface_start, false)?;
                let end = iface.span.end;
                (ExportDefaultValue::TSInterfaceDeclaration(iface), end)
            }
            _ => {
                // Expression
                let expr = self.parse_expression()?;
                let end = self.semicolon_end()?;
                return Ok(Statement::ExportDefaultDeclaration(
                    ExportDefaultDeclaration {
                        declaration: ExportDefaultValue::Expression(expr),
                        span: Span::new(start, end),
                    },
                ));
            }
        };

        Ok(Statement::ExportDefaultDeclaration(
            ExportDefaultDeclaration {
                declaration,
                span: Span::new(start, end),
            },
        ))
    }

    /// Parse a `ModuleExportName` at the current token — the ONE implementation of
    /// that production. A `StringLiteral` (arbitrary module namespace name) or an
    /// `IdentifierName`: **any keyword**, because the name refers to a binding in
    /// another module rather than to one here (`export { with } from 'm'`,
    /// `export { class as C } from 'm'`, the `default` in `export * as default`).
    /// Advances past the name.
    ///
    /// `what` names the position in the error when no name is there, and is the
    /// caller's precisely because the positions differ: most sites have just consumed
    /// an `as` and say so, while the export specifier's LOCAL name has nothing in
    /// front of it.
    fn parse_module_export_name(
        &mut self,
        what: &str,
    ) -> Result<ModuleExportName<'arena>, ParseError> {
        if matches!(self.current_kind(), TokenKind::String) {
            Ok(ModuleExportName::Literal(self.parse_string_literal()?))
        } else {
            let (start, end) = self.current_pos();
            let Some(name) = self.try_identifier_name() else {
                return Err(self.error_expected_found(what));
            };
            self.advance()?;
            Ok(ModuleExportName::Identifier(Identifier::simple(
                name,
                Span::new(start as u32, end as u32),
            )))
        }
    }

    /// Whether the current token is the `as` keyword.
    #[inline]
    fn at_as_keyword(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::As))
    }

    /// [`Parser::at_as_keyword`] one token ahead — the specifier lookahead that
    /// decides whether a name slot is a `ModuleExportName` or a binding.
    #[inline]
    fn peek_at_as_keyword(&mut self) -> bool {
        self.peek_kind() == TokenKind::Keyword(KeywordKind::As)
    }

    /// Whether the current token is an identifier or any keyword — acorn's
    /// `tokenIsKeywordOrIdentifier` (an identifier-*shaped* word, reserved or
    /// not), the test that drives the type-only specifier state machine below.
    #[inline]
    fn at_keyword_or_ident(&self) -> bool {
        matches!(
            self.current_kind(),
            TokenKind::Identifier | TokenKind::Keyword(_)
        )
    }

    /// Consume the current identifier-or-keyword token as an `Identifier`. Used
    /// for the `as` word(s) in the type-only specifier state machine (a keyword
    /// token, never escaped). Assumes [`Parser::at_keyword_or_ident`].
    fn take_keyword_or_ident(&mut self) -> Result<Identifier<'arena>, ParseError> {
        let (start, end) = self.current_pos();
        let Some(name) = self.try_identifier_name() else {
            return Err(self.error_expected("identifier in specifier"));
        };
        self.advance()?;
        Ok(Identifier::simple(
            name,
            Span::new(start as u32, end as u32),
        ))
    }

    /// Parse the rename target after `as` (`rightOfAs`): a `BindingIdentifier`
    /// for an import (`import { x as local }`), or a `ModuleExportName` — an
    /// identifier or a string — for an export (`export { local as 'name' }`).
    fn parse_specifier_rename_target(
        &mut self,
        is_import: bool,
    ) -> Result<(ModuleExportName<'arena>, usize), ParseError> {
        if is_import {
            let (start, end) = self.current_pos();
            let Some(name) = self.try_binding_name() else {
                return Err(self.error_expected_after("identifier", "as"));
            };
            self.advance()?;
            Ok((
                ModuleExportName::Identifier(Identifier::simple(
                    name,
                    Span::new(start as u32, end as u32),
                )),
                end,
            ))
        } else {
            let (_, end) = self.current_pos();
            Ok((self.parse_module_export_name("identifier after 'as'")?, end))
        }
    }

    /// Parse the name in the `{ type <name> … }` case (`leftOfAs` when `type` is
    /// the type-only modifier). Import mirrors acorn's `parseIdent(true)` +
    /// `checkUnreserved` unless a rename follows (so `import { type class }`
    /// rejects but `import { type class as C }` accepts); export uses the same
    /// identifier-or-contextual-keyword name its plain specifiers accept.
    fn parse_type_modifier_name(
        &mut self,
        is_import: bool,
    ) -> Result<(Identifier<'arena>, usize), ParseError> {
        let (start, end) = self.current_pos();
        let name = if is_import && self.peek_at_as_keyword() {
            self.try_identifier_name()
        } else {
            self.try_ident_or_contextual_name()
        };
        let Some(name) = name else {
            return Err(self.error_expected("identifier in specifier"));
        };
        self.advance()?;
        Ok((
            Identifier::simple(name, Span::new(start as u32, end as u32)),
            end,
        ))
    }

    /// Disambiguate a module specifier that begins with the contextual `type`
    /// keyword: `type` may be the type-only modifier (`{ type A }`) or the name
    /// itself (`{ type as age }` — a value import/export of a binding named
    /// `type`, renamed). A faithful port of acorn-typescript's
    /// `parseTypeOnlyImportExportSpecifier`, which needs a two-token lookahead
    /// past the `as`(es):
    ///
    /// - `type as <name>` → `type` is the name (value), `as <name>` the rename
    /// - `type as as` → `type` is the name (value), renamed to `as`
    /// - `type as as <name>` → `as` is the name (type-only), renamed
    /// - `type as` → `as` is the name (type-only)
    /// - `type <name>` → `type` is the modifier (type-only)
    /// - `type` → `type` is the name (value)
    ///
    /// `is_import` selects the rename-target grammar (see
    /// [`Parser::parse_specifier_rename_target`]). The caller has already checked
    /// the current token is the contextual `type` keyword.
    fn parse_type_specifier_parts(
        &mut self,
        is_import: bool,
    ) -> Result<TypeSpecifierParts<'arena>, ParseError> {
        // The leading `type` is consumed as the tentative name (acorn's
        // `node.imported/local = parseModuleExportName()`); the state machine may
        // reassign `left` to a later token if `type` turns out to be the modifier.
        let (type_start, type_end) = self.current_pos();
        let mut left = Identifier::simple(
            self.current_ident_name(),
            Span::new(type_start as u32, type_end as u32),
        );
        self.advance()?; // consume `type`

        let mut right: Option<ModuleExportName<'arena>> = None;
        let mut has_type_specifier = false;
        let mut can_parse_as = true;
        let mut end = type_end;

        if self.at_as_keyword() {
            // `{ type as …? }`
            let first_as = self.take_keyword_or_ident()?;
            end = first_as.span.end as usize;
            if self.at_as_keyword() {
                // `{ type as as …? }`
                let second_as = self.take_keyword_or_ident()?;
                end = second_as.span.end as usize;
                if self.at_keyword_or_ident() {
                    // `{ type as as something }` — type-only, `as` is the name, renamed.
                    has_type_specifier = true;
                    left = first_as;
                    let (r, r_end) = self.parse_specifier_rename_target(is_import)?;
                    right = Some(r);
                    end = r_end;
                    can_parse_as = false;
                } else {
                    // `{ type as as }` — value, `type` is the name, renamed to `as`.
                    right = Some(ModuleExportName::Identifier(second_as));
                    can_parse_as = false;
                }
            } else if self.at_keyword_or_ident() {
                // `{ type as something }` — value, `type` is the name, renamed.
                can_parse_as = false;
                let (r, r_end) = self.parse_specifier_rename_target(is_import)?;
                right = Some(r);
                end = r_end;
            } else {
                // `{ type as }` — type-only, `as` is the name.
                has_type_specifier = true;
                left = first_as;
            }
        } else if self.at_keyword_or_ident() {
            // `{ type something …? }` — `type` is the modifier; `something` the name.
            has_type_specifier = true;
            let (name, name_end) = self.parse_type_modifier_name(is_import)?;
            left = name;
            end = name_end;
        }

        if can_parse_as && self.at_as_keyword() {
            self.advance()?; // consume `as`
            let (r, r_end) = self.parse_specifier_rename_target(is_import)?;
            right = Some(r);
            end = r_end;
        }

        Ok(TypeSpecifierParts {
            left,
            right,
            has_type_specifier,
            end,
        })
    }

    /// Parse export all declaration:
    /// - `export * from "y"`
    /// - `export * as ns from "y"`
    /// - `export type * from "y"`
    /// - `export type * as ns from "y"`
    fn parse_export_all_declaration(
        &mut self,
        start: u32,
        export_kind: ExportKind,
    ) -> Result<Statement<'arena>, ParseError> {
        // Consume '*'
        debug_assert!(matches!(self.current_kind(), TokenKind::Star));
        self.advance()?;

        // Check for `as ns` — a `ModuleExportName` (identifier or string).
        let exported = if self.at_as_keyword() {
            self.advance()?; // consume 'as'
            Some(self.parse_module_export_name("identifier after 'as'")?)
        } else {
            None
        };

        // Expect 'from'
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::From)) {
            return Err(self.error_expected("'from' in export all declaration"));
        }
        self.advance()?;

        // Parse source string
        let source = self.parse_string_literal()?;
        // Parse import attributes: `with { type: "json" }`
        let attributes = self.parse_import_attributes()?;
        let end = self.semicolon_end()?;

        Ok(Statement::ExportAllDeclaration(ExportAllDeclaration {
            exported,
            source,
            attributes,
            export_kind,
            span: Span::new(start, end),
        }))
    }

    /// Parse export specifiers `{ x, y as z }` with optional `from "source"`:
    /// - `export { x, y as z }` / `export { x } from "y"` (`export_kind: Value`)
    /// - `export { type x, y }` (inline type modifier, value exports only)
    /// - `export type { Name } from "..."` (`export_kind: Type`; specifiers
    ///   stay value — the type-ness lives on the declaration)
    fn parse_export_specifiers(
        &mut self,
        start: u32,
        export_kind: ExportKind,
    ) -> Result<Statement<'arena>, ParseError> {
        // Consume '{'
        debug_assert!(matches!(self.current_kind(), TokenKind::BraceOpen));
        self.advance()?;

        let mut specifiers = self.bvec();

        // Parse specifiers until '}'
        while !matches!(self.current_kind(), TokenKind::BraceClose) {
            let (spec_start, _) = self.current_pos();

            // A specifier that begins with the contextual `type` keyword needs
            // acorn's type-only disambiguation (`export { type as age }` is a
            // value export of the local `type`, renamed — not the modifier),
            // via the shared helper. Not recognized inside `export type { ... }`
            // (TS rejects doubled type modifiers), so there `type A` falls to the
            // plain path and errors at `A`.
            if matches!(export_kind, ExportKind::Value)
                && matches!(self.current_kind(), TokenKind::Identifier)
                && self.current_value() == "type"
            {
                let parts = self.parse_type_specifier_parts(/* is_import */ false)?;
                let local = ModuleExportName::Identifier(parts.left.clone());
                let exported = parts
                    .right
                    .unwrap_or(ModuleExportName::Identifier(parts.left));
                specifiers.push(ExportSpecifier {
                    local,
                    exported,
                    export_kind: if parts.has_type_specifier {
                        ExportKind::Type
                    } else {
                        ExportKind::Value
                    },
                    span: Span::new(spec_start as u32, parts.end as u32),
                });
            } else {
                let (local, exported, spec_end) = self.parse_export_specifier_names()?;

                specifiers.push(ExportSpecifier {
                    local,
                    exported,
                    export_kind: ExportKind::Value,
                    span: Span::new(spec_start as u32, spec_end),
                });
            }

            // Check for comma
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance()?;
            } else {
                break;
            }
        }

        // Expect '}'
        if !matches!(self.current_kind(), TokenKind::BraceClose) {
            return Err(self.error_expected("'}' to close export specifiers"));
        }
        self.advance()?;

        // Check for 'from "source"', then optional import attributes. Per the
        // spec a `with` clause attaches only to a re-export (`export … from …`),
        // so attributes stay empty for a local `export { x }`.
        let (source, attributes) =
            if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::From)) {
                self.advance()?;
                let source = self.parse_string_literal()?;
                let attributes = self.parse_import_attributes()?;
                (Some(source), attributes)
            } else {
                (None, None)
            };

        let end = self.semicolon_end()?;

        Ok(Statement::ExportNamedDeclaration(ExportNamedDeclaration {
            declaration: None,
            specifiers: specifiers.into_bump_slice(),
            source,
            attributes,
            export_kind,
            span: Span::new(start, end),
        }))
    }

    /// Parse an export specifier: `local`, `local as exported`, or `default`.
    ///
    /// Returns (local, exported, spec_end_pos).
    ///
    /// BOTH names are a `ModuleExportName` — a string or **any `IdentifierName`**,
    /// reserved words included, since a re-export names another module's binding
    /// rather than referencing one here (`export { with } from 'm'`, `export { class
    /// as C } from 'm'`). Without a `from` clause the local *is* an
    /// `IdentifierReference`, and a reserved word there is a Static Semantics early
    /// error (`ReferencedBindings`) — deferred like the rest, and like the string and
    /// `default` locals this production has always accepted.
    fn parse_export_specifier_names(
        &mut self,
    ) -> Result<(ModuleExportName<'arena>, ModuleExportName<'arena>, u32), ParseError> {
        let local = self.parse_module_export_name("identifier in export specifier")?;
        let exported = if self.at_as_keyword() {
            self.advance()?; // consume 'as'
            self.parse_module_export_name("identifier after 'as'")?
        } else {
            local.clone()
        };

        // Each name carries its own end via its span; the specifier ends at the
        // exported name (which is the local name when there's no `as`).
        let spec_end = exported.span().end;
        Ok((local, exported, spec_end))
    }

    /// Parse a plain named-import specifier's names — the `imported`
    /// `ModuleExportName`, the `local` binding, and the specifier's end offset.
    /// The import-side counterpart of [`Parser::parse_export_specifier_names`];
    /// the `type`-modifier path is handled separately by
    /// [`Parser::parse_type_specifier_parts`]. Grammar:
    ///
    ///   ImportSpecifier : ImportedBinding
    ///                   | ModuleExportName as ImportedBinding
    ///
    /// With `as`, the imported name is a `ModuleExportName` — a string (arbitrary
    /// module namespace name) or any `IdentifierName` including reserved words
    /// (`import { class as C }`). Without `as`, it is an `ImportedBinding` (a
    /// `BindingIdentifier`), so reserved words are rejected (`import { class }` is a
    /// syntax error, see `input_invalid_keyword_no_binding`).
    fn parse_import_specifier_names(
        &mut self,
    ) -> Result<(ModuleExportName<'arena>, Identifier<'arena>, u32), ParseError> {
        let (imp_start, imp_end) = self.current_pos();
        let imported = if matches!(self.current_kind(), TokenKind::String) {
            ModuleExportName::Literal(self.parse_string_literal()?)
        } else {
            let imported_name = if self.peek_at_as_keyword() {
                self.try_identifier_name()
            } else {
                self.try_ident_or_contextual_name()
            };
            let Some(imported_name) = imported_name else {
                return Err(self.error_expected("identifier in import specifier"));
            };
            self.advance()?;
            ModuleExportName::Identifier(Identifier::simple(
                imported_name,
                Span::new(imp_start as u32, imp_end as u32),
            ))
        };

        // Check for 'as' rename → local binding (always an identifier)
        let (local, spec_end) = if self.at_as_keyword() {
            self.advance()?;

            let (local_start, local_end) = self.current_pos();
            let Some(local_name) = self.try_binding_name() else {
                return Err(self.error_expected_after("identifier", "as"));
            };
            self.advance()?;

            (
                Identifier::simple(local_name, Span::new(local_start as u32, local_end as u32)),
                local_end,
            )
        } else {
            // No `as`: the local binding is the imported identifier itself.
            // A string imported name has no valid binding without `as` —
            // reject (matches acorn).
            match &imported {
                ModuleExportName::Identifier(id) => (id.clone(), imp_end),
                ModuleExportName::Literal(_) => {
                    return Err(self.error_expected_after("'as'", "string import name"));
                }
            }
        };

        Ok((imported, local, spec_end as u32))
    }

    /// An ES `import` declaration is a `ModuleItem`, so it is a syntax error at
    /// `Goal::Script` — but a TypeScript **import-equals** (`import x = A.B`,
    /// `import x = require('y')`) is not an `ImportDeclaration` at all. It predates ES
    /// modules, is how a script or a namespace aliases, and tsc accepts it in a
    /// non-module file: its own `conformance/externalModules/topLevelAwait.2.ts`
    /// asserts exactly that, commented *"await allowed in import=namespace when not a
    /// module"*, and compiles with no `.errors.txt` baseline.
    ///
    /// So the gate cannot fire on the `import` keyword — it has to wait until the shape
    /// is decided, which is why this is called from the two sites that build an
    /// `ImportDeclaration` rather than at the top of `parse_import_declaration`. The
    /// `await` binding in that seed follows for free: at `Script` goal `await` is an
    /// ordinary identifier, so once import-equals is reachable the binding parses like
    /// any other name.
    ///
    /// acorn rejects import-equals at script goal too, but that is base acorn's
    /// ES-grammar check firing before the TS plugin ever sees the statement — a slip,
    /// not a TypeScript judgement — so tsv follows tsc here and diverges from the
    /// shape oracle. `position` is the `import` keyword, so the error still points at
    /// the statement head rather than at whatever token ends it.
    fn check_import_declaration_goal(&self, position: usize) -> Result<(), ParseError> {
        if self.goal == crate::Goal::Module {
            return Ok(());
        }
        Err(self.error_msg_at("'import' is only allowed in a module", position))
    }

    /// Parse import declaration:
    /// - `import x from "y"` (default)
    /// - `import { a, b } from "y"` (named)
    /// - `import * as ns from "y"` (namespace)
    /// - `import "y"` (side-effect)
    /// - `import x, { a, b } from "y"` (default + named)
    /// - `import type { a } from "y"` (type-only import)
    /// - `import { type a, b } from "y"` (inline type modifier)
    /// - `import x from "y" with { type: "json" }` (import attributes)
    pub(super) fn parse_import_declaration(&mut self) -> Result<Statement<'arena>, ParseError> {
        let (start, _) = self.current_pos();

        // NOTE: the `Goal::Script` gate is NOT here, on the keyword — see
        // `check_import_declaration_goal`, called at the two points that actually build
        // an `ImportDeclaration`. (Dynamic `import(...)` and `import.meta` are
        // expressions, parsed elsewhere — the statement dispatcher routes
        // `import(`/`import.` there before here.)

        // Consume 'import' keyword
        debug_assert!(matches!(
            self.current_kind(),
            TokenKind::Keyword(KeywordKind::Import)
        ));
        self.advance()?;

        // The import-phase proposals: `import source <binding> from …` and
        // `import defer * as ns from …`. `source`/`defer` are contextual — a phase
        // keyword only in the phase-specific shape, otherwise an ordinary default
        // binding (`import defer from …` imports a default named `defer`). acorn
        // supports neither proposal, so accepting them is a deliberate divergence
        // from the Svelte/acorn oracle — see docs/conformance_svelte.md.
        let phase = if matches!(self.current_kind(), TokenKind::Identifier) {
            let value = self.current_value();
            let is_defer = value == "defer";
            let is_source = value == "source";
            if is_defer && matches!(self.peek_kind(), TokenKind::Star) {
                self.advance()?; // consume `defer`
                ImportPhase::Defer
            } else if is_source && matches!(self.peek_kind(), TokenKind::Identifier) {
                self.advance()?; // consume `source`
                ImportPhase::Source
            } else {
                ImportPhase::None
            }
        } else {
            ImportPhase::None
        };

        let mut specifiers = self.bvec();

        // Check for side-effect import: `import "y"`
        if matches!(self.current_kind(), TokenKind::String) {
            self.check_import_declaration_goal(start)?;
            let source = self.parse_string_literal()?;
            // Check for import attributes after source
            let attributes = self.parse_import_attributes()?;
            let end = self.semicolon_end()?;

            return Ok(Statement::ImportDeclaration(ImportDeclaration {
                specifiers: &[],
                source,
                attributes,
                import_kind: ImportKind::Value,
                phase,
                span: Span::new(start as u32, end),
            }));
        }

        // Check for `import type` (type-only import)
        let import_kind = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "type"
        {
            // Look ahead to see if this is `import type { ... }` or `import type X from ...`
            // vs `import type from "y"` (importing a default export named "type").
            // Skip comments so `import type /* c */ {}` isn't misread as a default
            // import named `type` (the comment is collected for the printer).
            //
            // A type-only default binding is a `BindingIdentifier`, so a contextual
            // type keyword is a valid name (`import type any from "y"`) — the
            // binding-name set after `type` therefore includes `can_be_binding_name`
            // keywords, not just plain identifiers. `from` is a binding name too, but
            // stays excluded (the `!From` guard): `import type from "y"` is instead a
            // *value* import of a default binding named `type`.
            let next_kind = self.peek_kind();
            let next_starts_type_import =
                matches!(next_kind, TokenKind::BraceOpen | TokenKind::Star)
                    || next_kind.is_binding_name_word();
            if next_starts_type_import
                && !matches!(next_kind, TokenKind::Keyword(KeywordKind::From))
            {
                self.advance()?; // consume 'type'
                ImportKind::Type
            } else {
                ImportKind::Value
            }
        } else {
            ImportKind::Value
        };

        // Whether a default specifier was parsed with no following comma — used to
        // reject `import x * as ns` / `import x { a }` (a default must be separated
        // from a namespace/named clause by a comma).
        let mut default_needs_comma = false;

        // Parse default import: `import x from "y"` or `import type X from "y"`
        // Also check for `import x = require("y")` or `import x = A.B`. The binding is
        // a `BindingIdentifier`, so a contextual type keyword is a valid name
        // (`import any from "y"`, `import string = N.M`).
        if let Some(name) = self.try_binding_name() {
            let (id_start, id_end) = self.current_pos();
            self.advance()?;

            // Check for `import x = ...` (TSImportEqualsDeclaration)
            if matches!(self.current_kind(), TokenKind::Equals) {
                // A phase keyword has no import-equals form (`import source x =
                // require(…)` is not in the proposal grammar); reject rather than
                // silently drop the phase. Only `Source` can reach here — `Defer`
                // requires `* as`, so its leading token is `*`, not this binding.
                if phase != ImportPhase::None {
                    return Err(self.error_msg(
                        "an import-phase keyword cannot precede an import-equals declaration",
                    ));
                }
                return self.parse_import_equals_declaration(
                    start,
                    id_start,
                    id_end,
                    name,
                    import_kind,
                    false, // is_export
                );
            }

            specifiers.push(ImportSpecifier::Default(ImportDefaultSpecifier {
                local: Identifier::simple(name, Span::new(id_start as u32, id_end as u32)),
                span: Span::new(id_start as u32, id_end as u32),
            }));

            // Check for comma (default + named/namespace). A default import must be
            // followed by `,` (then a namespace/named clause) or `from`: a default
            // butting directly against `* as ns` / `{ … }` with no comma is a syntax
            // error (`import x * as ns`, `import x { a }`), matching acorn. Tracked so
            // the namespace/named blocks below can reject the missing-comma form.
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance()?;
            } else {
                default_needs_comma = true;
            }
        }

        // Every remaining shape is a genuine ES `ImportDeclaration`, and every one
        // passes through here: a default binding falls out of the block above (having
        // already returned if it was an import-equals), while `* as ns` and `{ … }`
        // never enter it, since `try_binding_name` declines their leading token.
        self.check_import_declaration_goal(start)?;

        // Parse namespace import: `import * as ns from "y"`
        if matches!(self.current_kind(), TokenKind::Star) {
            if default_needs_comma {
                return Err(self.error_expected_after("','", "default import"));
            }
            let ns_start = self.current_pos().0;
            self.advance()?;

            // Expect 'as' keyword
            if !self.at_as_keyword() {
                return Err(self.error_expected_after("'as'", "*"));
            }
            self.advance()?;

            // Parse local name — a `BindingIdentifier`, so a contextual type keyword
            // is a valid namespace-import binding (`import * as any from "y"`).
            let Some(local) = self.take_binding_identifier()? else {
                return Err(self.error_expected_after("identifier", "as"));
            };
            let local_end = local.span.end;

            specifiers.push(ImportSpecifier::Namespace(ImportNamespaceSpecifier {
                local,
                span: Span::new(ns_start as u32, local_end),
            }));
        }

        // Parse named imports: `import { a, b as c } from "y"`
        if matches!(self.current_kind(), TokenKind::BraceOpen) {
            if default_needs_comma {
                return Err(self.error_expected_after("','", "default import"));
            }
            self.advance()?;

            while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
                let (spec_start, _) = self.current_pos();

                // A specifier that begins with the contextual `type` keyword needs
                // acorn's type-only disambiguation: `type` may be the modifier
                // (`import { type A }`) or the imported name itself
                // (`import { type as age }` — a value import of a binding named
                // `type`, renamed). The two-token lookahead lives in the shared
                // helper; every other specifier is a plain value import. Not
                // recognized inside `import type { … }` (TS rejects doubled type
                // modifiers), so there `type A` falls to the plain path and errors
                // at `A` — mirroring the `export type { … }` guard below.
                if matches!(import_kind, ImportKind::Value)
                    && matches!(self.current_kind(), TokenKind::Identifier)
                    && self.current_value() == "type"
                {
                    let parts = self.parse_type_specifier_parts(/* is_import */ true)?;
                    // An import's rename target is a `BindingIdentifier`, so `right`
                    // is always an identifier here (never a string) — every
                    // import-side assignment in `parse_type_specifier_parts` goes
                    // through `try_binding_name` or the `as` keyword token.
                    let local = match parts.right {
                        Some(ModuleExportName::Identifier(id)) => id,
                        // Unreachable: import-side `right` is only ever set from
                        // `try_binding_name` or an `as` keyword token.
                        #[expect(clippy::unreachable)] // import rename target is never a string
                        Some(ModuleExportName::Literal(_)) => {
                            unreachable!("import rename target is never a string literal")
                        }
                        None => parts.left.clone(),
                    };
                    specifiers.push(ImportSpecifier::Named(ImportNamedSpecifier {
                        imported: ModuleExportName::Identifier(parts.left),
                        local,
                        import_kind: if parts.has_type_specifier {
                            ImportKind::Type
                        } else {
                            ImportKind::Value
                        },
                        span: Span::new(spec_start as u32, parts.end as u32),
                    }));
                } else {
                    let (imported, local, spec_end) = self.parse_import_specifier_names()?;
                    specifiers.push(ImportSpecifier::Named(ImportNamedSpecifier {
                        imported,
                        local,
                        import_kind: ImportKind::Value,
                        span: Span::new(spec_start as u32, spec_end),
                    }));
                }

                // Comma separator
                if matches!(self.current_kind(), TokenKind::Comma) {
                    self.advance()?;
                } else {
                    break;
                }
            }

            self.expect(&TokenKind::BraceClose)?;
        }

        // A source-phase import is `import source ImportedBinding FromClause` — a
        // single binding, no namespace/named clause and no second specifier. The
        // phase commits on the leading `source <ident>` one-token lookahead, so a
        // multi-specifier or non-default clause that slipped past it is rejected
        // here: `import source x, { a }`, `import source x, * as ns`, and (after a
        // stray `type` modifier) `import source type { a }`. (`import defer` is held
        // to its `* as ns` shape by the phase lookahead, so it needs no analogue.)
        if phase == ImportPhase::Source
            && !(specifiers.len() == 1 && matches!(specifiers[0], ImportSpecifier::Default(_)))
        {
            return Err(self.error_msg("a source-phase import takes a single binding"));
        }

        // Expect 'from' keyword
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::From)) {
            return Err(self.error_expected_after("'from'", "import specifiers"));
        }
        self.advance()?;

        // Parse module source
        if !matches!(self.current_kind(), TokenKind::String) {
            return Err(self.error_expected("string literal as module source"));
        }
        let source = self.parse_string_literal()?;

        // Parse import attributes: `with { type: "json" }`
        let attributes = self.parse_import_attributes()?;

        let end = self.semicolon_end()?;

        Ok(Statement::ImportDeclaration(ImportDeclaration {
            specifiers: specifiers.into_bump_slice(),
            source,
            attributes,
            import_kind,
            phase,
            span: Span::new(start as u32, end),
        }))
    }

    /// Parse import attributes: `with { type: "json" }`.
    ///
    /// `None` when there is no `with` clause; `Some(vec)` when one is present —
    /// `Some([])` for an empty `with {}`, which is preserved (acorn/prettier
    /// keep it).
    fn parse_import_attributes(
        &mut self,
    ) -> Result<Option<&'arena [ImportAttribute<'arena>]>, ParseError> {
        // The `WithClause`'s own `with` token. It is the reserved word (a `ReservedWord`
        // barred from every name position — see `KeywordKind::With`), spelled out by this
        // production rather than read as an identifier, so match the keyword token.
        if !matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::With)) {
            return Ok(None);
        }
        self.advance()?; // consume 'with'

        // Expect opening brace
        if !matches!(self.current_kind(), TokenKind::BraceOpen) {
            return Err(self.error_expected_after("'{'", "with"));
        }
        self.advance()?;

        let mut attributes = self.bvec();
        // Decoded `[[Key]]` StringValues seen so far, for the duplicate-key early
        // error (ecma262 §sec-imports-static-semantics-early-errors).
        let mut seen_keys: Vec<String> = Vec::new();

        while !matches!(self.current_kind(), TokenKind::BraceClose | TokenKind::Eof) {
            let (attr_start, _) = self.current_pos();

            // Parse attribute key — an `IdentifierName` (`type`, or a reserved
            // word like `default`) or a string literal (`'resolution-mode'`).
            // Per ecma262 `AttributeKey : IdentifierName | StringLiteral`.
            let key = if matches!(self.current_kind(), TokenKind::String) {
                ImportAttributeKey::Literal(self.parse_string_literal()?)
            } else if let Some(key_name) = self.try_identifier_name() {
                let (key_start, key_end) = self.current_pos();
                self.advance()?;
                ImportAttributeKey::Identifier(Identifier::simple(
                    key_name,
                    Span::new(key_start as u32, key_end as u32),
                ))
            } else {
                return Err(self.error_expected("identifier or string as import attribute key"));
            };

            // Duplicate-key check: keys with the same StringValue are a Syntax
            // Error (`with {type:'a', type:'b'}` / `with {'type':'a', type:'b'}`).
            let key_string = self.attribute_key_string(&key);
            if seen_keys.iter().any(|k| k == &key_string) {
                return Err(
                    self.error_msg_at("Duplicated key in attributes", key.span().start as usize)
                );
            }
            seen_keys.push(key_string);

            // Expect colon
            if !matches!(self.current_kind(), TokenKind::Colon) {
                return Err(self.error_expected_after("':'", "import attribute key"));
            }
            self.advance()?;

            // Parse attribute value (string literal)
            if !matches!(self.current_kind(), TokenKind::String) {
                return Err(self.error_expected("string literal as import attribute value"));
            }
            let value = self.parse_string_literal()?;
            let attr_end = value.span.end;

            attributes.push(ImportAttribute {
                key,
                value,
                span: Span::new(attr_start as u32, attr_end),
            });

            // Comma separator
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance()?;
            } else {
                break;
            }
        }

        self.expect(&TokenKind::BraceClose)?;

        Ok(Some(attributes.into_bump_slice()))
    }

    /// The decoded `[[Key]]` StringValue of an import-attribute key (ecma262):
    /// an identifier resolves to its name, a string literal to its decoded
    /// content. Used to detect duplicate keys, where `type` and `'type'` collide.
    fn attribute_key_string(&self, key: &ImportAttributeKey<'_>) -> String {
        match key {
            ImportAttributeKey::Identifier(id) => match id.escaped_name {
                Some(s) => s.to_string(),
                None => {
                    let start = id.span.start as usize - self.base_offset;
                    self.source[start..start + id.name_len as usize].to_string()
                }
            },
            ImportAttributeKey::Literal(
                lit @ Literal {
                    value: LiteralValue::String(cooked),
                    ..
                },
            ) => self.resolve_cooked(cooked, lit.span).to_string(),
            // Attribute keys are only identifiers or string literals.
            ImportAttributeKey::Literal(_) => String::new(),
        }
    }

    /// Parse `import x = require("y")` or `import x = A.B`
    fn parse_import_equals_declaration(
        &mut self,
        start: usize,
        id_start: usize,
        id_end: usize,
        name: IdentName<'arena>,
        import_kind: ImportKind,
        is_export: bool,
    ) -> Result<Statement<'arena>, ParseError> {
        // Already have: import <identifier>
        // Current token is `=`
        self.advance()?; // consume `=`

        let id = Identifier::simple(name, Span::new(id_start as u32, id_end as u32));

        let module_reference = if matches!(self.current_kind(), TokenKind::Identifier)
            && self.current_value() == "require"
            && matches!(self.peek_kind(), TokenKind::ParenOpen)
        {
            // `require("module")`
            let ref_start = self.current_pos().0;
            self.advance()?; // consume `require`
            self.advance()?; // consume `(`

            // Parse string literal
            if !matches!(self.current_kind(), TokenKind::String) {
                return Err(self.error_expected("string literal in require()"));
            }
            let expression = self.parse_string_literal()?;

            // Handle optional trailing comma before closing paren
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance()?;
            }

            let (_, ref_end) = self.current_pos();
            self.expect(&TokenKind::ParenClose)?;

            TSModuleReference::ExternalModuleReference(TSExternalModuleReference {
                expression,
                span: Span::new(ref_start as u32, ref_end as u32),
            })
        } else if self.at_reference_name() {
            // `A.B.C` (entity name) — must start with a name; a string / number /
            // empty reference (`import x = 'foo'`, `import x = 5`, `import x =`) is
            // a syntax error, matching acorn-typescript.
            //
            // A contextual type keyword is an ordinary name here, like everywhere else
            // (`import x = string`, `import x = number.inner`); a bare
            // `TokenKind::Identifier` test saw only the words the lexer never made a
            // `Keyword` and rejected them all. `at_reference_name` names what this head
            // IS, and its two guards are inert in practice — an import-equals cannot
            // nest inside a generator or async function — so in strict
            // mode the bar on `let` / `yield` is the same deferred early error in the
            // reference and binding spellings alike, while `void` is excluded by the
            // `Identifier` production in both. So `import x = let.y` parses (tsc and
            // prettier accept it; acorn rejects, but it is the shape oracle only) and
            // `import x = void.y` still rejects. The heritage head asks the same
            // predicate for the same reason.
            TSModuleReference::EntityName(self.parse_module_reference_entity_name()?)
        } else {
            return Err(self.error_expected("'require(...)' or a module reference after '='"));
        };

        // The `type` modifier is valid on an import-equals only for an external
        // module reference (`import type A = require('m')`); on an entity-name
        // alias (`import type A = B.C`) tsc rejects it at parse time (TS1392), and
        // acorn raises when `importKind === 'type'` and the reference is not a
        // `TSExternalModuleReference`. This governs both `import type X = …` and the
        // `export import type X = …` re-export form, which reach here with
        // `ImportKind::Type` alike.
        if matches!(import_kind, ImportKind::Type)
            && !matches!(
                module_reference,
                TSModuleReference::ExternalModuleReference(_)
            )
        {
            return Err(self.error_msg("an import alias cannot use 'import type'"));
        }

        let end = self.semicolon_end()?;

        Ok(Statement::TSImportEqualsDeclaration(
            TSImportEqualsDeclaration {
                id,
                module_reference,
                import_kind,
                is_export,
                span: Span::new(start as u32, end),
            },
        ))
    }
}
