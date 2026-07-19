// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! `{@debug}` arguments must be plain identifiers.
//!
//! Svelte's parser (`1-parse/state/tag.js`) reads the whole `{@debug …}` content
//! as one expression (`read_expression`), flattens a top-level comma sequence
//! into the identifier list, then rejects it unless **every** element is an
//! `Identifier` (`e.debug_tag_invalid_arguments`, "`{@debug ...}` arguments must
//! be identifiers, not arbitrary expressions"). tsv previously parsed each
//! comma-separated argument as an arbitrary TS expression via a hand-rolled
//! comment-blanking scan plus `str::split(',')`, and never checked, so it
//! over-accepted regexes, member/call/binary expressions, and `this`.
//!
//! The regex case additionally produced **unreparseable output**: the fuzzer
//! mutant `{@debug , /ug* c */ b}` parses `/ug* c */` as a regex literal (dropping
//! the trailing `b`), and the printer re-emits the regex source immediately before
//! `}` (`{@debug /ug* c */}`), which the parser then rejects (`Unclosed block
//! tag`). The fuzzer surfaced this as a seed-42 `unreparseable` finding; rejecting
//! the input at parse time (as Svelte does) removes it.
//!
//! Parsing the content as one expression (mirroring `read_expression`) also fixes
//! the split-based scan's blind spots: a comma inside parens is not a top-level
//! separator, so `{@debug (a, b)}` is one parenthesized `SequenceExpression` whose
//! elements are both identifiers (accepted, `[a, b]`); and a stray/leading/trailing
//! comma (`{@debug a,}`, `{@debug ,a}`, `{@debug a, , b}`) or a comment-only body
//! (`{@debug /* c */}`) is a parse error, matching Svelte, instead of being
//! silently accepted with the malformed slot dropped.
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

/// Parse and format `src` (panics if `src` does not parse).
fn format(src: &str) -> String {
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(src, &arena).expect("src should parse");
    tsv_svelte::format(&root, src)
}

/// Every argument is a plain identifier — accepted (matching Svelte). Includes
/// the parenthesized-sequence forms: a comma inside `()` is not a top-level
/// separator, so `(a, b)` is one `SequenceExpression` flattened to `[a, b]`.
#[test]
fn accepts_identifier_arguments() {
    const VALID: &[&str] = &[
        "{@debug}",              // "debug all"
        "{@debug a}",            // single identifier
        "{@debug a, b, c}",      // identifier list
        "{@debug /* c */ a}",    // leading comment + identifier (preserved)
        "{@debug a, /* c */ b}", // comment between identifiers
        "{@debug $state}",       // `$`-prefixed identifier
        "{@debug(a,b)}",         // no whitespace after the keyword
        "{@debug (a, b)}",       // parenthesized sequence → flattened to [a, b]
        "{@debug (a, b, c)}",    // parenthesized three-element sequence
    ];
    for src in VALID {
        assert!(
            accepts(src),
            "tsv should accept `{src}` (all args are identifiers)"
        );
    }
}

/// A non-identifier argument — rejected (matching Svelte's
/// `debug_tag_invalid_arguments`).
#[test]
fn rejects_non_identifier_arguments() {
    const INVALID: &[&str] = &[
        "{@debug /x/}",      // regex literal — the unreparseable trigger
        "{@debug a.b}",      // member expression
        "{@debug a?.b}",     // optional member expression
        "{@debug foo()}",    // call expression
        "{@debug a + b}",    // binary expression
        "{@debug this}",     // `this` expression
        "{@debug true}",     // boolean literal (not the `undefined` identifier)
        "{@debug a, b.c}",   // one valid + one invalid argument
        "{@debug (a, b.c)}", // parenthesized sequence with a non-identifier element
    ];
    for src in INVALID {
        assert!(
            !accepts(src),
            "tsv should reject `{src}` (arg is not an identifier)"
        );
    }
}

/// A malformed argument list — a stray comma, an empty slot, a comment-only
/// body, or a trailing token — is a parse error, matching Svelte's
/// `read_expression` + `eat('}', true)`. The former split-based scan silently
/// accepted these, dropping the empty slot.
#[test]
fn rejects_malformed_argument_lists() {
    const INVALID: &[&str] = &[
        "{@debug a,}",      // trailing comma
        "{@debug ,a}",      // leading comma
        "{@debug a, , b}",  // empty middle slot
        "{@debug /* c */}", // comment-only body (no expression)
        "{@debug a b}",     // trailing token after the expression
    ];
    for src in INVALID {
        assert!(
            !accepts(src),
            "tsv should reject `{src}` (malformed argument list)"
        );
    }
}

/// `{@debug (a, b)}` and `{@debug a, b}` parse to the same identifier list
/// (Svelte strips the parens via `remove_parens`), so they format identically —
/// matching prettier, which normalizes `(a, b)` → `a, b`.
#[test]
fn parenthesized_sequence_formats_like_bare_sequence() {
    assert_eq!(format("{@debug (a, b)}"), format("{@debug a, b}"));
}

/// The exact seed-42 fuzzer finding: a stray `/` makes the argument a regex
/// literal that swallows trailing content. Formerly accepted and formatted to
/// `{@debug /ug* c */}`, which no longer reparses (`Unclosed block tag`). Must be
/// rejected at parse time, not formatted into unreparseable output.
#[test]
fn rejects_regex_argument_that_would_format_unreparseable() {
    assert!(
        !accepts("{@debug , /ug* c */ b}"),
        "regex-literal debug arg must be rejected, not formatted to unreparseable output"
    );
}
