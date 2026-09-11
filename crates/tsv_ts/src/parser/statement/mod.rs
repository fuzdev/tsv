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

/// The canonical spelling of the decorator-list axis, since `parse_decorators` is
/// reached from two sides — the class parsers in this module and the generic
/// parameter-list parser in `crate::parser::parameters`, which cannot see `class`
/// directly. Every caller imports it from here, including the ones next door, so
/// there is one path to it rather than two.
pub(in crate::parser) use class::DecoratorListKind;

/// Which statement position a parse is in — the axis ecma262 draws between
/// `StatementListItem` (a declaration is admissible) and the single `Statement` an
/// `if` arm, a loop body, a `with` body or a labelled item takes (it is not).
///
/// It gates the reading of exactly one word: `let`. A **labelled** item bars the other
/// declarations too, but through a rule that reads the finished node
/// (`Parser::parse_labeled_statement`'s labelled-item check) rather than the token that
/// opens it — and that rule reaches labels alone. In any other nested body a `const` /
/// `class` / `function` declaration is one more deferred early error tsv parses
/// (`if (a) const x = 1;`, `for (;;) const x = 1;`, `while (a) class C {}`; acorn
/// rejects all three).
#[derive(Clone, Copy)]
enum StatementPosition {
    /// A `StatementList` — a `Program`, a block, a function body, a `switch` case.
    List,
    /// The single `Statement` a nested position takes.
    Single,
}

/// Which `ModuleItem` list a declaration is parsed in — the one fact the `import` /
/// `export` goal gate reads beyond the goal itself.
///
/// ecma262 has one such list, a `Module`'s body, where the goal decides. TypeScript adds
/// the namespace and ambient-module body, a module-item context at **either** goal: tsc
/// decides whether a file is a module from its top-level statements alone
/// (`isFileProbablyExternalModule`), so `namespace N { export const a = 1; }` is a script
/// holding an export, not a module. acorn-typescript refuses it at `sourceType: script` —
/// base acorn's check, run before the plugin reads the body — a cataloged divergence
/// (`docs/conformance_svelte.md` §TypeScript Corrections).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModuleItemContext {
    /// A `Program`'s own top level, where `Goal::Script` refuses the declaration.
    TopLevel,
    /// A TS `namespace` / `module` / `declare module` / `declare global` body.
    NamespaceBody,
}

impl<'a, 'arena> Parser<'a, 'arena> {
    /// Parse a `ModuleItem`: an import/export declaration or any
    /// `StatementListItem`. Import/export declarations are valid only here — at the
    /// module top level (`parse`'s loop) and inside a TS `namespace`/`module` body, as
    /// `context` says ([`Parser::check_module_item_goal`]). Every other statement
    /// context uses `parse_statement`, which rejects a bare import/export — a decorated
    /// `export` included — so a misplaced one is a syntax error (matching acorn's
    /// "'import' and 'export' may only appear at the top level"). `import(…)` /
    /// `import.meta` are expressions and are left to `parse_statement`.
    pub(crate) fn parse_module_item(
        &mut self,
        context: ModuleItemContext,
    ) -> Result<Statement<'arena>, ParseError> {
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Export)) {
            return self.parse_export_declaration(context);
        }
        // `import …` declaration (but not the `import(…)` / `import.meta` expressions).
        if matches!(self.current_kind(), TokenKind::Keyword(KeywordKind::Import))
            && !self.import_begins_expression()
        {
            return self.parse_import_declaration(context);
        }
        self.parse_statement_at(StatementPosition::List, Some(context))
    }

    /// Refuse a `ModuleItem`-only declaration — `keyword` names it — at `Goal::Script`
    /// (the **goal gate**), unless it sits in a namespace body ([`ModuleItemContext`]).
    /// `position` is the keyword, so the error points at the declaration's head. The one
    /// statement of the rule for the three places such a declaration is built:
    /// `parse_export_declaration`, `check_import_declaration_goal`, and the `export` of a
    /// decorated class.
    pub(super) fn check_module_item_goal(
        &self,
        context: ModuleItemContext,
        keyword: &str,
        position: usize,
    ) -> Result<(), ParseError> {
        if self.goal == crate::Goal::Module || context == ModuleItemContext::NamespaceBody {
            return Ok(());
        }
        let message = format!("'{keyword}' is only allowed in a module");
        Err(self.error_goal_gate_at(&message, position))
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

    /// Parse a `StatementListItem` — the statement position that admits a
    /// declaration beside a statement (a `Program`, a block, a function body, a
    /// `switch` case).
    pub(crate) fn parse_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        self.parse_statement_at(StatementPosition::List, None)
    }

    /// Parse the single `Statement` a nested position takes — an `if` arm, a `while` /
    /// `do` / `for` body, a `with` body, a labelled item.
    ///
    /// `IfStatement`, every iteration statement and `WithStatement` take a `Statement`,
    /// and `LabelledItem : Statement | FunctionDeclaration`, so a `LexicalDeclaration`
    /// cannot appear here at all. That is exactly what changes the reading of `let`:
    /// with no declaration to compete with, `let` is the `IdentifierReference` its own
    /// production admits, and ASI closes the statement at it. acorn spells the same fork
    /// as the `context` parameter of `parseStatement` / `isLet`.
    pub(super) fn parse_nested_statement(&mut self) -> Result<Statement<'arena>, ParseError> {
        self.parse_statement_at(StatementPosition::Single, None)
    }

    /// `module_item` is the `ModuleItem` list the statement sits in — `None` everywhere
    /// but `parse_module_item` — read only by the decorated-class arm: a decorator run
    /// is the one statement head an `export` can follow.
    fn parse_statement_at(
        &mut self,
        position: StatementPosition,
        module_item: Option<ModuleItemContext>,
    ) -> Result<Statement<'arena>, ParseError> {
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
        // `LabelledItem : Statement | FunctionDeclaration`, so `label: const x = 1`
        // still rejects — the body goes through `parse_nested_statement`, which builds
        // the declaration and is then rejected by the labelled-item check. `label: let`
        // never reaches that check at all: a labelled item is a `StatementPosition::Single`
        // position, where `let` reads as an `IdentifierReference` (see
        // [`StatementPosition`]), so `label: let x = 1` dies at the missing `;` instead.
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
                //
                // In a single-statement position there is no declaration to compete
                // with, so the reference reading is the only one — except for that same
                // `let [`, which `ExpressionStatement`'s lookahead bars from being an
                // expression, leaving no reading at all.
                KeywordKind::Let => match position {
                    StatementPosition::List if self.at_let_declaration() => {
                        self.parse_variable_declaration()
                    }
                    StatementPosition::Single
                        if matches!(self.peek_kind(), TokenKind::BracketOpen) =>
                    {
                        Err(self.error_msg(
                            "A lexical declaration cannot appear in a single-statement context",
                        ))
                    }
                    _ => self.parse_expression_statement(),
                },
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
                // The `with` statement is sloppy-mode only: strict code disallows it
                // by an early error keyed on `IsStrict`, so the verdict is read here,
                // at the keyword, from the enclosing code's settled strictness.
                // (`with` stays a reserved word in every mode — lexing it as an
                // identifier would make `with (a);` read as a CALL and reprint as
                // `with(a);`, a sloppy-mode program silently reinterpreted rather
                // than parsed or rejected.)
                KeywordKind::With => {
                    if self.strict {
                        Err(self.error_with_statement())
                    } else {
                        self.parse_with_statement()
                    }
                }
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
                self.parse_decorated_class(module_item)
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
        let expr = self.parse_expression_ref()?;
        let end = self.semicolon_end()?;
        Ok(Statement::ExpressionStatement(ExpressionStatement {
            expression: expr,
            span: Span::new(start, end),
            is_directive: false,
        }))
    }

    /// Mark a just-parsed statement as a directive while a `Program` or function
    /// body's **directive prologue** is still open, returning whether the prologue
    /// continues past it.
    ///
    /// Mirrors acorn's `adaptDirectivePrologue`: the leading run of unparenthesized
    /// string-literal expression statements are directives (`"use strict";` and
    /// friends), and the first statement that is not a directive candidate ends the
    /// run. It is called *per statement* rather than over the finished body because
    /// a `"use strict"` directive changes how the statements after it are graded —
    /// the strictness must be established before the next statement's tokens are
    /// consumed, not after the body is closed.
    ///
    /// A directive's text is compared against the source **byte-exactly**, quotes
    /// included: ecma262 sec-directive-prologues defines a Use Strict Directive as
    /// one whose string literal is the exact code point sequence `use strict` with
    /// no escapes and no line continuations, so `"use\u0020strict"` is an ordinary
    /// directive that does not turn strict mode on.
    ///
    /// `prologue` is the directives this body has already collected — every statement
    /// ahead of `stmt`, since the loop stops calling this at the first one that is not a
    /// directive. Turning strict mode on re-grades them: a legacy string escape in an
    /// *earlier* prologue literal is retroactively a syntax error, so
    /// `function f() { '\7'; 'use strict'; }` is rejected (ecma262
    /// sec-literals-string-literals: implementations must enforce the strict rules for
    /// such literals). Only a sloppy body that turns strict walks the buffer, and a
    /// directive prologue is a handful of short literals.
    pub(super) fn note_directive(
        &mut self,
        stmt: &mut Statement<'arena>,
        prologue: &[Statement<'arena>],
    ) -> Result<bool, ParseError> {
        let Statement::ExpressionStatement(expr_stmt) = stmt else {
            return Ok(false);
        };
        let Expression::Literal(lit) = &expr_stmt.expression else {
            return Ok(false);
        };
        if !matches!(lit.value, LiteralValue::String(_)) {
            return Ok(false);
        }
        // Reject parenthesized strings: the statement must open with a quote.
        let local_start = (expr_stmt.span.start as usize).saturating_sub(self.base_offset);
        let Some(rest @ [b'"' | b'\'', ..]) = self.source.as_bytes().get(local_start..) else {
            return Ok(false);
        };
        expr_stmt.is_directive = true;
        if rest.starts_with(USE_STRICT_DOUBLE) || rest.starts_with(USE_STRICT_SINGLE) {
            let was_strict = self.strict;
            self.strict = true;
            // Already-strict code graded each earlier literal at consumption, so
            // only the false→true flip has anything to re-read.
            if !was_strict {
                for earlier in prologue {
                    if let Some(err) = self.directive_legacy_escape_error(earlier) {
                        return Err(err);
                    }
                }
            }
        }
        Ok(true)
    }

    /// The retroactive half of the legacy-escape gate: the error a prologue directive
    /// carries now that the body has turned strict, if it carries one. The literal's token
    /// is long gone, so the question is asked of its raw source — the same walk the
    /// string-literal seam runs (`Parser::legacy_escape_error`).
    fn directive_legacy_escape_error(&self, stmt: &Statement<'arena>) -> Option<ParseError> {
        let Statement::ExpressionStatement(expr_stmt) = stmt else {
            return None;
        };
        let Expression::Literal(lit) = &expr_stmt.expression else {
            return None;
        };
        // The literal's own span, not the statement's: the two differ by the trailing
        // semicolon, and a directive is never parenthesized.
        let start = (lit.span.start as usize).saturating_sub(self.base_offset);
        let end = (lit.span.end as usize).saturating_sub(self.base_offset);
        let inner = self.source.get(start + 1..end.saturating_sub(1))?;
        self.legacy_escape_error(inner, start + 1 + self.base_offset)
    }
}

/// The two byte-exact spellings of a Use Strict Directive, quotes included. A
/// literal whose closing quote is the twelfth byte can hold nothing but these ten
/// characters, so a prefix match on the statement's source is the whole test.
const USE_STRICT_DOUBLE: &[u8] = b"\"use strict\"";
const USE_STRICT_SINGLE: &[u8] = b"'use strict'";
