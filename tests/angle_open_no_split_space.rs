// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! The GLUED authoring of the `<` `<` pair, at the positions where tsc never splits a `<<`
//! token — the one authoring the fixture path cannot carry in a `<script>` body.
//!
//! `<<T>() => R>x`, `typeof f<<T>() => void>` and `class extends fn<<T>() => U> {}` parse
//! under acorn-typescript — tsv's parse oracle, which splits the token wherever type
//! arguments are legal — and tsv REPAIRS each to the spaced form every parser reads alike.
//! The glued spelling is not a tsv fixed point, so it cannot be an `input.*`; and it cannot
//! be a `<script>` variant, because prettier reads that body with tsc's parser and THROWS
//! where the variant rule needs it to LAND somewhere. A TEMPLATE expression is
//! acorn-typescript's, so the fixtures' `unformatted_ours_glued` variants carry one glued
//! cell each there; these assertions carry the script-body spellings, the declarations a
//! template cannot hold among them.
//!
//! At `implements` and an interface's `extends` acorn-typescript rejects the glued form as
//! tsc does, so tsv rejects it too — graded against both parsers by the heritage fixture's
//! `input_invalid_*` files, and restated here beside the spellings that must keep parsing.
//!
//! The oracle-backed rest of the rule lives in
//! `typescript/expressions/binary/shift_left_{type_assertion,typeof_query,heritage,no_split_long}_prettier_divergence`,
//! and the positions where tsc DOES split — which stay glued — in
//! `typescript/expressions/binary/shift_left_vs_type_args`.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

fn parses(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    tsv_ts::parse(source, &arena).is_ok()
}

#[test]
fn the_glued_authoring_is_repaired_where_tsc_never_splits() {
    for (glued, spaced) in [
        ("const a = <<T>() => R>x;\n", "const a = < <T>() => R>x;\n"),
        (
            "type Y = typeof f<<T>() => void>;\n",
            "type Y = typeof f< <T>() => void>;\n",
        ),
        (
            "class A extends fn<<T>() => U> {}\n",
            "class A extends fn< <T>() => U> {}\n",
        ),
        (
            "const b = class extends a.b<<T>() => U, V> {};\n",
            "const b = class extends a.b< <T>() => U, V> {};\n",
        ),
    ] {
        assert_eq!(format(glued), spaced, "{glued:?}");
        assert_eq!(format(spaced), spaced, "{spaced:?} is a fixed point");
    }
}

#[test]
fn the_glued_authoring_stays_glued_where_tsc_splits() {
    for source in [
        "fn<<T>() => U>(x);\n",
        "new Fn<<T>() => U>();\n",
        "fn?.<<T>() => U>(x);\n",
        "const g = fn<<T>() => U>;\n",
        "fn<<T>() => U>`t`;\n",
        "type X = A<<T>() => U>;\n",
        "type Z = import('x').A<<T>() => U>;\n",
        "class A extends fn<<T>() => U>() {}\n",
        "class B implements I<A<<T>() => U>> {}\n",
    ] {
        assert_eq!(format(source), source, "{source:?}");
    }
}

#[test]
fn the_glued_authoring_is_rejected_where_both_parsers_reject_it() {
    for source in [
        "class B implements I<<T>() => U> {}",
        "class B implements J, I<<T>() => U> {}",
        "interface B extends I<<T>() => U> {}",
        "interface B extends J, I<<T>() => U> {}",
    ] {
        assert!(!parses(source), "{source:?} should be rejected");
    }
    for source in [
        "class B implements I< <T>() => U> {}",
        "interface B extends I< <T>() => U> {}",
        "class B implements I<A<<T>() => U>> {}",
        "interface B extends I<A<<T>() => U>> {}",
        "class B extends fn<<T>() => U> {}",
    ] {
        assert!(parses(source), "{source:?} should parse");
    }
}
