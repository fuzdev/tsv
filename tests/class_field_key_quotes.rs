// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A quoted class field key keeps its quotes on every path that prints a class member,
//! and a key that is not a string prints as it does anywhere else.
//!
//! The fixtures pin the rule inside a Svelte `<script>`
//! ([field_key_quoted](../tests/fixtures/typescript/declarations/class/field_key_quoted/)
//! and its siblings) and in a template expression of a TypeScript component
//! ([class_field_key_quoted](../tests/fixtures/svelte/expressions/class_field_key_quoted_prettier_divergence/)).
//! One printer also formats a standalone file and the template of a component with no
//! TypeScript `<script>`, and those entry points have their own printer state — a rule
//! keyed on it would hold every fixture and still unquote there. So this file holds the
//! same claim on those paths, plus the two neighbours of the rule a field key shares
//! with every other key: a numeric key's normalization, and the comment landings around
//! a kept key, which are those of a quoted key that was never an identifier.

/// Format a standalone TypeScript / JavaScript source, asserting the output is a fixed
/// point.
fn format_ts(source: &str) -> String {
    let once = tsv_ts::format_str(source).expect("format failed");
    let twice = tsv_ts::format_str(&once).expect("the output failed to format");
    assert_eq!(once, twice, "not a fixed point: {source:?}");
    once
}

/// Format a Svelte document, asserting the output is a fixed point.
fn format_svelte(source: &str) -> String {
    let once = tsv_svelte::format_str(source).expect("format failed");
    let twice = tsv_svelte::format_str(&once).expect("the output failed to format");
    assert_eq!(once, twice, "not a fixed point: {source:?}");
    once
}

/// A standalone file: every field form keeps its quotes (the quote style normalizes),
/// and the method, accessor and constructor keys beside them unquote.
#[test]
fn standalone_file_keeps_quoted_field_keys() {
    let source = "class A {\n\t'a': string;\n\t\"b\" = 1;\n\tstatic 'c';\n\taccessor 'd' = 2;\n\tdeclare 'e': number;\n\t'constructor'() {}\n\t'm'() {}\n\tget 'g'() {\n\t\treturn 1;\n\t}\n}\n";
    let expected = "class A {\n\t'a': string;\n\t'b' = 1;\n\tstatic 'c';\n\taccessor 'd' = 2;\n\tdeclare 'e': number;\n\tconstructor() {}\n\tm() {}\n\tget g() {\n\t\treturn 1;\n\t}\n}\n";
    assert_eq!(format_ts(source), expected);
    // the quoted spelling is its own fixed point
    assert_eq!(format_ts(expected), expected);
}

/// A class expression in a Svelte template, with and without a TypeScript `<script>`
/// beside it.
#[test]
fn template_class_expression_keeps_quoted_field_keys() {
    let template = "<p>{new (class {\n\t'a' = 1;\n\t\"b\";\n\t'm'() {}\n})().a}</p>\n";
    let expected = "<p>\n\t{new (class {\n\t\t'a' = 1;\n\t\t'b';\n\t\tm() {}\n\t})().a}\n</p>\n";
    assert_eq!(format_svelte(template), expected);

    let script = "<script lang=\"ts\">\n\tlet x = 1;\n</script>\n\n";
    assert_eq!(
        format_svelte(&format!("{script}{template}")),
        format!("{script}{expected}")
    );
}

/// A method key is not a field key under any modifier, a decorator or type parameters, in
/// a class expression or in an ambient class: it unquotes, and a quoted field beside it
/// does not.
#[test]
fn method_keys_unquote_under_every_modifier_and_host() {
    let source = "declare const dec: any;\nclass Base {\n\to() {}\n}\nclass A extends Base {\n\t'a' = 1;\n\tpublic 'm'() {}\n\tprivate 'n'() {}\n\tprotected static 'p'() {}\n\toverride 'o'() {}\n\t@dec 'q'() {}\n\t'r'<T>(x: T): T {\n\t\treturn x;\n\t}\n}\nconst C = class {\n\t'a' = 1;\n\t'm'() {}\n};\ndeclare class D {\n\t'a': string;\n\t'm'(): void;\n}\n";
    let expected = "declare const dec: any;\nclass Base {\n\to() {}\n}\nclass A extends Base {\n\t'a' = 1;\n\tpublic m() {}\n\tprivate n() {}\n\tprotected static p() {}\n\toverride o() {}\n\t@dec q() {}\n\tr<T>(x: T): T {\n\t\treturn x;\n\t}\n}\nconst C = class {\n\t'a' = 1;\n\tm() {}\n};\ndeclare class D {\n\t'a': string;\n\tm(): void;\n}\n";
    assert_eq!(format_ts(source), expected);
}

/// A numeric field key is a number literal like any other: it normalizes, and is never
/// quoted.
#[test]
fn numeric_field_key_normalizes() {
    let source =
        "class A {\n\t1.50 = 1;\n\t0XAB = 2;\n\t.5 = 3;\n\t5. = 4;\n\t1E3 = 5;\n\t1_000 = 6;\n}\n";
    let expected =
        "class A {\n\t1.5 = 1;\n\t0xab = 2;\n\t0.5 = 3;\n\t5 = 4;\n\t1e3 = 5;\n\t1_000 = 6;\n}\n";
    assert_eq!(format_ts(source), expected);
}

/// A kept identifier-valid key (`'abc'`) takes every comment landing a quoted key that is
/// no identifier (`'a-c'`, the same width) takes: the two outputs differ by the key alone.
#[test]
fn kept_key_shares_the_comment_landings_of_a_non_identifier_key() {
    // `K` stands for the key
    let members = [
        "/* c */ K: string;",
        "/* c */K: string;",
        "K /* c */: string;",
        "K // c\n\t: string;",
        "K /* c */ = 1;",
        "K // c\n\t= 1;",
        "K /* c */;",
        "K /* c */?: string;",
        "K /* c */!: string;",
        "static /* c */ K = 1;",
        "static // c\n\tK = 1;",
        "@dec /* c */ K: string;",
        "@dec\n\t/* c */\n\tK: string;",
        "accessor /* c */ K /* d */ = /* e */ 1; // f",
        "/** @type {string} */ K;",
        "/**\n\t * doc\n\t */\n\tK: string;",
    ];
    for member in members {
        let with =
            |key: &str| format_ts(&format!("class A {{\n\t{}\n}}\n", member.replace('K', key)));
        let kept = with("'abc'");
        assert_eq!(
            kept.matches("'abc'").count(),
            1,
            "the key keeps its quotes: {member:?} -> {kept:?}"
        );
        assert_eq!(
            kept.replace("'abc'", "'a-c'"),
            with("'a-c'"),
            "comment landings differ from the non-identifier key's: {member:?}"
        );
        // a double-quoted authoring reaches the same output
        assert_eq!(with("\"abc\""), kept, "quote style: {member:?}");
    }
}
