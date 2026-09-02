// `format_is_fixed_point` isn't a `#[test]`, so clippy.toml's allow-panic-in-tests doesn't reach it
#![allow(clippy::panic)]

//! The verbatim-literal width claim, graded on inputs no corpus holds.
//!
//! A string or number literal that prints as its own source slice goes to
//! `DocArena::source_span_plain` — no width scan at all — whenever that slice holds no
//! byte the width depends on (`\t`, `\n`, anything at or above `0x80`). Neither caller
//! pays a scan to know it: a number is plain ASCII by grammar, and a string's quote
//! choice already reads every content byte looking for a `'`, so
//! `printing::optimal_string_quote_in` returns what that one pass saw.
//!
//! ⚠️ **Neither direction of a wrong claim is visible in the output.** Over-claiming
//! measures a tabbed or non-ASCII literal as one column a byte, which moves a fits
//! verdict and changes nothing else — no format diff, no wire diff, no fixture.
//! Under-claiming is byte-identical and merely slow, which is how a seam that stopped
//! being reached would go unnoticed. So `Printer::verbatim_literal_doc` asserts the
//! claim against the span's own bytes in debug builds, both ways, and this file drives
//! that assert over the shapes a formatted corpus does not contain: only 1,212 of the
//! 84,771 verbatim string literals on a 1,666-file TypeScript corpus are non-plain, and
//! a fixture tree of format fixed points is no denser.
//!
//! The `'` half is graded here too, in the same sweep: it shares the scan, and a class
//! that lost that needle would silently re-quote a string that must not be re-quoted —
//! the one direction of this change that *is* visible, and cheapest to grade beside the
//! direction that is not.

/// Every position a string literal reaches the printer's literal seam from. Several are
/// separate arms of its callers — a quoted object key, an import attribute value, a
/// string literal *type*, an `enum` member value and a `case` label each arrive by their
/// own route — and the call argument additionally reaches `is_short_arg`, the second
/// caller of the verbatim question, which asks for the printed LENGTH rather than a doc.
const POSITIONS: &[&str] = &[
    "const a = STR;\n",
    "({ STR: 1 });\n",
    "x[STR];\n",
    "f(STR);\n",
    "f(STR, aLongEnoughSecondArgument, () => {\n\tg();\n});\n",
    "type T = STR;\n",
    "enum E {\n\tA = STR\n}\n",
    "switch (x) {\n\tcase STR:\n\t\tbreak;\n}\n",
    "import a from 'm' with { type: STR };\n",
    "const o = {\n\ta: STR,\n\tb: STR\n};\n",
    "export const long_name_here = [STR, STR, STR];\n",
];

/// Non-ASCII characters, one per UTF-8 encoded length, plus one whose column count is
/// not one (`中` is wide) and one outside the BMP.
const NON_ASCII: &[&str] = &["é", "ϕ", "中", "𝕏"];

/// Format `source`, then format the result again, and require a fixed point. The seam's
/// own debug assertion is the grader — this only has to REACH it, once per position, and
/// prove the document still parses after the print.
#[track_caller]
fn format_is_fixed_point(source: &str) -> String {
    let out =
        tsv_ts::format_str(source).unwrap_or_else(|e| panic!("parse failed for {source:?}: {e}"));
    let again =
        tsv_ts::format_str(&out).unwrap_or_else(|e| panic!("reparse failed for {out:?}: {e}"));
    assert_eq!(again, out, "not a fixed point: {source:?}");
    out
}

/// Drive every position with `literal`. The positions are here to reach the seam from
/// each of its callers' arms, NOT to preserve the literal's spelling — a quoted object
/// key whose content is a valid identifier is deliberately unquoted (prettier's
/// `quoteProps: "as-needed"`), so only `survives_verbatim` makes the stronger claim.
#[track_caller]
fn all_positions(literal: &str) {
    for template in POSITIONS {
        format_is_fixed_point(&template.replace("STR", literal));
    }
}

/// The value position, where the printed text IS the source literal — so this is where
/// the *spelling* claim belongs: the literal must come back byte for byte.
#[track_caller]
fn survives_verbatim(literal: &str) {
    let out = format_is_fixed_point(&format!("const a = {literal};\n"));
    assert!(
        out.contains(literal),
        "{literal:?} did not survive the print: {out:?}"
    );
}

/// A single-quoted literal wrapping `content`.
fn single(content: &str) -> String {
    format!("'{content}'")
}

/// The completeness half, and the one that goes quietly wrong: a plain ASCII literal
/// must CLAIM the flag, at every length across the eight-byte word boundary the scan is
/// built around. A seam that stopped claiming would still format byte-identically; only
/// the assert's second direction fails.
#[test]
fn a_plain_ascii_string_always_claims_plain() {
    for len in 0..=40usize {
        let literal = single(&"a".repeat(len));
        all_positions(&literal);
        survives_verbatim(&literal);
    }
    // Escape sequences are plain ASCII in SOURCE — `\t` here is two bytes, a backslash
    // and a `t`, and the width question is asked of the bytes, never of the decoded
    // value.
    for content in ["", " ", "a b", r"\n", r"\t", r"A", r"\\", r#"a"b"#, r#""""#] {
        let literal = single(content);
        all_positions(&literal);
        survives_verbatim(&literal);
    }
}

/// A byte the width depends on, at **every position of every length**, so the claim is
/// asked about a span whose offending byte lands in every lane of the word the scan
/// reads — including the lanes past the content, which the scan reads and must not
/// believe.
#[test]
fn a_width_relevant_string_never_claims_plain() {
    let mut needles: Vec<String> = NON_ASCII.iter().map(|s| (*s).to_string()).collect();
    // A real tab inside the literal, and a line CONTINUATION — a backslash followed by a
    // raw line terminator, which is the only way a `\n` gets inside a string's span.
    needles.push("\t".to_string());
    needles.push("\\\n".to_string());
    for needle in &needles {
        for len in 0..=12usize {
            for at in 0..=len {
                let content = format!("{}{needle}{}", "a".repeat(at), "b".repeat(len - at));
                let literal = single(&content);
                all_positions(&literal);
                survives_verbatim(&literal);
            }
        }
    }
}

/// The quote question shares the scan, so it is graded in the same sweep. A content
/// holding a `'` takes the counting arm; the quote that survives is the one that needs
/// fewer escapes, and a scan that lost the `'` needle would re-quote every one of these.
#[test]
fn a_content_holding_a_quote_keeps_the_quote_that_escapes_less() {
    // A `'` inside, so double quotes win and must be preserved.
    for content in ["it's", "'", "''", "a'b'c", "'a", "a'"] {
        for len in 0..=10usize {
            let padded = format!("{}{content}{}", "x".repeat(len), "y".repeat(len));
            let literal = format!("\"{padded}\"");
            all_positions(&literal);
            survives_verbatim(&literal);
        }
    }
    // Both kinds present and `'` not rarer: single quotes win, so the source already has
    // the optimal quote and the escaped `\'` stays escaped.
    survives_verbatim(r#"'a"b\'c'"#);
    // Doubles only: single quotes win and the content is untouched.
    for content in [r#"a\"b"#, r#"\""#, r#"say \"hi\""#] {
        let literal = single(content);
        all_positions(&literal);
        survives_verbatim(&literal);
    }
    // ⭐ The RE-QUOTE arm — the one the corpus cannot reach at all: on a 1,666-file
    // TypeScript corpus not one of the 84,771 verbatim literals swaps its quote, so
    // `rebuild_string_literal` is reached only from here and from the fixtures. The
    // spelling of the swap belongs to the prettier-derived fixtures; what this asks is
    // that the arm is entered and lands on a fixed point.
    for literal in [r"'\''", r"'it\'s'", r"'a\'b\'c'"] {
        all_positions(literal);
        let out = format_is_fixed_point(&format!("const a = {literal};\n"));
        assert!(
            !out.contains(literal),
            "{literal:?} should have been re-quoted: {out:?}"
        );
    }
}

/// A literal at the very END of the document, where the host has fewer than eight bytes
/// past the content and the scan's scalar tail runs — the one arm the word loop never
/// reaches, and the arm whose class is spelled separately.
#[test]
fn a_literal_at_the_document_end_still_claims_correctly() {
    for len in 0..=20usize {
        let a = "a".repeat(len);
        format_is_fixed_point(&format!("x = '{a}'"));
        format_is_fixed_point(&format!("x = '{a}\t'"));
        format_is_fixed_point(&format!("x = '{a}中'"));
        format_is_fixed_point(&format!("x = \"{a}'\""));
    }
}

/// Every numeric spelling the grammar admits, in every position it is legal in — the
/// claim there is unconditional (`NumericLiteral` is ASCII by grammar), so this drives
/// the assert over the forms that normalize and the forms that do not.
#[test]
fn every_number_spelling_claims_plain() {
    const NUMBERS: &[&str] = &[
        "0",
        "1",
        "42",
        "1234567890123",
        "0x1f",
        "0X1F",
        "0xdeadBEEF",
        "0b1010",
        "0B1010",
        "0o17",
        "0O17",
        "1e10",
        "1E10",
        "1e+10",
        "1e-10",
        "1.5",
        "1.50",
        ".5",
        "5.",
        "0.0",
        "1_000_000",
        "0x1_f",
        "123n",
        "0xFFn",
        "1_2n",
    ];
    for n in NUMBERS {
        for template in POSITIONS {
            let source = template.replace("STR", n);
            // A number is not legal as a quoted key or an import-attribute value; the
            // parse failure filters those positions out.
            if tsv_ts::format_str(&source).is_ok() {
                format_is_fixed_point(&source);
            }
        }
        format_is_fixed_point(&format!("const a = {n};\n"));
    }
}
