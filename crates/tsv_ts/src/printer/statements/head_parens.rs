// Statement-head parens: which parens the printer must KEEP or SYNTHESIZE because
// the statement's own position gives its first token a second reading.
//
// Three questions, one concern:
// - lookahead restrictions (`ExpressionStatement`'s `let [`, a for-of head's `let`) —
//   dropping the paren changes what the statement IS, or stops it parsing
// - declaration-starter ambiguity (`(type) as T;`, `(using) as T;`) — some parser that
//   reads the output (tsv's own, tsc, acorn at ES2026) commits to the declaration
//   reading, so the bare form does not reparse there
// - directive-prologue avoidance (`('use strict');`) — a bare string statement in a
//   `Program`/`BlockStatement` would become a directive
//
// All of them are RECOMPUTED from the AST, never preserved from source, so a redundant
// source paren is still stripped and any number collapse to exactly one.

use super::Printer;
use crate::ast::internal::{self, Expression, ExpressionKind};
use crate::printer::is_string_literal;
use crate::printer::needs_parens::{leftmost_no_lookahead, leftmost_no_lookahead_reached};
use tsv_lang::Span;

/// Strip only `as`/`satisfies` casts from the head of a statement expression,
/// returning the innermost operand — but only if at least one cast was peeled.
/// Mirrors prettier's `ancestorNeitherAsNorSatisfies` walk
/// (parentheses/identifier.js): unlike `leftmost_no_lookahead` it does NOT descend
/// through member/call heads (`type.foo` is unambiguous), so it fires only for a
/// bare-identifier operand of a cast chain.
fn strip_statement_casts<'a>(expr: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let mut cur = expr;
    let mut stripped = false;
    loop {
        cur = match &cur.kind {
            ExpressionKind::TSAsExpression(e) => e.expression,
            ExpressionKind::TSSatisfiesExpression(e) => e.expression,
            _ => break,
        };
        stripped = true;
    }
    stripped.then_some(cur)
}

/// Identifier names whose **bare** `<kw> as T` / `<kw> satisfies T` at statement position
/// some parser that reads tsv's output takes for the head of a DECLARATION — so the pair
/// around the word is kept (and synthesized), and the cast reaches every reader as the
/// expression it is. Where a reader objects, it rejects the bare form rather than rereading
/// it as another program:
///
/// - `type` / `module` / `namespace` / `interface` / `let` — a declaration keyword
///   (`type <name> = …`, `namespace <name> { … }`), which tsv's own parser commits to as
///   well, so bare `namespace as T;` is one tsv cannot reparse;
/// - `using` — an ES2026 `using` declaration binding a name `as` / `satisfies`, for tsc and
///   for acorn at `ecmaVersion: 'latest'`. The canonical parsers run acorn at ES2025, where
///   `using` is no keyword and the bare form is the cast tsv's parse follows; the pair is
///   the spelling both editions read alike;
/// - `accessor` / `readonly` — a modifier tsc reads ahead of a declaration at statement
///   start (`Declaration or statement expected.`);
/// - `yield` / `await` — a `yield` / `await` expression to tsc. Neither is an identifier in
///   module code, so they arrive only through the format path's Script fallback;
/// - `component` / `hook` — declaration keywords in prettier's Flow dialect; bare they
///   reparse everywhere, and they are here only to keep prettier's pair (its
///   `parentheses/identifier.js` list, which this set otherwise matches or widens).
///
/// `let` is in the set for the cast position only; its other three positions are
/// lookahead restrictions on the statement grammar rather than a declaration-starter
/// ambiguity, and live in [`Printer::let_bracket_head_target`] /
/// [`Printer::for_in_of_let_head_target`].
fn is_statement_ambiguous_keyword(name: &str) -> bool {
    matches!(
        name,
        "type"
            | "module"
            | "namespace"
            | "interface"
            | "let"
            | "using"
            | "accessor"
            | "readonly"
            | "yield"
            | "await"
            | "component"
            | "hook"
    )
}

/// Identifier names whose bare cast heading a `for` INIT clause reads as a declaration head
/// — `for (using as T; ;)` is a `using` declaration to tsc and to ES2026 acorn, and
/// `for (let as T; ;)` a `let` one to tsc, which commits a `for (let` head on the keyword.
/// Narrower than [`is_statement_ambiguous_keyword`]: the init is no statement start, so
/// `type` / `namespace` / the modifiers are ordinary identifiers there.
fn is_for_init_ambiguous_keyword(name: &str) -> bool {
    matches!(name, "using" | "let")
}

impl<'a> Printer<'a> {
    /// Whether an expression statement opens with a grouping `(` the print may keep
    /// or drop — the precondition for claiming the comments between it and the
    /// expression.
    ///
    /// `span.start < expr_start` alone does NOT prove one: the Svelte compiler prints
    /// a **synthesized** program through `format_canonical`, where a statement's span
    /// points into a generated buffer and need not open at its own expression. Reading
    /// that as a paren claimed comments the statement-list gap owns, and printed them
    /// twice. The byte settles it.
    pub(in crate::printer::statements) fn statement_opens_with_paren(
        &self,
        span: Span,
        expr_start: u32,
    ) -> bool {
        span.start < expr_start && self.source.as_bytes().get(span.start as usize) == Some(&b'(')
    }

    /// The nested span an *unwrapped* expression statement must parenthesize around
    /// ITSELF, or `None` when nothing does — the leftmost object / function / class
    /// (`(class {}).foo`, `({}).foo`, `(class {}) + 1`), a `let` heading a computed
    /// member (`(let)[a] = 1;`), or a contextual keyword heading an `as` / `satisfies`
    /// cast (`(type) as T;` / `(module) satisfies U;`, which reparse as a `type` /
    /// `module` declaration without the parens).
    ///
    /// Asked only where the whole expression isn't already wrapped. Two callers, one
    /// question: the ordinary path hands the span to `expr_stmt_paren_target` for the
    /// matching node's doc builder to consume, and the format-ignore path — which has no
    /// interior to hand it to — reads it as "the frozen slice needs a shell".
    pub(in crate::printer::statements) fn expr_stmt_nested_paren_target(
        &self,
        expression: &Expression<'_>,
    ) -> Option<Span> {
        let leftmost = leftmost_no_lookahead(expression);
        if matches!(
            leftmost.kind,
            ExpressionKind::ObjectExpression(_)
                | ExpressionKind::FunctionExpression(_)
                | ExpressionKind::ClassExpression(_)
        ) {
            return Some(leftmost.span());
        }
        if let Some(span) = self.let_bracket_head_target(expression) {
            return Some(span);
        }
        match strip_statement_casts(expression) {
            Some(Expression {
                kind: ExpressionKind::Identifier(id),
                ..
            }) if self.with_ident_name(id, is_statement_ambiguous_keyword) => Some(id.span),
            _ => None,
        }
    }

    /// The `let` identifier that must keep its parens because the statement it heads
    /// would otherwise be read as a **declaration**, or `None`.
    ///
    /// `ExpressionStatement : [lookahead ∉ { `{`, `function`, `async function`, `class`,
    /// `let [` }] Expression ;` — so `let[a] = 1;` is a `VariableDeclaration` binding the
    /// array pattern `[a]`, while `(let)[a] = 1;` assigns to the member `let[a]`. Dropping
    /// the parens does not merely relocate a token: it changes what the statement IS, and
    /// (for a non-binding index like `let[0]`) produces text that no longer parses.
    ///
    /// The restriction is on the statement's first two tokens, so the `let` must be the
    /// **leftmost** node and the bracket must be its own — a computed, non-optional
    /// member whose object it is. `let.a = 1;`, `let()[a] = 1;` and `foo[let[a]] = 1;`
    /// are all unrestricted and stay bare, matching prettier's
    /// `shouldAddParenthesesToIdentifier` clause for `key === "object"`.
    ///
    /// Shared by the three positions the same restriction covers: an expression
    /// statement, a `for` init, and a for-in left (the for-of / for-in LEFT takes the
    /// wider [`Printer::for_in_of_let_head_target`] instead).
    pub(in crate::printer) fn let_bracket_head_target(
        &self,
        expression: &Expression<'_>,
    ) -> Option<Span> {
        let (leftmost, is_computed_member_object) = leftmost_no_lookahead_reached(expression);
        let ExpressionKind::Identifier(id) = &leftmost.kind else {
            return None;
        };
        (is_computed_member_object && self.with_ident_name(id, |name| name == "let"))
            .then_some(id.span)
    }

    /// The identifier a C-style `for` INIT clause must keep its parens around, or `None`:
    /// the `let [` restriction the init shares with a statement
    /// ([`Printer::let_bracket_head_target`]), or a word heading a cast chain that would
    /// open a declaration there ([`is_for_init_ambiguous_keyword`]).
    pub(in crate::printer) fn for_init_head_target(
        &self,
        expression: &Expression<'_>,
    ) -> Option<Span> {
        self.let_bracket_head_target(expression).or_else(|| {
            match strip_statement_casts(expression) {
                Some(Expression {
                    kind: ExpressionKind::Identifier(id),
                    ..
                }) if self.with_ident_name(id, is_for_init_ambiguous_keyword) => Some(id.span),
                _ => None,
            }
        })
    }

    /// The `let` identifier a for-in / for-of LEFT must keep its parens around, or `None`.
    ///
    /// Wider than [`Printer::let_bracket_head_target`] by design: the for-of head carries
    /// its own `[lookahead ∉ { `let` }]`, so a bare `for (let of foo);` is a syntax error
    /// however the head continues — `(let)`, `(let).a`, `(let)[a]` and `(let)().a` all
    /// need the parens, not just the bracket form. Prettier draws the same line (its
    /// `startsWithNoLookaheadToken` clause finds any enclosing for-in/of), which is why
    /// it *moves* an author's `(let.a)` onto the identifier as `(let).a`.
    pub(in crate::printer) fn for_in_of_let_head_target(
        &self,
        expression: &Expression<'_>,
    ) -> Option<Span> {
        match &leftmost_no_lookahead(expression).kind {
            ExpressionKind::Identifier(id) if self.with_ident_name(id, |name| name == "let") => {
                Some(id.span)
            }
            _ => None,
        }
    }

    /// Whether a bare string-literal expression statement needs synthetic parens
    /// to avoid being read as a directive-prologue entry.
    ///
    /// Mirrors Prettier's `needs-parentheses.js` `StringLiteral`/`Literal` case:
    /// recomputed fresh from AST structure, never preserved from source. A
    /// non-directive string statement gets parens exactly when its immediate
    /// container is a `Program` or `BlockStatement` (`in_program_or_block`) —
    /// plain blocks, `if`/`for`/`while`/`try`/`catch` bodies, and function/arrow/
    /// method bodies all qualify; `SwitchCase`, `StaticBlock`, and
    /// `TSModuleBlock` (namespace) bodies don't. Because this is recomputed
    /// rather than preserved, redundant source parens are stripped in an
    /// ineligible container (`static { ('x'); }` → `'x';`) and any number of
    /// source parens collapse to exactly one where they're needed.
    ///
    /// Only called from the `!stmt.is_directive` branch of
    /// `build_expression_statement_doc`, so a real directive never reaches here.
    pub(in crate::printer::statements) fn needs_avoid_directive_parens(
        &self,
        stmt: &internal::ExpressionStatement<'_>,
        in_program_or_block: bool,
    ) -> bool {
        in_program_or_block && is_string_literal(stmt.expression)
    }
}
