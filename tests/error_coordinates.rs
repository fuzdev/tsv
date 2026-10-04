// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A parse error's point, graded against an independent reference.
//!
//! Every located parse error reports where it sits in its document's wire coordinates
//! (`ParseError::wire_point`): `start`, the UTF-16 offset a wire node there would carry,
//! and `line` (1-based) / `column` (0-based, UTF-16 units) under the document's line rule —
//! ECMAScript's terminators for a TypeScript document, LF alone for a Svelte document
//! (every island in it included) and for a CSS one — with a leading byte-order mark
//! counted by TypeScript and elided by Svelte and CSS. That is the `loc` definition
//! (`tests/loc_definition.rs`), so an error's point is the `loc` a node at the error would
//! carry. The message's `line:col` header prints the same point, as `line:column+1`, and
//! the excerpt under it never echoes a leading BOM, whichever way the header counts it.
//!
//! A format reports the error of the caller's source, not of the CR-folded text its parse
//! actually reads: the same point and message a parse of that source reports.
//!
//! The reference is the test's own — a line-start scan over the document's UTF-16 units —
//! so it shares nothing with `tsv_lang`. It grades every `input_invalid_*` fixture file
//! (each as written, and with its line feeds respelled CRLF) and the inputs no fixture can
//! hold: a lone CR, U+2028, a BOM, an astral character ahead of the error — and, since no
//! invalid fixture is CSS, a table of invalid CSS. Each synthetic case also names where its
//! error sits, so the error's byte `position` is graded as well as the point taken from it.

use bumpalo::Bump;
use std::path::Path;
use tsv_lang::ParseError;

#[path = "support/utf16_lines.rs"]
mod utf16_lines;
use utf16_lines::{LineRule, line_column, line_starts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    TypeScript,
    Svelte,
    Css,
}

/// The `(start, line, column)` of byte offset `position` in `source`, computed from
/// scratch: clamped to the end, floored to a character boundary, over the UTF-16 units of
/// the text the wire indexes.
fn reference_point(source: &str, position: usize, language: Language) -> (u32, u32, u32) {
    let mut position = position.min(source.len());
    while !source.is_char_boundary(position) {
        position -= 1;
    }
    let bom = if language != Language::TypeScript && source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let rule = match language {
        Language::TypeScript => LineRule::Ecmascript,
        Language::Svelte | Language::Css => LineRule::Lf,
    };
    let units: Vec<u16> = source[bom..].encode_utf16().collect();
    let offset = source[bom..position.max(bom)].encode_utf16().count();
    let (line, column) = line_column(&line_starts(&units, rule), offset);
    (offset as u32, line as u32, column as u32)
}

/// The parse error for `source`. A TypeScript source parses as the format path does when
/// no source type is named — module, retried as a script — so the parse and the format
/// it is compared with report from the same grammar.
fn parse_error(source: &str, language: Language) -> Option<ParseError> {
    let arena = Bump::new();
    match language {
        Language::TypeScript => tsv_ts::parse_with_goal_or_fallback(source, None, &arena).err(),
        Language::Svelte => tsv_svelte::parse(source, &arena).err(),
        Language::Css => tsv_css::parse(source, &arena).err(),
    }
}

fn format_error(source: &str, language: Language) -> Option<ParseError> {
    match language {
        Language::TypeScript => tsv_ts::format_str(source).err(),
        Language::Svelte => tsv_svelte::format_str(source).err(),
        Language::Css => tsv_css::format_str(source).err(),
    }
}

/// Every way `error`'s point disagrees with the reference over `source`.
fn point_violations(source: &str, language: Language, error: &ParseError) -> Vec<String> {
    let mut out = Vec::new();
    let Some(position) = error.position() else {
        return vec!["a positionless error".to_string()];
    };
    let Some(point) = error.wire_point() else {
        return vec!["a located error with no point".to_string()];
    };
    let expected = reference_point(source, position, language);
    if (point.start, point.line, point.column) != expected {
        out.push(format!(
            "point {:?} != reference {expected:?} at byte {position}",
            (point.start, point.line, point.column)
        ));
    }
    let rendered = error.to_string();
    let located = rendered
        .lines()
        .nth(1)
        .and_then(|line| line.split_once(' '));
    let header = located.map(|(head, _)| head);
    let want = format!("{}:{}", point.line, point.column + 1);
    if header != Some(want.as_str()) {
        out.push(format!("header {header:?} != {want}"));
    }
    // the excerpt is display text: it never echoes a leading BOM, whatever the header counts
    if located.is_some_and(|(_, excerpt)| excerpt.starts_with('\u{feff}')) {
        out.push(format!("the excerpt echoes a leading BOM: {rendered:?}"));
    }
    out
}

/// Where a synthetic case's error sits, read off the source it is graded over (so a CRLF
/// respelling moves it with the text): the start of the first or last occurrence of a
/// fragment, just past the last one, or the end of the source. The ground truth the error's
/// own `position` is held to — the point is then graded from it.
#[derive(Clone, Copy, Debug)]
enum At {
    First(&'static str),
    Last(&'static str),
    AfterLast(&'static str),
    End,
}

impl At {
    fn offset(self, source: &str) -> usize {
        match self {
            At::First(fragment) => source.find(fragment).expect("the anchor fragment"),
            At::Last(fragment) => source.rfind(fragment).expect("the anchor fragment"),
            At::AfterLast(fragment) => {
                source.rfind(fragment).expect("the anchor fragment") + fragment.len()
            }
            At::End => source.len(),
        }
    }
}

/// Grade one failing source: its parse error against the reference — at `at`, when the
/// caller knows where the error sits — and its format error against its parse error.
/// `None` when the source parses.
fn grade(source: &str, language: Language, at: Option<At>) -> Option<Vec<String>> {
    let parsed = parse_error(source, language)?;
    let mut out = Vec::new();
    if let Some(at) = at {
        let want = at.offset(source);
        if parsed.position() != Some(want) {
            out.push(format!(
                "position {:?} != {want} ({at:?})",
                parsed.position()
            ));
        }
    }
    out.extend(point_violations(source, language, &parsed));
    match format_error(source, language) {
        None => out.push("the format succeeded where the parse failed".to_string()),
        Some(formatted) => {
            if formatted.wire_point() != parsed.wire_point() {
                out.push(format!(
                    "format point {:?} != parse point {:?}",
                    formatted.wire_point(),
                    parsed.wire_point()
                ));
            }
            if formatted.to_string() != parsed.to_string() {
                out.push(format!(
                    "format message {:?} != parse message {:?}",
                    formatted.to_string(),
                    parsed.to_string()
                ));
            }
        }
    }
    Some(out)
}

fn invalid_inputs(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read a fixture directory") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            invalid_inputs(&path, out);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("input_invalid_"))
        {
            out.push(path);
        }
    }
}

#[test]
fn every_invalid_fixture_reports_its_point() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut paths = Vec::new();
    invalid_inputs(&root, &mut paths);
    paths.sort();
    // per language: TypeScript, Svelte, CSS
    let mut graded = [0usize; 3];
    let mut failures = Vec::new();
    for path in paths {
        let name = path.to_string_lossy();
        let language = if name.ends_with(".svelte") {
            Language::Svelte
        } else if name.ends_with(".ts") {
            Language::TypeScript
        } else if name.ends_with(".css") {
            Language::Css
        } else {
            panic!("{name}: an invalid input in no language this test grades");
        };
        let source = std::fs::read_to_string(&path).expect("read the input");
        let mut variants = vec![("as written", source.clone())];
        if !source.contains('\r') && source.contains('\n') {
            variants.push(("CRLF", source.replace('\n', "\r\n")));
        }
        for (variant, text) in variants {
            // A Script-goal fixture is invalid at its own goal, and may parse under the
            // fallback; there is no error to grade then.
            let Some(found) = grade(&text, language, None) else {
                continue;
            };
            if !found.is_empty() {
                failures.push(format!("{name} ({variant}):\n  {}", found.join("\n  ")));
            }
            graded[match language {
                Language::TypeScript => 0,
                Language::Svelte => 1,
                Language::Css => 2,
            }] += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "{} invalid input(s) report a wrong point:\n{}",
        failures.len(),
        failures
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    let [typescript, svelte, css] = graded;
    assert!(
        typescript > 0 && svelte > 0,
        "every fixture language graded: {graded:?}"
    );
    // No `input_invalid_*.css` fixture exists — CSS error points are graded by the
    // synthetic `CSS_CASES` alone. One added here is graded above; raise this floor then.
    assert_eq!(
        css, 0,
        "an invalid CSS fixture now exists: give CSS a floor here"
    );
}

/// The inputs no fixture holds — a terminator other than LF ahead of the error, a BOM, an
/// astral character — in every language and every island of a Svelte document, each also
/// respelled with CRLF line breaks, which the format path folds.
#[test]
fn synthetic_errors_report_their_points() {
    let cases: &[(Language, &str, At)] = &[
        (
            Language::TypeScript,
            "const 𝒜 = 1;\nconst = ;",
            At::Last("="),
        ),
        (Language::TypeScript, "\u{feff}const = ;", At::Last("=")),
        (Language::TypeScript, "\u{feff}a;\nconst = ;", At::Last("=")),
        (Language::TypeScript, "a;\rconst = ;", At::Last("=")),
        (
            Language::TypeScript,
            "a;\u{2028}b;\u{2029}const = ;",
            At::Last("="),
        ),
        (
            Language::TypeScript,
            "function f() {\n\tlet x = '𝒜';\n",
            At::End,
        ),
        // the module grammar fails at `with`, the script retry further on: the format
        // fallback reports the further error, and so does the parse it is graded against
        (
            Language::TypeScript,
            "with (a) {}\nconst = ;",
            At::Last("="),
        ),
        (
            Language::Svelte,
            "<script>\nlet a = '𝒜';\nconst = ;\n</script>",
            At::Last("="),
        ),
        (Language::Svelte, "\u{feff}<div {", At::Last("{")),
        (Language::Svelte, "\u{feff}<p>x</p>\n{a +}", At::Last("}")),
        (Language::Svelte, "<p>a\rb</p>\n{a +}", At::Last("}")),
        (Language::Svelte, "<p>a\u{2028}b</p>{a +}", At::Last("}")),
        (
            Language::Svelte,
            "<script>\na;\rconst = ;\n</script>",
            At::Last("="),
        ),
        (
            Language::Svelte,
            "<style>\na { color: red; }\n𝒜 {\n</style>",
            At::Last("</style>"),
        ),
        (Language::Svelte, "<div>{a b}</div>", At::Last("b")),
        (Language::Svelte, "<p>𝒜</p>\n<div", At::End),
    ];
    let mut cases = cases.to_vec();
    cases.extend_from_slice(CSS_CASES);
    let backslash =
        |&(language, source): &(Language, &'static str)| (language, source, At::First("\\"));
    cases.extend(ESCAPE_CASES.iter().map(backslash));
    cases.extend(OPTIONAL_TOKEN_ESCAPE_CASES.iter().map(backslash));
    let mut failures = Vec::new();
    for (language, source, at) in cases {
        let mut variants = vec![source.to_string()];
        if !source.contains('\r') && source.contains('\n') {
            variants.push(source.replace('\n', "\r\n"));
        }
        for text in variants {
            let found = grade(&text, language, Some(at))
                .unwrap_or_else(|| vec!["the source parses; the case asserts nothing".to_string()]);
            if !found.is_empty() {
                failures.push(format!("{language:?} {text:?}:\n  {}", found.join("\n  ")));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Invalid CSS — the one language no `input_invalid_*` fixture covers, so its points are
/// graded here alone: a leading BOM, lone terminators, and then, each behind content
/// holding an astral character and a line break so a position counted in the wrong units
/// or from the wrong line cannot pass, an unclosed block or at-rule, a stray closer and
/// opener, an unterminated comment and string, and the selector grammar's own errors.
const CSS_CASES: &[(Language, &str, At)] = &[
    (Language::Css, "\u{feff}a {", At::End),
    (Language::Css, "a { color: red; }\rb {", At::End),
    (Language::Css, "/* \u{2028} 𝒜 */ a {", At::End),
    (Language::Css, "a {\n  b: c;\n", At::End),
    (Language::Css, "a { b: '𝒜' }\n\tb {\n\t\tc: d;\n", At::End),
    (Language::Css, "a { b: '𝒜' }\n@media screen {", At::End),
    (Language::Css, "a { b: '𝒜' }\n@import", At::End),
    (Language::Css, "a { b: '𝒜' }\nb { c: d; }}", At::Last("}")),
    (Language::Css, "a { b: '𝒜' }\n{ b: c }", At::Last("{")),
    (
        Language::Css,
        "a {}\n/* 𝒜 */ b { c: d }\n/* x",
        At::Last("/*"),
    ),
    (Language::Css, "a { b: '𝒜' }\n\"x", At::Last("\"")),
    (Language::Css, "a { b: '𝒜' }\nb { : c }", At::AfterLast(":")),
    (Language::Css, "a { b: '𝒜' }\nb[ { }", At::Last("{")),
    (Language::Css, "a { b: '𝒜' }\nb { c d }", At::Last("}")),
];

/// Malformed escapes, each behind content and a line break so a position relative to the
/// literal (or to its island) cannot pass for the escape's own: the decoder's errors, a
/// template's deferred one, the strict-mode legacy escapes (graded where the literal
/// becomes a node, and retroactively by a later `"use strict"`), each in a Svelte island
/// too, and a CSS identifier's escape.
const ESCAPE_CASES: &[(Language, &str)] = &[
    (Language::TypeScript, "let a;\nlet s = \"\\u{zz}\";"),
    (Language::TypeScript, "let a;\nlet s = '\\x4';"),
    (Language::TypeScript, "let a;\nlet s = '\\u12';"),
    (Language::TypeScript, "let a;\nlet s = '\\u{12';"),
    (Language::TypeScript, "let a;\nlet s = '\\u{110000}';"),
    (Language::TypeScript, "let a;\nlet s = '\\u{}';"),
    // content ahead of the escape INSIDE its literal or template segment, which is what
    // tells the backslash from the start of the slice the decoder was handed
    (Language::TypeScript, "let a;\nlet s = 'ab\\u{zz}';"),
    (Language::TypeScript, "let a;\nlet t = `x${a}y\\u{zz}`;"),
    (Language::TypeScript, "let a;\nlet s = 'ab\\x4';"),
    (
        Language::TypeScript,
        "let a = '𝒜';\nlet t = `x${a}\\u{zz}`;",
    ),
    (Language::TypeScript, "'use strict';\nlet s = '\\7';"),
    (
        Language::TypeScript,
        "function f() {\n\t'\\08';\n\t'use strict';\n}",
    ),
    (
        Language::Svelte,
        "<div>hi</div>\n<script>\n\tlet s = '\\u{zz}';\n</script>",
    ),
    (
        Language::Svelte,
        "<div>hi</div>\n<script>\n\tlet t = `\\x4`;\n</script>",
    ),
    (
        Language::Svelte,
        "<div>hi</div>\n<script>\n\tlet s = '\\7';\n</script>",
    ),
    (Language::Svelte, "<p>x</p>\n{'\\u{zz}'}"),
    (Language::Svelte, "<p>x</p>\n{'ab\\u{zz}'}"),
    (Language::Svelte, "<p>𝒜</p>\n<div title={`\\x4`}></div>"),
    (Language::Svelte, "<p>x</p>\n{#if '\\u12'}{/if}"),
    (Language::Css, "a {}\n.b\\\n{}"),
    (
        Language::Svelte,
        "<p>x</p>\n<style>\n\ta {}\n\t.b\\\n{}\n</style>",
    ),
];

/// Malformed escapes in the token right after an OPTIONAL one — a parameter default's `=`
/// (every parameter form, and an arrow's or a `{#snippet}` head's list), a class field's,
/// an enum member's, a type parameter default's, a binding pattern's and a for-head
/// declarator's `=`, a property value's `:`, a second argument's `,`. The parser consumes
/// such a token only when it is present, and consuming it lexes the token after it; that
/// lexer error must not read as the optional token's absence, which reports a stray `=` /
/// `:` / `,` in its place.
const OPTIONAL_TOKEN_ESCAPE_CASES: &[(Language, &str)] = &[
    (Language::TypeScript, "let a;\nfunction s(b = '\\x4') {}"),
    (Language::TypeScript, "let a;\nlet f = (b = '\\x4') => 1;"),
    (
        Language::TypeScript,
        "let a;\nclass C {\n\tm(b = '\\x4') {}\n}",
    ),
    (Language::TypeScript, "let a;\nfunction s([b] = '\\x4') {}"),
    (Language::TypeScript, "let a;\nfunction s(b = '\\x4', c) {}"),
    (Language::TypeScript, "let a;\nfunction s(b, c = '\\x4') {}"),
    (
        Language::TypeScript,
        "let a;\nfunction s(b = '\\u{110000}') {}",
    ),
    (Language::TypeScript, "let a;\nfunction s(b: T = '\\x4') {}"),
    (
        Language::TypeScript,
        "let a;\nclass C {\n\tconstructor(private b = '\\x4') {}\n}",
    ),
    (Language::TypeScript, "let a;\nclass C {\n\tb = '\\x4';\n}"),
    (Language::TypeScript, "let a;\nenum E {\n\tB = '\\x4'\n}"),
    (Language::TypeScript, "let a;\ntype T<B = '\\x4'> = B;"),
    (Language::TypeScript, "let a;\nlet { b = '\\x4' } = a;"),
    (Language::TypeScript, "let a;\nfor (let b = '\\x4'; ; ) {}"),
    (Language::TypeScript, "let a;\nlet o = { b: '\\x4' };"),
    (Language::TypeScript, "let a;\nimport('a', '\\x4');"),
    (
        Language::Svelte,
        "<p>x</p>\n{#snippet s(b = '\\x4')}{/snippet}",
    ),
    (
        Language::Svelte,
        "<div>hi</div>\n<script>\n\tfunction s(b = '\\x4') {}\n</script>",
    ),
];

/// A malformed escape reports the same error wherever its literal sits: each case's
/// message is the one the same literal reports as a `let` initializer.
#[test]
fn an_escape_after_an_optional_token_reports_the_escape() {
    let mut failures = Vec::new();
    for &(language, source) in OPTIONAL_TOKEN_ESCAPE_CASES {
        let backslash = source.find('\\').expect("an escape");
        let open = source[..backslash].rfind('\'').expect("an opening quote");
        let close = backslash + source[backslash..].find('\'').expect("a closing quote");
        let control = format!("let s = {};", &source[open..=close]);
        let message = |source: &str, language| {
            parse_error(source, language).map(|error| {
                let rendered = error.to_string();
                rendered.lines().next().unwrap_or_default().to_string()
            })
        };
        let want = message(&control, Language::TypeScript);
        let found = message(source, language);
        if want.is_none() || found != want {
            failures.push(format!(
                "{language:?} {source:?}: {found:?}, the `let` control {control:?}: {want:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every malformed escape is reported at its own backslash — the first in the source —
/// and its message excerpts the backslash's own line. (Its point and header are graded
/// with the synthetic cases.)
#[test]
fn escape_errors_point_at_their_backslash() {
    let mut failures = Vec::new();
    for &(language, source) in ESCAPE_CASES.iter().chain(OPTIONAL_TOKEN_ESCAPE_CASES) {
        let backslash = source.find('\\').expect("an escape");
        let Some(error) = parse_error(source, language) else {
            failures.push(format!("{language:?} {source:?}: parses"));
            continue;
        };
        let position = error.position();
        if position != Some(backslash) {
            failures.push(format!(
                "{language:?} {source:?}: at {position:?}, the backslash at {backslash}"
            ));
        }
        // the cases break lines with LF alone, so the display line is the LF-bounded one
        let line_start = source[..backslash].rfind('\n').map_or(0, |i| i + 1);
        let line_end = source[backslash..]
            .find('\n')
            .map_or(source.len(), |i| backslash + i);
        let rendered = error.to_string();
        let excerpt = rendered
            .lines()
            .nth(1)
            .and_then(|line| line.split_once(' '))
            .map(|(_, text)| text);
        if excerpt != Some(&source[line_start..line_end]) {
            failures.push(format!(
                "{language:?} {source:?}: excerpt {excerpt:?}, the backslash's line {:?}",
                &source[line_start..line_end]
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// An embedded entry point reports a position and no point: its source is a slice of a
/// host document, whose coordinates only the host can state (`tsv_svelte::parse` fills the
/// context over the whole document).
#[test]
fn an_embedded_parse_error_has_no_point_until_its_host_fills_it() {
    let arena = Bump::new();
    let error = tsv_ts::parse_embedded("let = ;", 5, &arena).expect_err("a syntax error");
    assert!(error.position().is_some());
    assert!(error.wire_point().is_none());
    let error = tsv_css::parse_embedded("a {", 5, &arena).expect_err("a syntax error");
    assert!(error.position().is_some());
    assert!(error.wire_point().is_none());
}

/// The points the reference computes for the synthetic cases' distinguishing features,
/// spelled out — the reference grades the implementation, and these grade the reference.
#[test]
fn the_reference_reads_each_rule() {
    // astral: two units
    assert_eq!(reference_point("𝒜x", 4, Language::TypeScript), (2, 1, 2));
    // BOM: counted by TypeScript, elided by Svelte and CSS
    assert_eq!(
        reference_point("\u{feff}x", 3, Language::TypeScript),
        (1, 1, 1)
    );
    assert_eq!(reference_point("\u{feff}x", 3, Language::Svelte), (0, 1, 0));
    assert_eq!(reference_point("\u{feff}x", 3, Language::Css), (0, 1, 0));
    // lone CR: a line for TypeScript, a character for Svelte and CSS
    assert_eq!(reference_point("a\rb", 2, Language::TypeScript), (2, 2, 0));
    assert_eq!(reference_point("a\rb", 2, Language::Svelte), (2, 1, 2));
    // CRLF: one terminator everywhere
    assert_eq!(
        reference_point("a\r\nb", 3, Language::TypeScript),
        (3, 2, 0)
    );
    assert_eq!(reference_point("a\r\nb", 3, Language::Css), (3, 2, 0));
}

/// `WireCoordinates::point` against the loc wire itself: over sources holding the
/// synthetic cases' features, every wire object's `start` and `end` — read back to the
/// byte it indexes — is the point `point` gives that byte, `loc` included. The reference
/// above grades `point` from scratch; this ties it to the one `loc` the wire writers emit
/// from their own tables, so the error path and the wire cannot drift apart.
#[test]
fn the_point_of_every_wire_position_is_its_loc() {
    let cases: &[(Language, &str)] = &[
        (Language::TypeScript, "const 𝒜 = 1;\nconst b = `𝒜\n${𝒜}`;"),
        (Language::TypeScript, "\u{feff}a;\nb;"),
        (Language::TypeScript, "a;\rb;\r\nc;"),
        (Language::TypeScript, "a;\u{2028}b;\u{2029}c;"),
        (Language::TypeScript, "let s = 'x\\\r\ny';\nlet t = 1;"),
        (
            Language::Svelte,
            "<script>\nlet a = '𝒜';\nconst b = 1;\n</script>\n<p>{a}</p>",
        ),
        (Language::Svelte, "\u{feff}<p>x</p>\n{a + b}"),
        (Language::Svelte, "<p>a\rb</p>\n{a}\r\n<i>{b}</i>"),
        (Language::Svelte, "<p>a\u{2028}b</p>{a}"),
        (Language::Svelte, "<script>\na;\rb;\n</script>"),
        (
            Language::Svelte,
            "<style>\na { color: red; }\n𝒜 { b: c; }\n</style>",
        ),
        (Language::Css, "\u{feff}a {}\nb { c: d; }"),
        (Language::Css, "a { color: red; }\rb {}\r\nc {}"),
        (Language::Css, "/* \u{2028} 𝒜 */ a {}\n𝒜 { b: c; }"),
    ];
    let mut graded = 0;
    let mut failures = Vec::new();
    for &(language, source) in cases {
        let arena = Bump::new();
        let (wire, coordinates) = match language {
            Language::TypeScript => (
                tsv_ts::convert_ast_json_bytes_with_locations(
                    &tsv_ts::parse(source, &arena).expect("parses"),
                    source,
                ),
                tsv_ts::WIRE_COORDINATES,
            ),
            Language::Svelte => (
                tsv_svelte::convert_ast_json_bytes_with_locations(
                    &tsv_svelte::parse(source, &arena).expect("parses"),
                    source,
                ),
                tsv_svelte::WIRE_COORDINATES,
            ),
            Language::Css => (
                tsv_css::convert_ast_json_bytes_with_locations(
                    &tsv_css::parse(source, &arena).expect("parses"),
                    source,
                ),
                tsv_css::WIRE_COORDINATES,
            ),
        };
        let wire: serde_json::Value = serde_json::from_slice(&wire).expect("the wire is JSON");
        // the byte each UTF-16 offset of the text the wire indexes sits at
        let elided = if language != Language::TypeScript && source.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        let mut byte_of = Vec::new();
        for (i, ch) in source[elided..].char_indices() {
            byte_of.extend(std::iter::repeat_n(elided + i, ch.len_utf16()));
        }
        byte_of.push(source.len());
        let mut stack = vec![&wire];
        while let Some(value) = stack.pop() {
            match value {
                serde_json::Value::Array(items) => stack.extend(items),
                serde_json::Value::Object(map) => {
                    stack.extend(map.values());
                    let Some(loc) = map.get("loc") else { continue };
                    for edge in ["start", "end"] {
                        let (Some(offset), Some(point)) = (
                            map.get(edge).and_then(serde_json::Value::as_u64),
                            loc.get(edge),
                        ) else {
                            continue;
                        };
                        let byte = byte_of[usize::try_from(offset).expect("an offset")];
                        let ours = coordinates.point(source, byte);
                        let wire_point = (
                            offset,
                            point.get("line").and_then(serde_json::Value::as_u64),
                            point.get("column").and_then(serde_json::Value::as_u64),
                        );
                        let ours = (
                            u64::from(ours.start),
                            Some(u64::from(ours.line)),
                            Some(u64::from(ours.column)),
                        );
                        if ours != wire_point {
                            failures.push(format!(
                                "{language:?} {source:?} {edge} @ byte {byte}: point {ours:?}, \
                                 wire {wire_point:?}"
                            ));
                        }
                        graded += 1;
                    }
                }
                _ => {}
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(graded > 100, "graded {graded} positions");
}
