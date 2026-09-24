//! A Svelte block or tag pattern — the `{@const}` id, the `{:then}` / `{:catch}` value, the
//! `{#each}` context — is Svelte's `read_pattern` (`1-parse/read/context.js`): it must START
//! as an identifier or as a `{` / `[` opening a matched bracket, and it ENDS at the name or
//! the bracket's close (plus an optional `: T`). Whatever follows is the host's grammar — the
//! declarator's `=`, the tag's `}`.
//!
//! Handing the whole binding side to the TypeScript expression parser instead reads `(a)` as a
//! parenthesized target and `a.b` as a member target, both of which canonical rejects. For
//! `{@const}` the spellings that open with a paren also break the printer: the id's span stops
//! inside the shell, so the byte the printer takes for the declarator's `=` is the `)` — a
//! debug-assert panic in a debug build, and the parens silently dropped in a release one
//! (`{@const (a.b) = x}` → `{@const a.b = x}`).
//!
//! Each case is verified against canonical Svelte via `tsv_debug canonical_parse`.

/// Format `src` as a Svelte component — the path whose printer a paren-shell id would panic.
fn format(src: &str) -> Result<String, String> {
    tsv_svelte::format_str(src).map_err(|e| e.to_string())
}

/// The spellings that reached the printer's declarator-`=` assertion: a paren shell around the
/// whole `{@const}` id. Each must now fail to PARSE, so the printer never sees it.
#[test]
fn const_id_paren_shell_is_a_parse_error() {
    const INVALID: &[&str] = &[
        "{#if c}{@const (a) = x}{a}{/if}",
        "{#if c}{@const ((a)) = x}{a}{/if}",
        "{#if c}{@const (a.b) = x}{/if}",
        "{#if c}{@const ([a]) = x}{a}{/if}",
        "{#if c}{@const ({ a }) = x}{a}{/if}",
        "<script lang=\"ts\"></script>{#if c}{@const (a): T = x}{a}{/if}",
        "<script lang=\"ts\"></script>{#if c}{@const (a) : T = x}{a}{/if}",
    ];
    for src in INVALID {
        let err = format(src).expect_err(&format!("tsv should reject `{src}`"));
        assert!(
            err.contains("Expected identifier or destructure pattern"),
            "`{src}` should fail at the pattern head, got: {err}"
        );
    }
}

/// A `{@const}` binding that would continue as an expression past the name or bracket — the
/// declarator's `=` is missing where Svelte looks for it.
#[test]
fn const_id_expression_tail_is_a_parse_error() {
    const INVALID: &[&str] = &[
        "{#if c}{@const a.b = x}{/if}",
        "{#if c}{@const a?.b = x}{/if}",
        "{#if c}{@const a[0] = x}{/if}",
        "{#if c}{@const [a][0] = x}{/if}",
        "{#if c}{@const { a }.b = x}{/if}",
        "{#if c}{@const [a] [b] = x}{/if}",
        "{#if c}{@const a, b = x}{/if}",
    ];
    for src in INVALID {
        let err = format(src).expect_err(&format!("tsv should reject `{src}`"));
        assert!(
            err.contains("Expected token ="),
            "`{src}` should fail at the declarator's `=`, got: {err}"
        );
    }
}

/// A `{@const}` id is read by the shared `read_identifier`, not handed to the TypeScript
/// parser, so the reserved-word rule is Svelte's: the strict-mode and future-reserved words
/// (`let`, `yield`, `static`, …) and `eval` / `arguments` all reject, and so does an escaped
/// name, which `read_identifier` does not decode — its `\` is not a pattern start. The
/// TypeScript parser defers the reserved words as early errors and decodes the escape, so a
/// binding handed to it whole accepts every one of these.
#[test]
fn const_id_reserved_or_escaped_name_is_a_parse_error() {
    const INVALID: &[(&str, &str)] = &[
        ("let", "Unexpected reserved word"),
        ("yield", "Unexpected reserved word"),
        ("static", "Unexpected reserved word"),
        ("implements", "Unexpected reserved word"),
        ("package", "Unexpected reserved word"),
        ("private", "Unexpected reserved word"),
        ("protected", "Unexpected reserved word"),
        ("public", "Unexpected reserved word"),
        ("interface", "Unexpected reserved word"),
        ("eval", "Unexpected reserved word"),
        ("arguments", "Unexpected reserved word"),
        (r"\u0061", "Expected identifier or destructure pattern"),
        (r"\u{61}", "Expected identifier or destructure pattern"),
    ];
    for (id, message) in INVALID {
        let src = format!("{{#if c}}{{@const {id} = x}}{{/if}}");
        let err = format(&src).expect_err(&format!("tsv should reject `{src}`"));
        assert!(
            err.contains(message),
            "`{src}` should fail with `{message}`, got: {err}"
        );
    }
}

/// The bracket matcher is string- and template-aware but not regex-aware, like canonical's
/// `match_bracket`: the `]` inside `/]/` closes the pattern, and the regex left behind is
/// unterminated.
#[test]
fn await_value_bracket_closed_inside_regex_is_a_parse_error() {
    let src = "{#await p}x{:then [a = /]/]}y{/await}";
    let err = format(src).expect_err(&format!("tsv should reject `{src}`"));
    assert!(
        err.contains("Unterminated regular expression"),
        "`{src}` should fail on the cut regex, got: {err}"
    );
}

/// The `{:then}` / `{:catch}` value, in both the continuation and the shorthand head: a tail
/// past the pattern is left before the tag's `}`.
#[test]
fn await_value_expression_tail_is_a_parse_error() {
    const INVALID: &[&str] = &[
        "{#await p}x{:then a.b}y{/await}",
        "{#await p}x{:then [a][0]}y{/await}",
        "{#await p}x{:then a = 1}y{/await}",
        "{#await p}x{:catch a.b}y{/await}",
        "{#await p then a[0]}x{/await}",
        "{#await p catch a.b}x{/await}",
    ];
    for src in INVALID {
        assert!(format(src).is_err(), "tsv should reject `{src}`");
    }
}

/// What the head gate and the bounded extent must keep: a member or paren target INSIDE a
/// destructure (an assignment target to acorn), the shared identifier reader's full class, an
/// annotation whose end only the type parser can find, and a bracket matcher that steps over
/// strings and templates. Each formats, and formats to a fixed point.
#[test]
fn valid_patterns_still_format() {
    const VALID: &[&str] = &[
        "{#if c}{@const [(a)] = x}{a}{/if}",
        "{#if c}{@const [a.b] = x}{/if}",
        "{#if c}{@const { a: (b) } = x}{b}{/if}",
        "{#if c}{@const ä = x}{ä}{/if}",
        "{#if c}{@const 𝑎 = x}{𝑎}{/if}",
        "{#if c}{@const $a = x}{$a}{/if}",
        "{#if c}{@const $$props = x}{/if}",
        "{#if c}{@const\na\n=\nx}{a}{/if}",
        "{#if c}{@const { '}': a } = x}{a}{/if}",
        "{#if c}{@const [a = `]`] = x}{a}{/if}",
        "<script lang=\"ts\"></script>{#if c}{@const a: (x: number) => void = f}{a}{/if}",
        "<script lang=\"ts\"></script>{#if c}{@const { a }: { b: (c: number) => void } = x}{a}{/if}",
        "<script lang=\"ts\"></script>{#if c}{@const a: A<B> = x >= y}{a}{/if}",
        "<script lang=\"ts\"></script>{#if c}{@const a: Map<A>= x}{a}{/if}",
        "<script lang=\"ts\"></script>{#if c}{@const { a } : T = x}{a}{/if}",
        "<script lang=\"ts\"></script>{#if c}{@const [a]\n: T = x}{a}{/if}",
        "{#await p}x{:then [(a)]}{a}{/await}",
        "{#await p}x{:then { a }}{a}{/await}",
        "{#await p then a }{a}{/await}",
    ];
    for src in VALID {
        let once = format(src).unwrap_or_else(|e| panic!("tsv should accept `{src}`: {e}"));
        let twice = format(&once).unwrap_or_else(|e| panic!("`{src}` formatted to `{once}`: {e}"));
        assert_eq!(once, twice, "`{src}` is not idempotent");
    }
}

/// A destructure whose bracket never closes reports at its opening bracket, in the host
/// document's coordinates, at every `read_pattern` position.
#[test]
fn unmatched_pattern_bracket_reports_at_the_bracket() {
    const INVALID: &[(&str, &str)] = &[
        ("<p>hi</p>\n{#await p then [a}{/await}", "[a}"),
        ("<p>hi</p>\n{#await p}{:catch [e}{/await}", "[e}"),
        ("<p>hi</p>\n{#each xs as [a}{/each}", "[a}"),
        ("<p>hi</p>\n{#if c}{@const [a = 1}{/if}", "[a = 1}"),
    ];
    for (src, at) in INVALID {
        let arena = bumpalo::Bump::new();
        let Err(err) = tsv_svelte::parse(src, &arena) else {
            panic!("tsv should reject `{src}`");
        };
        assert_eq!(
            err.position(),
            src.find(at),
            "`{src}` should report at its opening bracket, got: {err}"
        );
    }
}
