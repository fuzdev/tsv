// `format_holds` isn't a `#[test]`, so clippy.toml's allow-panic-in-tests doesn't reach it
#![allow(clippy::panic)]

//! The identifier name channel's `plain_ascii` flag, graded on inputs no corpus holds.
//!
//! A name that is plain one-column ASCII has a visual width equal to its byte length —
//! `IdentifierPart`'s ASCII subset is `[A-Za-z0-9_$]`, so no tab and no line terminator
//! can be inside one — and the printer spends that: `DocArena::source_span_plain`
//! allocates the doc text with no width scan at all. The flag that licenses the skip is
//! derived from the lexer, which walked those bytes anyway: it records the START offsets
//! of the last two identifier tokens that held a non-ASCII char (and nothing on any
//! other token), the parser asks it about `current` by offset, and the answer travels
//! `IdentName` → `Identifier` to the printer.
//!
//! ⚠️ **Neither direction of a wrong flag is visible in the output.** Over-claiming
//! measures a non-ASCII name as one column a byte, which moves a fits verdict and
//! nothing else — no format diff, no wire diff, no fixture. Under-claiming is
//! byte-identical and merely slow, which is how a plumbing site that stops being
//! reached would go unnoticed. So the name seam (`Printer::ident_name_doc`) asserts the
//! flag against the name's own bytes in debug builds, both ways, and this file is what
//! drives the assert over the shapes real code does not contain: non-ASCII identifiers
//! at every length and alignment, escaped names, names read through a lookahead, and
//! two non-ASCII names lexed back to back — the case the lexer's second slot exists for.
//!
//! The corpus cannot stand in for them. A formatted TypeScript corpus is essentially
//! all-ASCII in *name* position (three names in 483,358 on a 1,666-file corpus hold a
//! non-ASCII byte), and a fixture tree of format fixed points is no denser.

/// Every name position that reaches the printer's name seam, as a source template with
/// `NAME` standing in for the identifier. Several are separate arms of the seam's own
/// callers (a member property, a private name, a type parameter, an import default and
/// a test-call callee each arrive by their own route), and several force a LOOKAHEAD
/// between the name's lex and its use — the single-parameter arrow and the labelled
/// statement — so the lexer has already moved one token past the name when it is asked.
const POSITIONS: &[&str] = &[
    "NAME;\n",
    "const NAME = 1;\n",
    "let NAME: string;\n",
    "x.NAME;\n",
    "x?.NAME;\n",
    "({ NAME });\n",
    "({ NAME: 1 });\n",
    "class C {\n\t#NAME = 1;\n\tNAME() {}\n}\n",
    "type T<NAME> = NAME;\n",
    "interface I {\n\tNAME: number;\n}\n",
    "import NAME from 'm';\n",
    "import { NAME } from 'm';\n",
    "export { NAME };\n",
    "function f(NAME) {\n\treturn NAME;\n}\n",
    "const f = (NAME) => NAME;\n",
    "NAME: for (;;) break NAME;\n",
    "enum E {\n\tNAME = 1\n}\n",
    "namespace NAME {\n\texport const a = 1;\n}\n",
    "declare function g(): asserts NAME is string;\n",
    "it('a', () => {\n\tNAME;\n});\n",
];

/// Non-ASCII `ID_Continue` characters, one per UTF-8 encoded length, plus one whose
/// column count is not one (`中` is wide) and one outside the BMP.
const NON_ASCII: &[&str] = &["é", "ϕ", "中", "𝕏"];

/// Format `source`, then format the result again. The seam's own debug assertion is the
/// grader — this only has to reach it, and to prove the name survived the print.
#[track_caller]
fn format_holds(source: &str, name: &str) {
    let out =
        tsv_ts::format_str(source).unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
    assert!(
        out.contains(name),
        "the name {name:?} did not survive the print of {source:?}: {out:?}"
    );
    let again =
        tsv_ts::format_str(&out).unwrap_or_else(|e| panic!("reparse failed for {out:?}: {e}"));
    assert_eq!(again, out, "not a fixed point: {source:?}");
}

/// Drive every position with `name`.
#[track_caller]
fn all_positions(name: &str) {
    for template in POSITIONS {
        format_holds(&template.replace("NAME", name), name);
    }
}

/// A non-ASCII byte at **every position of every length**, so the flag is asked about a
/// name whose non-ASCII byte lands in every lane of the scan the flag replaces — and, in
/// the ASCII-prefix cases, about one the lexer's own fast path walks before bailing.
#[test]
fn a_non_ascii_name_never_claims_plain_ascii() {
    for glyph in NON_ASCII {
        for len in 0..=12usize {
            for at in 0..=len {
                let name = format!("{}{glyph}{}", "a".repeat(at), "b".repeat(len - at));
                all_positions(&name);
            }
        }
    }
}

/// The completeness half, and the one that goes quietly wrong: a plain ASCII name must
/// CLAIM the flag, at every length across the eight-byte word boundary the scan it
/// replaces is built around. A plumbing site that stopped being reached would still
/// format byte-identically; only this fails.
#[test]
fn a_plain_ascii_name_always_claims_it() {
    for len in 1..=40usize {
        all_positions(&format!("a{}", "b".repeat(len - 1)));
    }
    for name in ["_", "$", "_$a0", "$$props", "aB0_$", "\u{61}bc"] {
        all_positions(name);
    }
}

/// Two non-ASCII names lexed back to back, with the parser reading the FIRST after the
/// lexer has already produced the second as its lookahead. The lexer's record holds two
/// starts for exactly this: the second name takes the newer slot and the first must
/// survive in the older one. Every shape in which the grammar puts two names in
/// adjacent tokens, and the third name behind them proves the record is not deeper
/// than it needs to be — by then the first has been consumed.
#[test]
fn two_adjacent_non_ascii_names_both_hold() {
    for (a, b, c) in [("é", "ñ", "ö"), ("中", "ϕ", "𝕏"), ("aé", "bñ", "cö")] {
        for template in [
            "A
B
C;
",
            "A
(B);
C;
",
            "A
[B];
C;
",
            "let A
let B
let C
",
            "x = A
B
++C;
",
            "A as B as C;
",
            "type T = A extends B ? C : never;
",
            "async A => B;
C;
",
            "class A extends B {
	C() {}
}
",
            "label: A
B
C;
",
            "function f(A, B, C) {}
",
            "const { A, B, C } = x;
",
            "import { A as B, C } from 'm';
",
            "enum E {
	A,
	B,
	C
}
",
            "A?.B?.C;
",
            "A<B>(C);
",
            "type U = A<B, C>;
",
            "for (A of B) C;
",
            "if (A) B;
else C;
",
            "declare module A {
	let B: C;
}
",
        ] {
            let source = template.replace('A', a).replace('B', b).replace('C', c);
            for name in [a, b, c] {
                format_holds(&source, name);
            }
        }
    }
}

/// A `\u` escape puts the name in the arena instead of the source, so it takes the
/// pooled-text arm and must never claim the flag — including when the escape decodes to
/// an ASCII character, where the decoded name and the raw span differ in LENGTH.
#[test]
fn an_escaped_name_never_claims_plain_ascii() {
    for source in [
        "\\u0061bc;\n",
        "a\\u0062c;\n",
        "ab\\u0063;\n",
        "x.\\u0061bc;\n",
        "const \\u0061 = 1;\n",
        "({ \\u0061: 1 });\n",
        "class C {\n\t#\\u0061 = 1;\n}\n",
        "\\u00e9;\n",
        "a\\u00e9b;\n",
        "\\u{1D54F};\n",
    ] {
        let out = tsv_ts::format_str(source)
            .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
        let again =
            tsv_ts::format_str(&out).unwrap_or_else(|e| panic!("reparse failed for {out:?}: {e}"));
        assert_eq!(again, out, "not a fixed point: {source:?}");
    }
}

/// A keyword read as a name takes the raw channel rather than the decoded one, and the
/// two meta-properties take `IdentName::from_span`, whose `plain_ascii` is a contract on
/// the caller rather than a measurement. Both must satisfy the seam's assertion.
#[test]
fn keyword_names_and_meta_properties_hold_the_contract() {
    for source in [
        "x.class;\n",
        "x.default;\n",
        "({ class: 1 });\n",
        "class C {\n\t#constructor2 = 1;\n\tstatic;\n}\n",
        "type T = { readonly a: 1 };\n",
        "const a = new.target;\n",
        "const b = import.meta;\n",
        "function f(this: T) {}\n",
        "type P = { [K in keyof T as `x${string}`]: 1 };\n",
        "enum E {\n\tclass = 1\n}\n",
    ] {
        let out = tsv_ts::format_str(source)
            .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
        let again =
            tsv_ts::format_str(&out).unwrap_or_else(|e| panic!("reparse failed for {out:?}: {e}"));
        assert_eq!(again, out, "not a fixed point: {source:?}");
    }
}

/// The Svelte parser SYNTHESIZES a name channel of its own for the three shorthands —
/// `{name}`, `bind:name`, `class:name` — where the identifier is never lexed as one and
/// the flag is computed from the slice instead of arriving from the lexer. Every such
/// name in the fixture tree is ASCII, so nothing there separates that computation from
/// a bare `true`; these do.
#[test]
fn svelte_synthesized_shorthand_names_hold_the_contract() {
    for glyph in ["a", "é", "ϕ", "中", "𝕏"] {
        for len in [0usize, 1, 7, 8, 9, 16] {
            let name = format!("{glyph}{}", "b".repeat(len));
            for template in [
                "<div {NAME}></div>\n",
                "<input bind:NAME />\n",
                "<div class:NAME></div>\n",
            ] {
                let source = template.replace("NAME", &name);
                let out = tsv_svelte::format_str(&source)
                    .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
                assert!(
                    out.contains(&name),
                    "the name {name:?} did not survive the print of {source:?}: {out:?}"
                );
                let again = tsv_svelte::format_str(&out)
                    .unwrap_or_else(|e| panic!("reparse failed for {out:?}: {e}"));
                assert_eq!(again, out, "not a fixed point: {source:?}");
            }
        }
    }
}

/// The same names through the Svelte pipeline, where the TypeScript printer is entered
/// per island and the spans are document-absolute.
#[test]
fn svelte_islands_hold_the_contract() {
    for glyph in ["a", "é", "中", "𝕏"] {
        for len in [0usize, 1, 7, 8, 9, 15, 16, 17] {
            let name = format!("{glyph}{}", "b".repeat(len));
            for template in [
                "<script lang=\"ts\">\n\tlet NAME = 1;\n</script>\n\n<p>{NAME}</p>\n",
                "<script lang=\"ts\">\n\tlet NAME = $state(1);\n</script>\n\n<input bind:value={NAME} />\n",
                "<script lang=\"ts\">\n\tconst NAME = 1;\n</script>\n\n{#if NAME}\n\t<p>{NAME}</p>\n{/if}\n",
            ] {
                let source = template.replace("NAME", &name);
                let out = tsv_svelte::format_str(&source)
                    .unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
                let again = tsv_svelte::format_str(&out)
                    .unwrap_or_else(|e| panic!("reparse failed for {out:?}: {e}"));
                assert_eq!(again, out, "not a fixed point: {source:?}");
            }
        }
    }
}
