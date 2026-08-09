// Statement-head parens: which parens the printer must KEEP or SYNTHESIZE because
// the statement's own position gives its first token a second reading.
//
// Three questions, one concern:
// - lookahead restrictions (`ExpressionStatement`'s `let [`, a for-of head's `let`) —
//   dropping the paren changes what the statement IS, or stops it parsing
// - declaration-starter ambiguity (`(type) as T;`) — tsv's parser commits to the
//   declaration reading, so the bare form does not reparse
// - directive-prologue avoidance (`('use strict');`) — a bare string statement in a
//   `Program`/`BlockStatement` would become a directive
//
// All of them are RECOMPUTED from the AST, never preserved from source, so a redundant
// source paren is still stripped and any number collapse to exactly one.

use super::Printer;
use crate::ast::internal::{self, Expression};
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
        cur = match cur {
            Expression::TSAsExpression(e) => e.expression,
            Expression::TSSatisfiesExpression(e) => e.expression,
            _ => break,
        };
        stripped = true;
    }
    stripped.then_some(cur)
}

/// Contextual-keyword identifier names whose **bare** `<kw> as T` / `<kw> satisfies T`
/// at statement position tsv's parser *rejects* — it commits to a declaration reading
/// (`type <name> = …` alias, `module <name> { … }` namespace) and errors when no
/// `=`/`{` follows. Dropping the source parens on `(type) as T` would make the output
/// unreparseable, so the parens are kept.
///
/// This is deliberately tsv's reject-set, NOT prettier's full identifier list
/// (parentheses/identifier.js also lists `await`/`yield`/`component`/`hook`).
/// ⚠️ **The membership test is whether the parser rejects the word BARE at statement
/// position — a rejection is the reason to KEEP the parens, not a reason the shape never
/// reaches the formatter.** Only `await` is out for the strong reason: it never arrives
/// at all (tsv rejects `(await)` itself at `Goal::Module`). `using` is out on purpose —
/// tsv **accepts** bare `using as T` (a cast, per its acorn oracle) and keeps it bare, a
/// deliberate divergence pinned by
/// `typescript_specific/using/cast_prettier_divergence`; wrapping it would break that.
///
/// TODO: `yield` / `component` / `hook` are out only because their bare cast *reparses*
/// — which is not the same as agreeing with prettier, and tsv does not: prettier keeps
/// those parens, tsv strips them (`(yield) as never;` → `yield as never;`). That is an
/// uncataloged divergence in either direction. Match prettier by adding the three
/// (nothing else changes — they are ordinary identifiers), or sanction the strip with a
/// `_prettier_divergence` fixture and a catalog entry.
///
/// `let` is in the set for the cast position only; its other three positions are
/// lookahead restrictions on the statement grammar rather than a declaration-starter
/// ambiguity, and live in [`Printer::let_bracket_head_target`] /
/// [`Printer::for_in_of_let_head_target`].
fn is_statement_ambiguous_keyword(name: &str) -> bool {
    matches!(name, "type" | "module" | "interface" | "let")
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
            leftmost,
            Expression::ObjectExpression(_)
                | Expression::FunctionExpression(_)
                | Expression::ClassExpression(_)
        ) {
            return Some(leftmost.span());
        }
        if let Some(span) = self.let_bracket_head_target(expression) {
            return Some(span);
        }
        match strip_statement_casts(expression) {
            Some(Expression::Identifier(id))
                if self.with_ident_name(id, is_statement_ambiguous_keyword) =>
            {
                Some(id.span)
            }
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
        let Expression::Identifier(id) = leftmost else {
            return None;
        };
        (is_computed_member_object && self.with_ident_name(id, |name| name == "let"))
            .then_some(id.span)
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
        match leftmost_no_lookahead(expression) {
            Expression::Identifier(id) if self.with_ident_name(id, |name| name == "let") => {
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
        in_program_or_block && is_string_literal(&stmt.expression)
    }
}
