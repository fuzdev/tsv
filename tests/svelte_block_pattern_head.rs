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
//! The same reader decides whether a `{:then}` / `{:catch}` continuation HAS a value, and the
//! continuation dispatch ahead of it decides which keyword the `{:` names.
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

/// Every character canonical's `is_whitespace` admits (`1-parse/index.js`) — tsv's
/// `is_svelte_ws`, JS `\s`. CR is in the class, though the format path folds it to LF first.
const SVELTE_WS: &[char] = &[
    '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2001}',
    '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}',
    '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}', '\u{feff}',
];

/// A `{:then}` / `{:catch}` continuation is valueless only when its `}` follows the keyword
/// directly. Canonical's `next` tries `eat('}')` first and otherwise commits to a pattern, so
/// any whitespace run with no pattern behind it is `expected_pattern` — while the same run
/// ahead of a pattern, and the opening shorthand's `{#await p then<ws>}`, stay valid.
#[test]
fn await_continuation_whitespace_only_value_is_a_parse_error() {
    for &ws in SVELTE_WS {
        for keyword in ["then", "catch"] {
            let src = format!("{{#await p}}x{{:{keyword}{ws}}}y{{/await}}");
            let err = format(&src).expect_err(&format!("tsv should reject {src:?}"));
            assert!(
                err.contains("Expected identifier or destructure pattern"),
                "{src:?} should fail at the missing pattern, got: {err}"
            );
        }
        for src in [
            format!("{{#await p}}x{{:then{ws}v}}{{v}}{{/await}}"),
            format!("{{#await p}}x{{:catch v{ws}}}{{v}}{{/await}}"),
            format!("{{#await p then{ws}}}x{{/await}}"),
            format!("{{#await p catch{ws}}}x{{/await}}"),
        ] {
            format(&src).unwrap_or_else(|e| panic!("tsv should accept {src:?}: {e}"));
        }
    }
    for src in [
        "{#await p}x{:then}y{/await}",
        "{#await p}x{:catch}y{/await}",
    ] {
        format(src).unwrap_or_else(|e| panic!("tsv should accept `{src}`: {e}"));
    }
}

/// A continuation keyword must follow the `:` directly — canonical's `next` eats it with no
/// whitespace skip, though `allow_whitespace` runs between the `{` and the `:`. Reading past
/// the gap bound a value NAMED `then` in `{: then}` (printed `{:then then}`), and a gap holding
/// a multi-byte space panicked when `else` was sliced off by its byte length.
#[test]
fn continuation_keyword_after_whitespace_is_a_parse_error() {
    const INVALID: &[&str] = &[
        "{#await p}x{: then}y{/await}",
        "{#await p}x{ : then}y{/await}",
        "{#await p}x{: then v}y{/await}",
        "{#await p}x{: catch}y{/await}",
        "{#await p then v}x{: catch e}y{/await}",
        "{#if a}x{: else}y{/if}",
        "{#if a}x{: else if b}y{/if}",
        "{#each xs as x}x{: else}y{/each}",
        "{#if a}x{:\u{a0}\u{3000}else}y{/if}",
        "{#if a}x{:\u{a0}\u{3000}else if b}y{/if}",
        "{#each xs as x}x{:\u{a0}\u{3000}else}y{/each}",
        "{#await p}x{:\u{a0}\u{3000}then}y{/await}",
    ];
    for src in INVALID {
        assert!(format(src).is_err(), "tsv should reject `{src}`");
    }
    const VALID: &[&str] = &[
        "{#await p}x{ :then}y{/await}",
        "{#await p}x{ :then v}{v}{/await}",
        "{#await p}x{:then}y{ :catch e}{e}{/await}",
        "{#if a}x{ :else}y{/if}",
        "{#if a}x{ :else if b}y{/if}",
        "{#each xs as x}x{ :else}y{/each}",
    ];
    for src in VALID {
        format(src).unwrap_or_else(|e| panic!("tsv should accept `{src}`: {e}"));
    }
}
