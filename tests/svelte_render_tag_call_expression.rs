//! `{@render}` expressions must be call expressions.
//!
//! Svelte's parser (`1-parse/state/tag.js`) reads the `{@render …}` content as one
//! expression (`read_expression`), then rejects it unless the expression is a
//! `CallExpression`, **or** a `ChainExpression` whose inner `.expression` is a
//! `CallExpression` (`e.render_tag_invalid_expression`, "`{@render ...}` tags can
//! only contain call expressions"). tsv previously parsed the content as an
//! arbitrary TS expression and never checked, so it over-accepted `{@render foo}`,
//! `{@render a.b}`, and other non-call forms.
//!
//! tsv has no distinct `ChainExpression` node — an optional chain folds into the
//! member/call node it wraps (`Expression::has_optional_in_chain` drives the wire
//! `ChainExpression` wrap at serialization). So Svelte's two-branch check collapses
//! to a single one against tsv's internal AST: the top-level expression is a
//! `CallExpression`. `foo()` is a bare `CallExpression`; `foo?.()` is a
//! `CallExpression` with `optional: true` (wire `ChainExpression > CallExpression`);
//! both have a tsv top node of `CallExpression`. Every rejected form has a different
//! top node — `a?.b` is a `MemberExpression` (wire `ChainExpression > MemberExpression`,
//! Svelte's rejected chain shape), `foo()!` a `TSNonNullExpression`, `foo` an
//! `Identifier`, and so on.
//!
//! Canonical Svelte rejects every `INVALID` case below and accepts every `VALID`
//! case (verified via `tsv_debug canonical_parse`). Pinned as a Rust test rather
//! than an `input_invalid_*` fixture: a new fixture file reshuffles the
//! corpus-sensitive `fuzz:audit` seed-0 sample onto the next latent bug (see the
//! fuzzer-backlog lore); a parser-rejection assertion has no such coupling.

/// `true` if tsv's Svelte parser accepts `src`.
fn accepts(src: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_svelte::parse(src, &arena).is_ok()
}

/// A call expression (or an optional chain ending in one) — accepted, matching
/// Svelte. Includes the chain forms `foo?.()` and `a?.b()`, which tsv models as a
/// `CallExpression` carrying an optional and serializes as `ChainExpression >
/// CallExpression` (Svelte's accepted chain shape), plus the parenthesized
/// `(foo())`, whose grouping parens are stripped to a bare `CallExpression`.
#[test]
fn accepts_call_expressions() {
    const VALID: &[&str] = &[
        "{@render foo()}",   // bare call
        "{@render foo?.()}", // optional call — ChainExpression > CallExpression
        "{@render a.b.c()}", // call on a member chain
        "{@render foo()()}", // call of a call
        "{@render a?.b()}",  // optional member, then call — ChainExpression > CallExpression
        "{@render (foo())}", // grouping parens stripped → CallExpression
    ];
    for src in VALID {
        assert!(
            accepts(src),
            "tsv should accept `{src}` (a call expression)"
        );
    }
}

/// A non-call expression — rejected, matching Svelte's
/// `render_tag_invalid_expression`. `a?.b` is the rejected chain shape (an optional
/// chain ending in a member, not a call); `foo()!` seals the call in a non-null
/// assertion, so the top node is no longer a call.
#[test]
fn rejects_non_call_expressions() {
    const INVALID: &[&str] = &[
        "{@render foo}",       // identifier
        "{@render a.b}",       // member expression
        "{@render a?.b}",      // optional member — ChainExpression > MemberExpression
        "{@render foo()!}",    // non-null assertion sealing a call
        "{@render (a, b)}",    // sequence expression
        "{@render foo + bar}", // binary expression
        "{@render 1}",         // numeric literal
    ];
    for src in INVALID {
        assert!(
            !accepts(src),
            "tsv should reject `{src}` (not a call expression)"
        );
    }
}
