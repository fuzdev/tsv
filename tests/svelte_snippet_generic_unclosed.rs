// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A `{#snippet}` head whose generic `<` never closes is reported as the missing `>`, at the
//! end of the head — not as the end of the file, which the document has not reached.
//!
//! The head is bounded by its `}` before the generic is read (`scan_block_tag_content`), so
//! an unclosed `<` runs out of HEAD, the way an unclosed `(` does — and that one already
//! reads `Expected ')'` at the head's end, the control below. Svelte's own scanner
//! (`match_bracket` with `pointy_bois`, in a `lang="ts"` document) is not bounded by the
//! head and walks to the end of the template, reporting `Unexpected end of input` there;
//! the message is tsv's own either way, and a mid-document `Unexpected end of file` names
//! neither.
//!
//! Not fixturable: `input_invalid_*` asserts only that both parsers reject, never the
//! message or its point.

/// The rendered error for `source`: its message line and its `line:col` header.
fn error(source: &str) -> (String, String) {
    let arena = bumpalo::Bump::new();
    let rendered = tsv_svelte::parse(source, &arena)
        .expect_err("the head must not parse")
        .to_string();
    let mut lines = rendered.lines();
    let message = lines.next().expect("a message line").to_owned();
    let header = lines
        .next()
        .and_then(|located| located.split(' ').next())
        .expect("a located `line:col` line")
        .to_owned();
    (message, header)
}

/// Every `(source, message, header)` case, failures collected so one run names them all.
#[track_caller]
fn check(cases: &[(&str, &str, &str)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|&(source, message, header)| {
            let actual = error(source);
            let expected = (message.to_owned(), header.to_owned());
            (actual != expected).then(|| {
                format!("{source:?}\n    tsv:      {actual:?}\n    expected: {expected:?}")
            })
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} case(s) differ:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn an_unclosed_snippet_generic_expects_its_close_at_the_head_end() {
    check(&[
        // the `}` closing the head sits at column 44
        (
            "<script lang=\"ts\"></script>{#snippet s<T(a)}{/snippet}<p>after</p>",
            "Expected '>'",
            "1:44",
        ),
        // the head's trailing whitespace is not part of it
        (
            "<script lang=\"ts\"></script>{#snippet s<T(a) }{/snippet}<p>after</p>",
            "Expected '>'",
            "1:44",
        ),
        // a nested `<` leaves the outer one open
        (
            "<script lang=\"ts\"></script>\n{#snippet s<T<U>(a)}\n\t<p>{a}</p>\n{/snippet}\n",
            "Expected '>'",
            "2:20",
        ),
    ]);
}

/// The sibling the generic's message follows: an unclosed parameter `(`.
#[test]
fn control_an_unclosed_parameter_list_expects_its_close_at_the_head_end() {
    check(&[(
        "<script lang=\"ts\"></script>{#snippet s(a}{/snippet}<p>after</p>",
        "Expected ')'",
        "1:41",
    )]);
}
