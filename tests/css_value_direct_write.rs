//! A declaration value whose doc is a single source-span text is appended to the output
//! as its own slice rather than rendered (`Printer::print_css_value`). The slice is taken
//! whole whatever bytes it holds, so each input here is a value that takes that path while
//! holding a byte a narrower reading of it could mishandle — a line break, a tab, a
//! non-ASCII character, a control byte, a trailing space that is an escape's payload —
//! and each must come back from `format_str` exactly as written.
//!
//! The values are assembled from named characters, never spelled as escapes inside one
//! literal, so what the formatter is handed is unambiguous in this file.

const BACKSLASH: char = '\\';
const TAB: char = '\t';
const LINE_FEED: char = '\n';

/// `a { color: <value>; }` in the formatter's own layout.
fn declaration(value: &str) -> String {
    format!("a {{{LINE_FEED}{TAB}color: {value};{LINE_FEED}}}{LINE_FEED}")
}

/// `(label, value)` — each value prints as one verbatim source-span leaf.
fn cases() -> Vec<(&'static str, String)> {
    let vertical_tab = char::from(0x0b_u8);
    vec![
        (
            "a dimension whose unit holds a hex escape terminated by a line feed",
            format!("1px{BACKSLASH}41{LINE_FEED}b"),
        ),
        (
            "a dimension whose unit holds a hex escape terminated by a tab",
            format!("1px{BACKSLASH}9{TAB}b"),
        ),
        (
            "a dimension whose unit holds an escaped tab",
            format!("1p{BACKSLASH}{TAB}x"),
        ),
        (
            "a dimension whose unit holds a non-ASCII character",
            String::from("10p\u{e9}"),
        ),
        (
            "an identifier holding a vertical tab",
            format!("a{vertical_tab}b"),
        ),
        (
            "a dimension ending in an escaped space",
            format!("50px{BACKSLASH} "),
        ),
    ]
}

#[test]
fn a_verbatim_leaf_value_is_its_own_fixed_point() {
    for (label, value) in cases() {
        let source = declaration(&value);
        let formatted = tsv_css::format_str(&source)
            .unwrap_or_else(|e| panic!("{label}: {source:?} failed to parse: {e}"));
        assert_eq!(formatted, source, "{label}");
    }
}
