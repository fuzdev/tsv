// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! The instantiation followers the fixture path cannot carry on the bare side, each
//! blocked by what a PARSER makes of that spelling rather than by prettier.
//!
//! `fn<T> << 1` is a tsv over-acceptance: tsc reads it as an instantiation expression,
//! acorn-typescript — tsv's parse oracle, and the parser Svelte itself uses — rejects it,
//! and tsv accepts and REPAIRS it, printing `(fn<T>) << 1`, the spelling all three read
//! alike. Accepting therefore costs nothing downstream: the pair the formatter adds settles
//! the reading for the parser that would not have taken the bare form. The bare spelling is
//! not a tsv fixed point, so it cannot be an `input.*`; and it cannot be a variant where the
//! other bare followers sit either — a TEMPLATE expression, the one island prettier reads
//! with acorn-typescript rather than with tsc's parser. Svelte's own parser rejects
//! `{fn<T> << 1}`, so prettier THROWS on the whole component
//! (`https://svelte.dev/e/js_parse_error`) where the variant rule needs it to LAND
//! somewhere, and the same rejection leaves `svelte compile` nothing to render, so the
//! render-equivalence arm has no oracle either.
//!
//! `fn<T> + 1` and `fn<T> - 1` are the CONTRAST that bounds the repair class, and they are
//! unfixturable for the opposite reason: both parsers accept them, as a relational chain
//! (`fn < T > +1`), so the instantiation rule has nothing to repair — the input was never an
//! instantiation. A variant must normalize to its `input.*`, and a third form is not its
//! input. What tsv prints is that chain with a pair around the `<` operand — the SEPARATE
//! relational-chain rule, which is the contrast's whole point: the pair lands around the
//! chain's left operand, never around the instantiation the tail rule would have wrapped.
//! That rule fires here because tsv's own parse, like acorn-typescript, reads `fn < T >⏎+1`
//! as `(fn<T>) + 1`, so a bare chain would re-lex once a break lands past the `>`. tsc would
//! not, so this pair is owed to tsv's reparse of its own output rather than to tsc (which
//! followers commit, for each parser, is stated in `docs/conformance_prettier_ts.md`
//! §Relational chain type-argument parens).
//!
//! The followers the two parsers BOTH reject are the class's far edge — no repair to make,
//! so tsv matching acorn is the drop-in contract rather than an over-rejection. They are
//! fixtures rather than assertions here, because `input_invalid_*` grades BOTH parsers'
//! rejection where a Rust test reaches only tsv's.
//!
//! The oracle-backed rest of the same rule lives in fixtures:
//! `typescript_specific/generics/instantiation_paren_follow_prettier_divergence` — the pair
//! as authored, where prettier strips it, plus the two bare followers acorn DOES accept
//! (`fn<T> < 1`, `fn<T> >= 1`), whose repair its `unformatted_ours_bare_follow` variant
//! pins in a template expression — and
//! `typescript_specific/generics/instantiation_operator_follow`, which pins the bare side
//! from both ends: the followers a bare instantiation KEEPS as its `input.svelte`, and the
//! ones neither parser admits (`f<T> > 1`, `f<T> >> 1`, `f<T> >>> 1`, `f<T>!`, and the
//! `typeof f<T> > 1` that ends an operand on the same close) as its `input_invalid_*`.
//!
//! The same join with a WORD as the follower — `as` / `satisfies`, and a Svelte block head's
//! own `as` / `then` — has the opposite parser split: tsc takes the list ahead of a binary
//! operator, while acorn-typescript gives it up ahead of any same-line token that can start an
//! expression. Where the bare spelling does not parse, the oracle-backed side is the fixtures
//! `typescript_specific/generics/instantiation_paren_assertion_follow_prettier_divergence` and
//! `svelte/blocks/head_instantiation_paren_prettier_divergence`. The spellings here are the
//! ones whose bare form DOES parse — as a comparison (`f<T> as [T]` is `f < T > as[T]`,
//! `{#await f<T> then}` a block with no `then` section) — so prettier's stripped output is
//! no tsv fixed point and cannot be an `output_prettier.*`; plus an inner pair around a
//! conditional's alternate in an `{#each}` head, whose prettier output loses a `)` and so
//! lands on no documented form; plus an `await` head, which `svelte compile` refuses without
//! `experimental.async`, leaving a variant's render check no oracle.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn format_svelte(source: &str) -> String {
    tsv_svelte::format_str(source).expect("format failed")
}

fn parses(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_ok()
}

#[test]
fn what_the_canonical_parser_rejects_gets_the_pair_back() {
    // The REPAIR. The bare spelling parses to the same tree as its repaired twin — one
    // `TSInstantiationExpression` under a `BinaryExpression` — so the added pair changes no
    // meaning; it settles the reading for the parser that would not have taken it.
    let bare = "const w = fn<T> << 1;\n";
    let repaired = "const w = (fn<T>) << 1;\n";
    assert_eq!(format(bare), repaired, "bare follower must gain the pair");
    assert!(
        parses(repaired),
        "the repaired form must reparse: {repaired}"
    );
}

#[test]
fn a_follower_that_re_lexes_is_not_repaired() {
    // A `+` or `-` continues a comparison chain, so the bare spelling is not a mis-spelled
    // instantiation at all — it is a different program, and tsv prints the program it
    // parsed. The pair it prints is the RELATIONAL-chain one, around the `<` operand, not
    // the instantiation pair this file's other case is about: tsv's own parse reads
    // `fn < T >⏎+1` as `fn<T> + 1`, so the chain would re-lex the moment a break lands past
    // the `>` (tsc would keep the chain there; see the module docs). The two pairs are in
    // different places, and which one appears is the assertion.
    for (bare, printed) in [
        ("const a = fn<T> + 1;\n", "const a = (fn < T) > +1;\n"),
        ("const b = fn<T> - 1;\n", "const b = (fn < T) > -1;\n"),
    ] {
        assert_eq!(
            format(bare),
            printed,
            "a re-lexed follower keeps its reading"
        );
        assert!(
            parses(&format(bare)),
            "the printed chain must reparse: {bare}"
        );
    }
}

#[test]
fn an_assertion_keeps_the_pair_its_bare_spelling_would_misread() {
    // Each pair is a fixed point, and each bare twin is a DIFFERENT program to tsv (and to
    // acorn-typescript): the word after the close continues a comparison, and the type
    // reads as an expression — an index, a subtraction, a tagged template.
    for (kept, bare, misread) in [
        (
            "x = (f<T>) as [T];\n",
            "x = f<T> as [T];\n",
            "x = (f < T) > as[T];\n",
        ),
        (
            "x = (f<T>) as -1;\n",
            "x = f<T> as -1;\n",
            "x = (f < T) > as - 1;\n",
        ),
        (
            "x = (f<T>) as `t`;\n",
            "x = f<T> as `t`;\n",
            "x = (f < T) > as`t`;\n",
        ),
    ] {
        assert_eq!(format(kept), kept, "the pair ahead of `as` must stay");
        assert_eq!(format(bare), misread, "the bare spelling is a comparison");
    }
}

#[test]
fn a_block_head_keeps_the_pair_its_keyword_would_join() {
    const HEAD: &str = "<script lang=\"ts\"></script>\n\n";
    // The value-less `then`: bare, the head is the comparison `f < T > then`, a block
    // with no `then` section at all.
    let kept = format!("{HEAD}{{#await (f<T>) then}}a{{/await}}\n");
    assert_eq!(
        format_svelte(&kept),
        kept,
        "the pair ahead of `then` must stay"
    );
    let bare = format!("{HEAD}{{#await f<T> then}}a{{/await}}\n");
    let misread = format!("{HEAD}{{#await (f < T) > then}}a{{/await}}\n");
    assert_eq!(
        format_svelte(&bare),
        misread,
        "the bare head is a comparison"
    );

    // An inner pair around a conditional's alternate moves out to the whole head, which
    // is what the head prints last.
    let inner = format!("{HEAD}{{#each c ? a : (f<T>) as item}}{{item}}{{/each}}\n");
    let outer = format!("{HEAD}{{#each (c ? a : f<T>) as item}}{{item}}{{/each}}\n");
    assert_eq!(
        format_svelte(&inner),
        outer,
        "the pair wraps the whole head"
    );
    assert_eq!(format_svelte(&outer), outer, "and holds");

    // An `await` head ends on its argument's close too. (Not a fixture cell: `svelte
    // compile` refuses a template `await` without `experimental.async`, so the render
    // check a variant needs has no oracle.)
    let awaited = format!("{HEAD}{{#each (await f<T>) as item}}{{item}}{{/each}}\n");
    assert_eq!(
        format_svelte(&awaited),
        awaited,
        "the pair ahead of `as` must stay"
    );
}
