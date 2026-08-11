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

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Parse a `ModuleItem`: an import/export declaration or any
    /// `StatementListItem`. Import/export declarations are valid only here — at the
    /// module top level (`parse`'s loop) and inside a TS `namespace`/`module` body.
    /// Every other statement context uses `parse_statement`, which rejects bare
    /// import/export, so a misplaced one is a syntax error (matching acorn's
    /// "'import' and 'export' may only appear at the top level"). `import(…)` /
    /// `import.meta` are expressions and are left to `parse_statement`.
    pub(crate) fn parse_module_item(&mut self) -> Result<Statement<'arena>, ParseError> {
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Export)) {
            return self.parse_export_declaration();
        }
        // `import …` declaration (but not the `import(…)` / `import.meta` expressions).
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Import))
            && !self.import_begins_expression()
        {
            return self.parse_import_declaration();
        }
        self.parse_statement()
    }

    /// Whether an `import` keyword at the current position begins an expression —
    /// the `import(…)` dynamic import or `import.meta` meta-property — rather than
    /// an import declaration. Callers have already confirmed `current` is `import`.
    fn import_begins_expression(&mut self) -> bool {
        matches!(self.peek_kind(), TokenKind::ParenOpen | TokenKind::Dot)
    }

    /// Error for an `import`/`export` declaration outside a module-item position.
    fn error_module_item_position(&self) -> ParseError {
        self.error_msg("'import' and 'export' may only appear at the top level")
    }

    pub(crate) fn parse_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        // A labeled statement, checked before the keyword dispatch below because a
        // `LabelIdentifier` may be a word the lexer turned into a `Keyword` token
        // (`async: …`, `string: …`, `let: …`) — those never reach the `Identifier`
        // arm, so the label reading was unreachable for every one of them while
        // `foo: …` worked. At statement start a name followed by `:` can only be a
        // label (a declaration never has one there, and an object literal starts
        // with `{`), so one token of lookahead settles it with no ambiguity.
        //
        // The gate is `at_reference_name`: plain identifiers, the contextual keywords,
        // `let` (barred only by the deferred strict-mode early error), `await` exactly
        // where the goal axis makes it an identifier, and `yield` only OUTSIDE a
        // generator — `LabelIdentifier[Yield] : [~Yield] `yield`` keeps that guard in
        // the production, so `function* g() { yield: ; }` is a parse error even though
        // the binding channel admits `yield` there. It short-circuits on a genuine
        // reserved word, so no `peek` runs for `if` / `for` / `return` / `function` /
        // … — only for tokens whose arms peek anyway. This is also the ONLY place the
        // label set is decided: `parse_labeled_statement` builds the name through a
        // wider channel that applies neither the goal axis nor `[~Yield]`, so the gate
        // must stay here (it debug-asserts this predicate for that reason).
        //
        // ⚠️ This widens who may be labelled, never WHAT may be labelled:
        // `LabelledItem : Statement | FunctionDeclaration`, so `label: let x = 1`
        // still rejects — the body goes through `parse_statement`, which parses a
        // lexical declaration and is then rejected by the labelled-item check.
        if self.at_reference_name() && self.peek_kind() == TokenKind::Colon {
            return self.parse_labeled_statement();
        }

        // Check if this is a variable declaration
        match self.current_kind() {
            TokenKind::Keyword(kw) => match kw {
                KeywordKind::Const => {
                    // Check for `const enum` declaration
                    if self.peek_kind() == TokenKind::Keyword(KeywordKind::Enum) {
                        self.parse_enum_declaration(true)
                    } else {
                        self.parse_variable_declaration()
                    }
                }
                KeywordKind::Var => self.parse_variable_declaration(),
                // `let` heads a declaration only when a binding follows (`let x`,
                // `let [a]`, `let {a}`); otherwise it is an ordinary
                // `IdentifierReference` and this is an expression statement (`let;`,
                // `let = 1`, `let.x = 1`, `let++`). See `Parser::at_let_declaration`
                // for the rule and for why `let [` still commits to the declaration.
                KeywordKind::Let => {
                    if self.at_let_declaration() {
                        self.parse_variable_declaration()
                    } else {
                        self.parse_expression_statement()
                    }
                }
                KeywordKind::Return => self.parse_return_statement(),
                KeywordKind::Function => {
                    // A `function` inside a `declare namespace`/`module` body carries no
                    // `declare` keyword of its own, so it is an ordinary function statement:
                    // `parse_function_or_overload` yields a `FunctionDeclaration` for a body
                    // and a `TSDeclareFunction` for a bodiless overload signature, exactly as
                    // at the top level. An ambient body is a static-semantic early-error (tsc
                    // TS1183) deferred to diagnostics — prettier formats it, so tsv parses it.
                    // (A *top-level* `declare function` is dispatched separately, in
                    // `parse_declare_statement_kind`, and keeps forcing a bodiless signature.)
                    self.parse_function_declaration()
                }
                KeywordKind::Class => self.parse_class_declaration(),
                KeywordKind::Enum => self.parse_enum_declaration(false),
                // `import`/`export` declarations are `ModuleItem`s — reachable only via
                // `parse_module_item` (the module top level and TS namespace/module
                // bodies). Reached here they are nested in a block, function body, or
                // single-statement position, where they are syntax errors. `import(…)`
                // and `import.meta` are ordinary expressions and stay valid anywhere.
                KeywordKind::Export => Err(self.error_module_item_position()),
                KeywordKind::Import => {
                    if self.import_begins_expression() {
                        self.parse_expression_statement()
                    } else {
                        Err(self.error_module_item_position())
                    }
                }
                KeywordKind::Async => {
                    // `async function` is a function declaration
                    // `async () => ...` or `async x => ...` is an expression
                    // peek_kind() skips comments between `async` and `function`
                    //
                    // `AsyncFunctionDeclaration : async [no LineTerminator here] function`
                    // (ecma262), the statement-level face of the rule the expression
                    // parser applies. A line break — including the one that ends a `//`
                    // in the gap — demotes `async` to an ordinary identifier expression
                    // and ASI splits the statement, so `async⏎function add() {}` is an
                    // expression statement plus a function declaration, as acorn and
                    // prettier read it, not one async function. Welding them was a
                    // re-meaning bug that also made the gap unprintable: the head emitter
                    // renders that gap inline, so a `//` in it swallowed the whole
                    // declaration head onto the comment's line.
                    if self.peek_kind() == TokenKind::Keyword(KeywordKind::Function)
                        && !self.peek_preceded_by_line_terminator()
                    {
                        self.parse_async_function_declaration()
                    } else {
                        // Async arrow function expression
                        self.parse_expression_statement()
                    }
                }
                KeywordKind::Await => {
                    // Script `[~Await]`: `await` is an ordinary identifier, so this is
                    // an expression statement (`await`, `await.x`, …) exactly like a
                    // plain identifier. The labeled form (`await: …`) was already
                    // dispatched by the `at_reference_name` check at the top.
                    if self.await_is_identifier() {
                        return self.parse_expression_statement();
                    }
                    // Check for `await using` declaration (Explicit Resource
                    // Management); a break at either gap, or a word-shaped binary
                    // operator (`await using in b`), leaves an expression statement
                    if self.at_await_using_declaration() {
                        return self.parse_await_using_declaration();
                    }
                    // Regular await expression (rejected by the expression parser
                    // when Module `[~Await]` — reserved with no `[+Await]`).
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
                // The `with` statement is sloppy-mode only and tsv is strict-only, so it
                // is refused at its keyword rather than by an accident further along.
                // (It used to lex as an identifier, which made `with (a);` read as a CALL
                // and reprint as `with(a);` — a sloppy-mode program silently
                // reinterpreted instead of rejected.)
                KeywordKind::With => Err(self.error_with_statement()),
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
                // Check for contextual keyword 'using' followed by a binding word
                // (Explicit Resource Management); a break makes `using` an identifier
                // statement, and an expression-continuation word (`in`/`instanceof`/
                // `as`/`satisfies`) keeps the expression reading
                if self.at_using_declaration() {
                    return self.parse_using_declaration();
                }
                // Contextual keyword `type` starts a type alias only when the name is
                // on the SAME line (tsc `nextTokenIsIdentifierOnSameLine`). A line
                // break demotes `type` to a plain identifier and ASI splits the
                // statement in two. The name may itself be a contextual type keyword
                // (`type any = …`). peek_kind() skips comments: `type /* c */ A = T`.
                if self.current_value() == "type" && self.peek_is_same_line_name_word() {
                    return self.parse_type_alias_declaration();
                }
                // Contextual keyword `interface` starts a declaration only when the
                // name is on the SAME line (tsc `nextTokenIsIdentifierOnSameLine`); a
                // line break demotes it to an identifier. The name may itself be a
                // contextual type keyword (`interface string {}`). peek_kind() skips comments.
                if self.current_value() == "interface" && self.peek_is_same_line_name_word() {
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
                // demotes them to identifiers. The name may itself be a contextual
                // type keyword (`namespace number {}`). Only `module` also takes a
                // string-literal name (`module 'x' {}`); acorn rejects
                // `namespace 'x'`. peek_kind() skips comments.
                if (matches!(self.current_value(), "namespace" | "module")
                    && self.peek_is_same_line_name_word())
                    || (self.current_value() == "module"
                        && self.peek_kind() == TokenKind::String
                        && !self.peek_preceded_by_line_terminator())
                {
                    return self.parse_module_declaration();
                }
                // Bare global augmentation: `global { … }` (no `declare`), at the
                // top level or nested in a `declare module`. Unlike namespace/module,
                // acorn imposes NO same-line rule — `global` followed by `{` (even
                // across a line break) is a `TSModuleDeclaration{global:true}`; only
                // the `{` disambiguates it from `global` as an identifier
                // (`global.x`, `global = …`). declare is false (acorn omits it).
                // peek_kind() skips comments.
                if self.current_value() == "global" && self.peek_kind() == TokenKind::BraceOpen {
                    let start = self.current_pos().0;
                    return self.parse_global_declaration(start, false);
                }
                // Regular expression statement (a `label:` was already dispatched at
                // the top of this function, for every identifier-shaped token alike)
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
    fn parse_expression_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
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
    pub(super) fn adapt_directive_prologue(&self, statements: &mut [Statement<'arena>]) {
        for stmt in statements {
            let Statement::ExpressionStatement(expr_stmt) = stmt else {
                break;
            };
            let Expression::Literal(lit) = &expr_stmt.expression else {
                break;
            };
            if !matches!(lit.value, LiteralValue::String(_)) {
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
