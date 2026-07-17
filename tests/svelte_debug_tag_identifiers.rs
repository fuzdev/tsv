//! `{@debug}` arguments must be plain identifiers.
//!
//! Svelte's parser (`1-parse/state/tag.js`) reads the `{@debug …}` content as an
//! expression, then rejects it unless **every** argument is an `Identifier`
//! (`e.debug_tag_invalid_arguments`, "`{@debug ...}` arguments must be
//! identifiers, not arbitrary expressions"). tsv previously parsed each
//! comma-separated argument as an arbitrary TS expression and never checked, so
//! it over-accepted regexes, member/call/binary expressions, and `this`.
//!
//! The regex case additionally produced **unreparseable output**: the fuzzer
//! mutant `{@debug , /ug* c */ b}` parses `/ug* c */` as a regex literal (dropping
//! the trailing `b`), and the printer re-emits the regex source immediately before
//! `}` (`{@debug /ug* c */}`), which the parser then rejects (`Unclosed block
//! tag`). The fuzzer surfaced this as a seed-42 `unreparseable` finding; rejecting
//! the input at parse time (as Svelte does) removes it.
//!
//! Canonical Svelte rejects every `INVALID` case below with
//! `debug_tag_invalid_arguments` and accepts every `VALID` case (verified via
//! `tsv_debug canonical_parse`). Pinned as a Rust test rather than an
//! `input_invalid_*` fixture: a new fixture file reshuffles the corpus-sensitive
//! `fuzz:audit` seed-0 sample onto the next latent bug (see the fuzzer-backlog
//! lore); a parser-rejection assertion has no such coupling.

/// `true` if tsv's Svelte parser accepts `src`.
fn accepts(src: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_svelte::parse(src, &arena).is_ok()
}

/// Every argument is a plain identifier — accepted (matching Svelte).
#[test]
fn accepts_identifier_arguments() {
    const VALID: &[&str] = &[
        "{@debug}",              // "debug all"
        "{@debug a}",            // single identifier
        "{@debug a, b, c}",      // identifier list
        "{@debug /* c */ a}",    // leading comment + identifier (preserved)
        "{@debug a, /* c */ b}", // comment between identifiers
        "{@debug $state}",       // `$`-prefixed identifier
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
        "{@debug /x/}",    // regex literal — the unreparseable trigger
        "{@debug a.b}",    // member expression
        "{@debug a?.b}",   // optional member expression
        "{@debug foo()}",  // call expression
        "{@debug a + b}",  // binary expression
        "{@debug this}",   // `this` expression
        "{@debug true}",   // boolean literal (not the `undefined` identifier)
        "{@debug a, b.c}", // one valid + one invalid argument
    ];
    for src in INVALID {
        assert!(
            !accepts(src),
            "tsv should reject `{src}` (arg is not an identifier)"
        );
    }
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
