// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A template expression that parses whole but leaves a token where its island must end
//! names the island's closing delimiter — the tag's `}`, an `{#each}` key's `)` — not the
//! end of the input: the document goes on past that delimiter, and the delimiter is what
//! the author has to write.
//!
//! Each such expression is parsed as an island bounded by its delimiter (the slice ends
//! there), so to the island's own parser the delimiter is the end of its input; the host
//! names it to the end check. Svelte says `Expected token }` (`Expected token )` for a key);
//! the message is tsv's own, and it names the same token.
//!
//! A `<script>` body is a whole program, so its end IS the end of what acorn reads there
//! (Svelte parses the body alone): it keeps `end of input`, the control below.
//!
//! Not fixturable: `input_invalid_*` asserts only that both parsers reject, never the
//! message or its point.

/// The rendered error for `source`: its message line and its `line:col` header.
fn error(source: &str) -> (String, String) {
    let arena = bumpalo::Bump::new();
    let rendered = format!(
        "{:#}",
        tsv_svelte::__private::parse(source, &arena).expect_err("the document must not parse")
    );
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

/// A complete expression followed by more than the tag's `}` — the brace is what was wanted.
#[test]
fn a_complete_expression_short_of_its_brace_names_the_brace() {
    check(&[
        ("<p>{a b}</p>", "Expected '}', found identifier", "1:7"),
        ("{@html a b}", "Expected '}', found identifier", "1:10"),
        ("{#if a b}x{/if}", "Expected '}', found identifier", "1:8"),
        (
            "<div class={a b}></div>",
            "Expected '}', found identifier",
            "1:15",
        ),
        (
            "<div {...a b}></div>",
            "Expected '}', found identifier",
            "1:12",
        ),
        ("{@const a = 1 2}", "Expected '}', found number", "1:15"),
        ("{@debug a b}", "Expected '}', found identifier", "1:11"),
        (
            "<div {@attach a b}></div>",
            "Expected '}', found identifier",
            "1:17",
        ),
    ]);
}

/// An `{#each}` key is bounded by its own parens, so its island ends at the `)`, not the
/// tag's `}` — the end is named per island, never assumed.
#[test]
fn an_each_key_names_its_close_paren() {
    check(&[
        (
            "{#each xs as x, i (a b)}y{/each}",
            "Expected ')', found identifier",
            "1:22",
        ),
        (
            "{#each xs as x (a 1)}y{/each}",
            "Expected ')', found number",
            "1:19",
        ),
    ]);
}

/// The `{#each}` iterable's slice stops at the binding's `as`, but an assertion keyword after a
/// line break is not the expression's (`as`/`satisfies` take no line terminator before them),
/// so a token is left there: reported as Svelte reports it, wanting the head's `}` — never
/// `Expected 'as', found 'as'`.
#[test]
fn an_each_iterable_left_short_names_the_head_brace() {
    check(&[
        (
            "{#each xs as A\nas B as item}y{/each}",
            "Expected '}', found 'as'",
            "2:1",
        ),
        (
            "{#each xs as A\nsatisfies B}y{/each}",
            "Expected '}', found 'satisfies'",
            "2:1",
        ),
    ]);
}

/// A `<script>` body ends where its program does.
#[test]
fn control_script_body_ends_its_own_input() {
    check(&[(
        "<script>a +</script>",
        "Expected expression, found end of input",
        "1:12",
    )]);
}
