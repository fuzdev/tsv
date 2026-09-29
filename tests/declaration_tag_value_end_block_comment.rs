// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! A `{let}` / `{const}` tag whose printed declaration ENDS on a same-line block comment the
//! value leaves there — the run of a grouping pair the printer strips, or the one it prints
//! past a pair the chain's last operand requires — carries the `;` that must stand between
//! that comment and the `}`: Svelte rejects `{let a = x || y /* c */}` (`Expected token }`).
//!
//! Not a fixture: prettier's first pass over the stripped-pair authoring drops the `;` and
//! prints exactly that rejected form, so no fixture shape can hold the authoring — the
//! validator needs prettier's first pass to parse or to land on a documented form, and from
//! `input.svelte` prettier moves the comment past the `;`, which Svelte rejects too. Each case
//! pins the exact output and that the output is its own fixed point. The sibling authorings
//! whose prettier pass does parse are the fixture
//! `svelte/tags/declaration/declaration_terminator_stripped_paren_comment_prettier_divergence`.

fn format(source: &str) -> String {
    tsv_svelte::format_str(source).expect("svelte format_str")
}

fn assert_formats(source: &str, expected: &str) {
    let out = format(source);
    assert_eq!(out, expected, "first pass of:\n{source}");
    assert_eq!(
        format(&out),
        out,
        "the output must be its own fixed point:\n{out}"
    );
}

/// The stripped pair's same-line block ends the declaration.
#[test]
fn stripped_pair_block_ends_declaration() {
    assert_formats(
        "{let a = x || (y /* c */)}\n",
        "{let a = x || y /* c */;}\n",
    );
}

/// The comment in a required pair prints past its `)`, and ends the declaration there.
#[test]
fn required_pair_block_ends_declaration() {
    assert_formats(
        "{let e = x - (y - z /* c */)}\n",
        "{let e = x - (y - z) /* c */;}\n",
    );
}

/// The `{const}` spelling takes the same `;`.
#[test]
fn const_tag_takes_the_same_terminator() {
    assert_formats(
        "{const a = x || (y /* c */)}\n",
        "{const a = x || y /* c */;}\n",
    );
}

/// A block the value prints inside itself ends nothing: no `;`.
#[test]
fn block_inside_the_value_takes_no_terminator() {
    assert_formats(
        "{let a = f(x || (y /* c */))}\n",
        "{let a = f(x || y /* c */)}\n",
    );
}
