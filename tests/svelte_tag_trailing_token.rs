//! A template expression must consume its whole slice — a trailing token is rejected.
//!
//! Every Svelte construct that embeds a single TS expression — the `{@html}` /
//! `{@render}` / `{@const}`-init / `{@debug}`-arg / `{@attach}` tags, the `{...spread}`
//! attribute, the bare `{expr}` mustache (element children and attribute values), and
//! the `{#if}` / `{#key}` tests and `{#each … (key)}` key — parses its content as one
//! expression and then requires the closing `}`. The pattern-bearing constructs — the
//! `{@const}` binding id and the `{:then}` / `{:catch}` value patterns — do the same with
//! a pattern. Svelte enforces this with `eat('}', true)`; a token left over after the
//! expression/pattern is a hard error ("Expected token }").
//!
//! tsv routes the expression callers through `tsv_ts::parse_expression_with_comments` and
//! the pattern callers through `tsv_ts::parse_pattern_with_comments`; both previously
//! returned without checking they had reached the end of their slice — so a trailing
//! token was silently DROPPED (`{@html a b}` → `{@html a}`, `<p>{a b}</p>` → `<p>{a}</p>`,
//! `{@const x y = a}` → `{@const x = a}`). That is content loss, not a divergence, and it
//! is fuzzer-invisible: the truncated output is itself idempotent and reparseable. The
//! fix is a shared end-of-slice check (`Parser::expect_end_of_input`) in both functions;
//! trailing **trivia** (comments, whitespace) is consumed by the lexer and so stays valid
//! (`{@html a /* c */}`).
//!
//! The type-annotation over-acceptances (`{@const x: T = a}`, `{:then x: T}` in a non-TS
//! component) are a *separate* concern — TS-only syntax tsv always accepts in templates,
//! not content loss — and are deliberately out of scope here.
//!
//! Verified against canonical Svelte via `tsv_debug canonical_parse`: every `INVALID`
//! case throws, every `VALID` case parses. Pinned as a Rust test rather than an
//! `input_invalid_*` fixture — a new fixture file reshuffles the corpus-sensitive
//! `fuzz:audit` seed-0 sample onto the next latent bug (see the fuzzer-backlog lore); a
//! parser-rejection assertion has no such coupling.

/// `true` if tsv's Svelte parser accepts `src`.
fn accepts(src: &str) -> bool {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_svelte::Interner::new();
    tsv_svelte::parse(src, &arena, &mut interner).is_ok()
}

/// The expression exactly fills its slice (or is followed only by trivia) — accepted,
/// matching Svelte.
#[test]
fn accepts_expression_that_fills_its_slice() {
    const VALID: &[&str] = &[
        "{@html a}",                   // html tag
        "{@html a /* c */}",           // trailing trivia is not a token
        "{@const x = a}",              // const init
        "{@debug a, b}",               // each arg is a lone identifier
        "{@render foo()}",             // render call
        "<p>{a}</p>",                  // bare mustache (element child)
        "<p>{a, b}</p>",               // sequence expression fills the slice
        "{#if a}x{/if}",               // if test
        "{#key a}x{/key}",             // key expression
        "{#each xs as x (a)}y{/each}", // each key
        "<a {@attach a}>x</a>",        // attach tag
        "<a {...a}>x</a>",             // spread attribute
        "{#snippet s(p)}x{/snippet}",  // snippet name
        // Pattern path (`parse_pattern_with_comments`):
        "{@const {a} = x}",               // destructuring binding id
        "{#each xs as {a}}b{/each}",      // each destructuring context
        "{#await p}a{:then x}b{/await}",  // then value pattern
        "{#await p}a{:catch e}b{/await}", // catch error pattern
    ];
    for src in VALID {
        assert!(
            accepts(src),
            "tsv should accept `{src}` (the expression fills its slice)"
        );
    }
}

/// A token trails the expression before the closing `}` — rejected, matching Svelte's
/// `eat('}', true)`. Without the end-of-slice check tsv silently dropped it.
#[test]
fn rejects_trailing_token_after_expression() {
    const INVALID: &[&str] = &[
        "{@html a b}",                   // html tag
        "{@render foo() bar}",           // render: trailing token past a valid call
        "{@const x = a b}",              // const init
        "{@debug a b}",                  // debug arg
        "<p>{a b}</p>",                  // bare mustache (element child)
        "{#if a b}x{/if}",               // if test
        "{#key a b}x{/key}",             // key expression
        "{#each xs as x (a b)}y{/each}", // each key
        "<a {@attach a b}>x</a>",        // attach tag
        "<a {...a b}>x</a>",             // spread attribute
        // Pattern path (`parse_pattern_with_comments`):
        "{@const x y = a}",                 // binding id
        "{@const {a} b = x}",               // destructuring binding id
        "{#await p}a{:then x y}b{/await}",  // then value pattern
        "{#await p}a{:catch e x}b{/await}", // catch error pattern
        "{#await p then x y}b{/await}",     // await-then shorthand pattern
    ];
    for src in INVALID {
        assert!(
            !accepts(src),
            "tsv should reject `{src}` (a token trails the expression)"
        );
    }
}
