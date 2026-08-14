// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! Svelte's `read_identifier` (`1-parse/index.js:243`) has two halves — the ECMAScript
//! identifier **character class** and the **reserved-word** rejection — and it backs
//! **six** positions. Both halves are swept over all six here, because both have drifted
//! per-position before: this is the file that would have caught the shorthand attribute
//! spelling the class as `is_alphanumeric() || '_' || '$'` while every other position had
//! moved to `ID_Start`/`ID_Continue`.
//!
//! The six positions: a `{#snippet}` name and an `{#each}` index
//! (`1-parse/state/tag.js`), the plain-identifier binding of `{#each … as p}` /
//! `{:then p}` / `{:catch p}` — `read_pattern` opens with `parser.read_identifier()`
//! (`1-parse/read/context.js:16`) — and a shorthand attribute `{name}`
//! (`1-parse/state/element.js:575`). The list is Svelte's own `RESERVED_WORDS`
//! (`svelte/src/utils.js:43`) — the JS keywords plus the strict-mode future-reserved set
//! plus `eval` / `arguments` — applied at PARSE time in both modes, so it is a rule of
//! Svelte's template grammar rather than a JS early error and tsv enforces it rather than
//! deferring it (root `CLAUDE.md` §Strict Mode Only).
//!
//! ⚠️ The one position that really does defer is `read_pattern`'s **destructuring** branch
//! (`{#each xs as { p }}`), which falls through to acorn only because `read_identifier`
//! read nothing there. Reading that split the other way round — "a `read_pattern` position
//! goes to acorn, so `{#each items as eval}` parses" — is false against the oracle and had
//! left four of the six positions unguarded; `a_destructured_pattern_defers_to_acorn`
//! below is the contrast that keeps the true half honest.
//!
//! Representative spellings are pinned as fixtures — `blocks/head_reserved_identifier/`
//! and `attributes/shorthand_reserved_invalid/` for the reserved half,
//! `blocks/head_unicode_identifier/` and `attributes/shorthand_unicode_identifier/` for the
//! class. The whole 48-word list × 6 positions is here rather than there, because a fixture
//! per word would be 288 files stating one rule.
//!
//! The **controls are the load-bearing half** of both sweeps. `of` / `async` / `get` are
//! *not* on Svelte's list, so a fix reaching for "is this a JS-ish keyword" instead of the
//! list itself fails here rather than silently over-rejecting; and the class sweep asserts
//! two ACCEPTS beside its reject, so a fix reaching for "reject the odd characters"
//! fails the same way.

/// Svelte's `RESERVED_WORDS`, verbatim and in its own order.
const RESERVED_WORDS: &[&str] = &[
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// Identifiers that merely *look* reserved and are not on Svelte's list, so both parsers
/// keep accepting them.
const NEAR_MISSES: &[&str] = &["of", "async", "get", "set", "from", "as", "undefined"];

fn parse_error(source: &str) -> Option<String> {
    let arena = bumpalo::Bump::new();
    tsv_svelte::parse(source, &arena)
        .err()
        .map(|e| e.to_string())
}

/// `label` names the [`POSITIONS`] entry, so a failure says which of the six broke
/// rather than only which spelling did.
#[track_caller]
fn assert_reserved_rejected(source: &str, word: &str, label: &str) {
    let error = parse_error(source).unwrap_or_else(|| "<parsed successfully>".to_owned());
    assert!(
        error.contains(&format!("Unexpected reserved word '{word}'")),
        "{label}: expected the reserved-word rejection for {source:?}, got: {error}"
    );
}

#[track_caller]
fn assert_accepted(source: &str, label: &str) {
    assert!(
        parse_error(source).is_none(),
        "{label}: expected {source:?} to parse, got: {:?}",
        parse_error(source)
    );
}

/// The six `read_identifier` positions, each as a template whose `NAME` placeholder takes
/// the identifier. One list so a new position is added once and every test below covers
/// it — the reserved matrix, the near-miss controls, and the message pin.
///
/// The `{@render}` beside the snippet and the `{…}` echo beside each binding keep the
/// near-miss cases *used* rather than merely declared, so a word that bound but could not
/// be referenced could not pass as a near-miss. `NAME` rather than `{}` because these
/// templates are dense with real braces and a brace-shaped placeholder reads as one.
const POSITIONS: &[(&str, &str)] = &[
    (
        "snippet name",
        "{#snippet NAME()}<p>text</p>{/snippet}{@render NAME()}",
    ),
    (
        "each index",
        "{#each items as item, NAME}<p>text {NAME}</p>{/each}",
    ),
    (
        "each binding",
        "{#each items as NAME}<p>text {NAME}</p>{/each}",
    ),
    (
        "then binding",
        "{#await promise}<p>a</p>{:then NAME}<p>b {NAME}</p>{/await}",
    ),
    (
        "catch binding",
        "{#await promise}<p>a</p>{:catch NAME}<p>b {NAME}</p>{/await}",
    ),
    ("shorthand attribute", "<div {NAME}></div><p>{NAME}</p>"),
];

/// Substitute `word` for every `NAME` placeholder in a [`POSITIONS`] template.
fn render(template: &str, word: &str) -> String {
    template.replace("NAME", word)
}

#[test]
fn every_reserved_word_is_rejected_at_every_read_identifier_position() {
    for (label, template) in POSITIONS {
        for word in RESERVED_WORDS {
            assert_reserved_rejected(&render(template, word), word, label);
        }
    }
}

#[test]
fn near_miss_identifiers_still_parse_at_every_position() {
    for (label, template) in POSITIONS {
        for word in NEAR_MISSES {
            assert_accepted(&render(template, word), label);
        }
    }
}

/// The reader's OTHER half, the character class, swept over the same positions.
///
/// Spelled as `\u{…}` escapes rather than literal glyphs: two of the three are invisible
/// or near-invisible, and a literal one is the kind of byte that does not survive a
/// round-trip through an editor or a patch.
///
/// This exists because the class drifted exactly the way the reserved list did — the
/// shorthand attribute spelled it `is_alphanumeric() || '_' || '$'` and so diverged in
/// BOTH directions while every other position had been migrated. A per-position fixture
/// catches that only where someone thought to write one; a sweep over the shared list
/// catches it at whichever position is next to be added.
const CLASS_CASES: &[(&str, &str, bool)] = &[
    // `ID_Start` but not alphabetic — the direction a local predicate OVER-REJECTS.
    ("U+2118 script capital P", "\u{2118}", true),
    // `ID_Continue` but not alphanumeric — same direction, in the tail.
    ("U+200C ZWNJ in the tail", "a\u{200c}", true),
    // Alphanumeric but not `ID_Continue` — the direction it OVER-ACCEPTS. Canonical stops
    // the identifier at the `²`, then fails on whatever it expected next.
    ("U+00B2 superscript two", "a\u{b2}", false),
];

#[test]
fn the_identifier_class_is_ecmascript_at_every_position() {
    for (label, template) in POSITIONS {
        for (case, word, accepted) in CLASS_CASES {
            let source = render(template, word);
            let error = parse_error(&source);
            assert_eq!(
                error.is_none(),
                *accepted,
                "{label} / {case}: expected {} for {source:?}, got: {error:?}",
                if *accepted { "ACCEPT" } else { "REJECT" }
            );
        }
    }
}

/// The one position that genuinely defers: `read_pattern` reaches acorn only when
/// `read_identifier` read *nothing*, i.e. on the `{`/`[` destructuring branch. Canonical
/// rejects these too, but with acorn's own strict-mode wording rather than
/// `unexpected_reserved_word` — so this asserts the MECHANISM (which reader answered),
/// not merely the verdict, and would fail if the seam above were ever widened to swallow
/// the destructuring branch as well.
#[test]
fn a_destructured_pattern_defers_to_acorn() {
    for source in [
        "{#each items as { eval }}<p>text</p>{/each}",
        "{#each items as [yield]}<p>text</p>{/each}",
        "{#await promise}<p>a</p>{:then { arguments }}<p>b</p>{/await}",
    ] {
        let error = parse_error(source).unwrap_or_else(|| "<parsed successfully>".to_owned());
        assert!(
            !error.contains("Unexpected reserved word"),
            "expected acorn's verdict rather than the template-grammar one for {source:?}, \
             got: {error}"
        );
    }
}
