// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! An inline element glued to text on both sides whose own attribute holds a `//` comment lays
//! out its content **block-style**, like every other attribute that forces the opening tag to
//! break (`elements/inline_glued_both_multiline_attr_prettier_divergence`).
//!
//! **Why a test rather than a fixture case.** Prettier deletes a trailing `//` in an attribute
//! expression (`data-attr={a}`), so it has no form of this document to grade against: neither an
//! `unformatted_*` variant nor a `prettier_variant_*` can hold it, and the comment position is
//! the part tsv keeps on purpose. This pins tsv's form and its idempotence.

fn format(source: &str) -> String {
    tsv_svelte::format_str(source).expect("formats")
}

const EXPECTED: &str = "<p>
\ttext1<span
\t\tdata-attr={a // c
\t\t}
\t>
\t\tinline
\t</span>text2
</p>
";

#[test]
fn line_comment_attr_lays_out_block_style() {
    for source in [
        "<p>text1<span data-attr={a // c\n}>inline</span>text2</p>\n",
        // the content hugged onto the attributes' `>` line
        "<p>\n\ttext1<span\n\t\tdata-attr={a // c\n\t\t}\n\t>inline</span>text2\n</p>\n",
    ] {
        assert_eq!(format(source), EXPECTED, "source: {source:?}");
    }
    assert_eq!(format(EXPECTED), EXPECTED);
}
