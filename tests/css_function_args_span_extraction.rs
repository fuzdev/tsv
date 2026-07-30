// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A **wrapping, comment-bearing function value** takes its argument text from the
//! function's own span, not from a search for `name(` in the declaration text.
//!
//! The wrapped branch of `print_decl_function_with_comments` re-emits the arguments
//! from source (CSS value comments aren't in the AST, so there is nothing else to
//! print from). It used to locate them by searching the declaration for the
//! function's name and then taking the first `(` after it. A property routinely
//! contains the function's name — `--linear-gradient: linear-gradient(…)` — and the
//! search then measures from the *property's* occurrence; that still lands on the
//! right paren, but only because nothing between the two can be a `(`.
//!
//! A block comment in the property→colon gap can be (`--linear-gradient /* ( */:
//! linear-gradient(…)`, a shape tsv already formats — see fixture
//! `css/tokens/comments/in_property_value_before_colon_prettier_divergence`). Then
//! the offset is wrong, the paren-depth scan never balances, extraction returns
//! `None`, and the whole value falls to the semantic fallback — which prints from
//! the AST and so drops the **closing comma** (and would drop an argument comment
//! that wasn't glued into an argument's span).
//!
//! Slicing the arguments out of the function's span instead is exact:
//! `extract_function_parts` accepts a value as a function only when the matching
//! close paren is its last byte, and a function name cannot contain a `(`.
//!
//! Not a fixture: the trigger needs a >100-column value *and* a property-position
//! comment, which is a second, separately-sanctioned divergence — pinning it in the
//! fixture corpus would put two unrelated divergences in one directory (and add a
//! directory, reshuffling the `fuzz:audit` seed-0 sample). The closing-comma rule
//! itself is pinned by `css/values/lists/comma_closing_prettier_divergence` and its
//! `_long` sibling.

/// `(label, input, expected)`. Each `input` formats to `expected` in one pass, and
/// `expected` is a fixed point. The value is over the print width, so the
/// comment-bearing function takes its **wrapped** branch.
const CASES: &[(&str, &str, &str)] = &[
    // The property carries the function's name AND a comment holding a `(` — the
    // shape that defeated the name search. The closing comma must survive.
    (
        "property contains the function name and a `(`-bearing comment",
        "a{--linear-gradient /* ( */: linear-gradient(/* k */ #000 0%, #111 2%, #222 30%, #333 50%, #444 90%, #555 95%, #5555 100%,)}",
        "a {\n\t--linear-gradient /* ( */ : linear-gradient(\n\t\t/* k */ #000 0%,\n\t\t#111 2%,\n\t\t#222 30%,\n\t\t#333 50%,\n\t\t#444 90%,\n\t\t#555 95%,\n\t\t#5555 100%,\n\t);\n}\n",
    ),
    // The same value under a property that does NOT contain the name — the control
    // the search got right, which must be unchanged.
    (
        "ordinary property (control)",
        "a{background: linear-gradient(/* k */ #000 0%, #111 2%, #222 30%, #333 50%, #444 90%, #555 95%, #5555 100%,)}",
        "a {\n\tbackground: linear-gradient(\n\t\t/* k */ #000 0%,\n\t\t#111 2%,\n\t\t#222 30%,\n\t\t#333 50%,\n\t\t#444 90%,\n\t\t#555 95%,\n\t\t#5555 100%,\n\t);\n}\n",
    ),
];

fn format_css(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let stylesheet = tsv_css::parse(source, &arena).expect("parse failed");
    tsv_css::format(&stylesheet, source)
}

#[test]
fn wrapped_function_args_come_from_the_function_span() {
    for &(label, input, expected) in CASES {
        let out = format_css(input);
        assert_eq!(
            out, expected,
            "case `{label}`: input should format to the expected form in one pass"
        );

        let out_twice = format_css(expected);
        assert_eq!(
            out_twice, expected,
            "case `{label}`: expected form must be a fixed point (idempotent)"
        );
    }
}
