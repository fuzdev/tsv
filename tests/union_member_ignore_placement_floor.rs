// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]
// the TS object-type literals (`{x:1}`) are source-under-test, not format placeholders
#![allow(clippy::literal_string_with_formatting_args)]

//! The format-ignore placement classification's TRAILING edges whose authored forms
//! normalize with a token move (a dropped `|`, a relocated line join), so the
//! `unformatted_*` fixture rules can't pin them — the normalized fixed point is asserted
//! here directly.
//!
//! The classification (docs/conformance_prettier.md §Format-ignore directive): own-line →
//! freeze the following member; glued directly before a value/member → freeze it whole;
//! anything else (content before the directive on its line, nothing glued after) →
//! trailing → inert.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// Head-trailing directive with an authored leading `|`: inert, so the value reformats —
/// the single-member-style `|` drops (a token move, excluded from `unformatted_*` rules)
/// and the interior formats. The result must be the head-trailing fixed point.
#[test]
fn head_trailing_authored_pipe_normalizes_inert() {
    let source = "type T = // prettier-ignore\n\t| {x:1} | b;\n";
    let expected = "type T = // prettier-ignore\n\t{ x: 1 } | b;\n";
    let pass1 = format(source);
    assert_eq!(pass1, expected, "head-trailing directive must be inert");
    assert_eq!(format(&pass1), pass1, "inert form must be a fixed point");
}

/// An end-of-line block directive between members (`a | /* d */⏎ member`) is trailing →
/// inert on pass 1: the member reformats and the union joins flat, which lands the
/// directive GLUED directly before the now-formatted member. The glued re-read on pass 2
/// freezes already-canonical content, so the flat form is a fixed point — the two
/// authorings (end-of-line vs glued) legitimately share it.
#[test]
fn trailing_block_end_of_line_normalizes_to_glued_fixed_point() {
    let source = "type T = a | /* prettier-ignore */\n\t{x:1} | b;\n";
    let expected = "type T = a | /* prettier-ignore */ { x: 1 } | b;\n";
    let pass1 = format(source);
    assert_eq!(
        pass1, expected,
        "end-of-line block directive must be inert (member reformats)"
    );
    assert_eq!(
        format(&pass1),
        pass1,
        "flat glued form must be a fixed point"
    );
}

/// A directive before the separator (`{a:1} /* d */ | b`) is trailing — the `|` breaks
/// the glue — so both members reformat and the placement is preserved as a fixed point.
#[test]
fn pre_separator_directive_inert() {
    let source = "type T = {a:1} /* prettier-ignore */ | {x:1};\n";
    let expected = "type T = { a: 1 } /* prettier-ignore */ | { x: 1 };\n";
    let pass1 = format(source);
    assert_eq!(pass1, expected, "pre-separator directive must be inert");
    assert_eq!(format(&pass1), pass1, "inert form must be a fixed point");
}
