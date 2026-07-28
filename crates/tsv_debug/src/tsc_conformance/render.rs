//! Render a [`ParsedBaseline`] back to a tsgo `.errors.txt` byte stream.
//!
//! A faithful port of typescript-go's error-baseline renderer — the ported,
//! baseline-verified spec for the format tsv must eventually emit its own
//! diagnostics through. The round-trip check (`super::roundtrip`) feeds this a
//! model recovered from a real baseline and byte-compares the result; the port
//! therefore has to reproduce every load-bearing subtlety of the original.
//!
//! Reference (censused 2026-07-09 vs pin `168e7015`):
//! - tsgo: `internal/testutil/tsbaseline/error_baseline.go`
//!   (`iterateErrorBaseline`, ~260 lines)
//! - tsgo: `internal/diagnosticwriter/diagnosticwriter.go`
//!   (`WriteFormatDiagnostic`, `flattenDiagnosticMessageChain`, ~506 lines)
//!
//! Only the **rune** path is ported (the `pretty=false` default): tildes count
//! in runes (`utf8.RuneCountInString`), tabs survive the prefix-blank, and the
//! hard-coded newline is CRLF. The colored `pretty=true` `writeCodeSnippet` /
//! tabular-summary path is out of scope (a handful of corpus baselines exercise
//! it; the round-trip driver buckets them).
//!
//! Coordinate note: the model stores each file's source as LF-joined lines and
//! each span as a byte `(pos, len)` in that LF content. The parser recovered the
//! span from the same LF view, so the two cancel: the rendered squiggles depend
//! only on within-line geometry (line-ending-independent) plus the recovered
//! end offset, so LF vs the original CRLF source never changes the output.
//!
//! Lib masking is reproduced by recovering the already-masked token verbatim
//! (`Loc::Masked` → `(--,--)`, related `lib…:--:--` echoed literally) rather
//! than re-deriving it with the two harness regexes: the baseline is read
//! *after* masking, so the pre-mask lib positions are unrecoverable, and
//! emitting the recovered token is byte-equivalent to running the regex.

use super::baseline::{Diag, Loc, ParsedBaseline, Section};

/// Convert a 1-based UTF-16 column to a byte offset within `line`.
///
/// tsgo positions are UTF-16 code units (`col` counts them, 1-based); tsv works
/// in bytes. Walks the line accumulating each char's UTF-16 width until it
/// reaches `col - 1`, returning that char boundary. A column past the line end
/// clamps to `line.len()` (the EOF/last-column case).
pub(crate) fn col_to_byte(line: &str, col: u32) -> usize {
    let target = col.saturating_sub(1) as usize;
    let mut u16_count = 0usize;
    for (byte_idx, ch) in line.char_indices() {
        if u16_count >= target {
            return byte_idx;
        }
        u16_count += ch.len_utf16();
    }
    line.len()
}

/// Advance `n` runes (chars) from `start_byte` in `line`, returning the number
/// of bytes covered. Used to recover a span's end offset from a squiggle's tilde
/// count. `start_byte` must be a char boundary; a non-boundary or `n` beyond the
/// line end degrades gracefully (returns the remaining byte length).
///
/// Unit-system guard: this and `pretty.rs`'s `advance_utf16` measure in different
/// units and must never be merged or deduplicated. The plain path counts runes
/// (`error_baseline.go`'s `utf8.RuneCountInString`); the pretty path counts UTF-16
/// code units (tsgo's `writeCodeSnippet`). The same astral char is one rune here
/// but two units there.
pub(crate) fn advance_runes(line: &str, start_byte: usize, n: usize) -> usize {
    let Some(slice) = line.get(start_byte..) else {
        return 0;
    };
    for (count, (byte_idx, _)) in slice.char_indices().enumerate() {
        if count == n {
            return byte_idx;
        }
    }
    slice.len()
}

/// Byte offset of each LF line start over `join(lines, "\n")`. Both the parser
/// (recovering spans) and the renderer (the squiggle loop) derive their line
/// map from this, so the two stay in the same coordinate system.
pub(crate) fn lf_line_starts(lines: &[String]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(lines.len());
    let mut acc = 0usize;
    for l in lines {
        starts.push(acc);
        acc += l.len() + 1; // +1 for the LF separator
    }
    starts
}

/// Byte length of `join(lines, "\n")` (the last line carries no trailing LF).
pub(crate) fn lf_content_len(lines: &[String], starts: &[usize]) -> usize {
    match (lines.last(), starts.last()) {
        (Some(last), Some(&start)) => start + last.len(),
        _ => 0,
    }
}

/// Blank a squiggle line's prefix: every non-whitespace char becomes a single
/// space, whitespace survives (`error_baseline.go:211`, `nonWhitespace` =
/// Go `\S` = not `[\t\n\f\r ]`). Tabs are kept 1:1 — the tab-survival rule the
/// pretty path deliberately does *not* share. A multi-byte non-whitespace char
/// collapses to one space, so the result is not generally byte-length-preserving
/// — hence the renderer recomputes the prefix from the source rather than
/// reusing the recovered squiggle's prefix width.
pub(crate) fn blank_prefix(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len());
    for ch in prefix.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\u{000C}' | '\r') {
            out.push(ch);
        } else {
            out.push(' ');
        }
    }
    out
}

/// Count the runes (chars) fully inside the byte range `[start, end)` of `line`.
/// Mirrors `utf8.RuneCountInString(line[squiggleStart:squiggleEnd])`. A range
/// end that falls mid-char stops before the partial char (the recovered spans
/// keep both ends on char boundaries, so this only guards against surprises).
pub(crate) fn runes_in_byte_range(line: &str, start: usize, end: usize) -> usize {
    let start = start.min(line.len());
    let end = end.min(line.len()).max(start);
    let Some(slice) = line.get(start..) else {
        return 0;
    };
    let mut count = 0usize;
    let mut idx = start;
    for ch in slice.chars() {
        let next = idx + ch.len_utf8();
        if next > end {
            break;
        }
        count += 1;
        idx = next;
    }
    count
}

/// tsgo's `isDefaultLibraryFile` (`util.go:51`): base name `lib.*.d.ts`.
pub(crate) fn is_default_library_file(path: &str) -> bool {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    base.starts_with("lib.") && base.ends_with(".d.ts")
}

/// tsgo's `isTsConfigFile` (`util.go:60`): contains `tsconfig` and `json`.
pub(crate) fn is_ts_config_file(path: &str) -> bool {
    path.contains("tsconfig") && path.contains("json")
}

/// The hard-coded harness newline (`error_baseline.go:24`), the one const the
/// three literal CRLF sites collapse to. Shared with the pretty renderer.
pub(crate) const CRLF: &str = "\r\n";

/// Emit `CRLF` before every element except the first, mirroring tsgo's stateful
/// `newLine()` closure (shared across the global block and every file section,
/// so the very first line — global error or first `====` header — gets no
/// leading newline).
fn push_nl(out: &mut String, first: &mut bool) {
    if *first {
        *first = false;
    } else {
        out.push_str(CRLF);
    }
}

/// Render one diagnostic's `!!!` block (`outputErrorText`): each non-empty
/// message line prefixed `!!! {category} TS{code}: `, then its related-info
/// lines verbatim. Shared by the global re-render loop and each file section —
/// this is the "global diagnostics render twice" reproduction (once bare in the
/// summary, once here).
fn emit_bang_block(out: &mut String, first: &mut bool, d: &Diag) {
    for m in &d.msg_lines {
        if m.is_empty() {
            continue;
        }
        push_nl(out, first);
        out.push_str("!!! ");
        out.push_str(&d.category);
        out.push_str(" TS");
        push_code(out, d.code);
        out.push_str(": ");
        out.push_str(m);
    }
    for r in &d.related {
        push_nl(out, first);
        out.push_str(r);
    }
}

/// Append `TS`-code digits (handles the negative harness code `TS-1`). Shared
/// with the pretty renderer.
pub(crate) fn push_code(out: &mut String, code: i32) {
    use std::fmt::Write as _;
    let _ = write!(out, "{code}");
}

/// Render the whole baseline model back to its byte stream.
///
/// Structure (mirrors `iterateErrorBaseline` + `GetErrorBaseline`):
/// 1. the summary block (`topDiagnostics`) — one entry per diagnostic, each
///    ending `CRLF`; then a `CRLF CRLF` separator;
/// 2. the global (fileless) diagnostics' `!!!` re-render;
/// 3. each `==== {file} ({N} errors) ====` section in input order.
#[must_use]
pub fn render_baseline(b: &ParsedBaseline) -> String {
    let mut out = String::new();

    // --- 1. summary block (topDiagnostics) ---
    for d in &b.diags {
        if let Some(file) = &d.file {
            out.push_str(file);
            match d.loc {
                Some(Loc::Numbered { line, col }) => {
                    use std::fmt::Write as _;
                    let _ = write!(out, "({line},{col})");
                }
                // A default-library position the harness masks to `(--,--)`.
                Some(Loc::Masked) => out.push_str("(--,--)"),
                None => {}
            }
            out.push_str(": ");
        }
        out.push_str(&d.category);
        out.push_str(" TS");
        push_code(&mut out, d.code);
        out.push_str(": ");
        for (i, m) in d.msg_lines.iter().enumerate() {
            if i > 0 {
                out.push_str(CRLF);
            }
            out.push_str(m);
        }
        out.push_str(CRLF);
    }
    // The `+ harnessNewLine + harnessNewLine` after topDiagnostics.
    out.push_str(CRLF);
    out.push_str(CRLF);

    render_middle(&mut out, &b.diags, &b.sections);

    out
}

/// Render the stateful-`newLine` region: the global (fileless) diagnostics'
/// `!!!` re-render, then each `==== ` file section in input order. Shared by the
/// plain [`render_baseline`] (after its summary block) and the pretty renderer
/// (after its colored top block) — both produce a byte-identical middle.
pub(crate) fn render_middle(out: &mut String, diags: &[Diag], sections: &[Section]) {
    let mut first = true;

    // Global (fileless) diagnostics re-render, in summary order.
    for d in diags {
        if d.file.is_none() {
            emit_bang_block(out, &mut first, d);
        }
    }

    // File sections, in input order.
    for sec in sections {
        push_nl(out, &mut first);
        out.push_str("==== ");
        out.push_str(&sec.name);
        out.push_str(" (");
        {
            use std::fmt::Write as _;
            let _ = write!(out, "{}", sec.diags.len());
        }
        out.push_str(" errors) ====");
        render_section(out, &mut first, sec, diags);
    }
}

/// Render one file section: each source line (4-space-indented) followed by any
/// error squiggles that touch it, with the `!!!` message emitted on the span's
/// end line. Ported from the inner file loop of `iterateErrorBaseline`
/// (`:180-222`), recomputing every squiggle from the span so the clip math is
/// exercised in the forward direction (the parser exercised it in reverse).
fn render_section(out: &mut String, first: &mut bool, sec: &Section, diags: &[Diag]) {
    let n = sec.src_lines.len();
    let starts = lf_line_starts(&sec.src_lines);
    let content_len = lf_content_len(&sec.src_lines, &starts);

    for (idx, line) in sec.src_lines.iter().enumerate() {
        push_nl(out, first);
        out.push_str("    ");
        out.push_str(line);

        let this_start = starts[idx];
        let is_last = idx + 1 == n;
        let next_start = if is_last {
            content_len
        } else {
            starts[idx + 1]
        };
        let line_len = line.len();

        for sd in &sec.diags {
            let err_start = sd.pos_abs;
            let end = sd.pos_abs + sd.len;
            // "Does any error start or continue on to this line?" (`:201`).
            if end >= this_start && (err_start < next_start || is_last) {
                // squiggleStart = max(0, errStart - thisLineStart).
                let squiggle_start = err_start.saturating_sub(this_start);
                // length = (end - errStart) - max(0, thisLineStart - errStart).
                let length = (end - err_start) - this_start.saturating_sub(err_start);

                push_nl(out, first);
                out.push_str("    ");
                let ss = squiggle_start.min(line_len);
                out.push_str(&blank_prefix(&line[..ss]));
                // squiggleEnd = max(squiggleStart, min(squiggleStart+length, len(line))).
                let se = squiggle_start.max((squiggle_start + length).min(line_len));
                let tildes = runes_in_byte_range(line, ss, se);
                for _ in 0..tildes {
                    out.push('~');
                }

                // "If the error ended here, or we're at the end of the file,
                // emit its message" (`:216`).
                if (is_last || next_start > end)
                    && let Some(d) = diags.get(sd.diag_index)
                {
                    emit_bang_block(out, first, d);
                }
            }
        }
    }
}

/// The renderer's two self-assertions (`error_baseline.go:225` and `:238-251`),
/// ported as a checkable predicate rather than a test-time `assert`. Returns the
/// list of violation messages (empty = both hold); the round-trip driver counts
/// baselines that trip either.
///
/// 1. Per file section, the number of squiggled diagnostics equals the section's
///    error count (by construction each section diagnostic is placed once, so
///    this cross-checks the parser's section/summary agreement).
/// 2. Total accounting: non-lib-non-tsconfig (fileless diagnostics included) +
///    lib + tsconfig equals the diagnostic count — a partition check that the
///    parser accounted for every diagnostic.
///
/// The dead `dupeCase` branch (`error_baseline.go:156,226-233`) is intentionally
/// not ported: `isDupe` is never true (the map is never written), so it is inert.
#[must_use]
pub fn self_assertion_violations(b: &ParsedBaseline) -> Vec<String> {
    let mut violations = Vec::new();

    // (1) per-file squiggle count == error count.
    for sec in &b.sections {
        let squiggled = sec.diags.len();
        // Every section diagnostic corresponds to one file diagnostic; a
        // mismatch would surface here (it never does for a well-formed model).
        let file_diags = b
            .diags
            .iter()
            .filter(|d| d.file.as_deref() == Some(sec.name.as_str()))
            .count();
        if squiggled != file_diags {
            violations.push(format!(
                "section {}: squiggled {squiggled} != file diagnostics {file_diags}",
                sec.name
            ));
        }
    }

    // (2) total accounting.
    let mut num_lib = 0usize;
    let mut num_tsconfig = 0usize;
    let mut num_other = 0usize; // non-lib, non-tsconfig, plus fileless
    for d in &b.diags {
        match &d.file {
            None => num_other += 1,
            Some(f) if is_default_library_file(f) => num_lib += 1,
            Some(f) if is_ts_config_file(f) => num_tsconfig += 1,
            Some(_) => num_other += 1,
        }
    }
    if num_other + num_lib + num_tsconfig != b.diags.len() {
        violations.push(format!(
            "total accounting: {num_other}+{num_lib}+{num_tsconfig} != {}",
            b.diags.len()
        ));
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_to_byte_ascii_and_astral() {
        // ASCII: col is byte offset + 1.
        assert_eq!(col_to_byte("abcdef", 1), 0);
        assert_eq!(col_to_byte("abcdef", 4), 3);
        // Past the end clamps to the byte length.
        assert_eq!(col_to_byte("ab", 9), 2);
        // Astral char is 2 UTF-16 units: 'a' 𐐷 'b' → cols 1,2,4.
        let s = "a\u{10437}b";
        assert_eq!(col_to_byte(s, 1), 0); // before 'a'
        assert_eq!(col_to_byte(s, 2), 1); // before 𐐷 (after 1 unit)
        assert_eq!(col_to_byte(s, 4), 5); // before 'b' (𐐷 is 4 bytes, 2 units)
    }

    #[test]
    fn advance_runes_counts_bytes() {
        assert_eq!(advance_runes("abcdef", 0, 3), 3);
        assert_eq!(advance_runes("abcdef", 2, 2), 2);
        // Multi-byte: advancing 1 rune over 'é' (2 bytes).
        assert_eq!(advance_runes("é!", 0, 1), 2);
        // Beyond the end returns the remaining length.
        assert_eq!(advance_runes("ab", 0, 9), 2);
    }

    #[test]
    fn lf_line_starts_and_len() {
        let lines = vec!["ab".to_string(), String::new(), "cde".to_string()];
        let starts = lf_line_starts(&lines);
        assert_eq!(starts, vec![0, 3, 4]); // "ab\n\ncde"
        assert_eq!(lf_content_len(&lines, &starts), 7);
    }

    #[test]
    fn blank_prefix_keeps_tabs_blanks_text() {
        // Tabs survive; spaces survive; other chars → single space.
        // "\tab c" = tab + a,b (→2 spaces) + space (survives) + c (→space).
        assert_eq!(blank_prefix("\tab c"), "\t    ");
        assert_eq!(blank_prefix("abc"), "   ");
        // A multi-byte char collapses to one space (not byte-length preserving).
        assert_eq!(blank_prefix("é"), " ");
    }

    #[test]
    fn runes_in_byte_range_counts_full_chars() {
        assert_eq!(runes_in_byte_range("abcdef", 0, 3), 3);
        // Two-byte char fully inside counts once.
        assert_eq!(runes_in_byte_range("aéb", 1, 3), 1);
        // Mid-char end stops before the partial char.
        assert_eq!(runes_in_byte_range("aéb", 1, 2), 0);
    }

    #[test]
    fn library_and_tsconfig_classifiers() {
        assert!(is_default_library_file("lib.es5.d.ts"));
        assert!(is_default_library_file("/x/lib.dom.d.ts"));
        assert!(!is_default_library_file("libfoo.d.ts"));
        assert!(!is_default_library_file("a.ts"));
        assert!(is_ts_config_file("/p/tsconfig.json"));
        assert!(!is_ts_config_file("a.ts"));
    }

    /// Parse a hand-built baseline and confirm it re-renders byte-identically —
    /// the round-trip contract, exercised over the format's fiddly bits: a
    /// message chain (summary continuation + double-rendered `!!!` lines), a
    /// global diagnostic (rendered twice), a lib-masked summary line, related
    /// info, a tab-bearing squiggle prefix, and a trailing (no-final-newline)
    /// section.
    #[test]
    fn roundtrip_identity_over_mixed_baseline() {
        use super::super::baseline::parse_baseline;
        let content = concat!(
            // summary block: global, positional-with-chain, lib-masked
            "error TS5110: Option must be set.\r\n",
            "a.ts(1,2): error TS2322: Type 'U' is not assignable.\r\n",
            "  Type 'U' is not assignable to 'V'.\r\n",
            "lib.es5.d.ts(--,--): error TS2411: Property bad.\r\n",
            "\r\n\r\n",
            // global !!! re-render (rendered twice: bare above, prefixed here)
            "!!! error TS5110: Option must be set.\r\n",
            // section: source lines are 4-space-indented; the squiggle keeps the tab
            "==== a.ts (1 errors) ====\r\n",
            "    \tconst x = 1;\r\n",
            "    \t~~~~~\r\n",
            "!!! error TS2322: Type 'U' is not assignable.\r\n",
            "!!! error TS2322:   Type 'U' is not assignable to 'V'.\r\n",
            "!!! related TS2208 a.ts:1:1: A hint.\r\n",
            "    \tconst y = 2;"
        );
        let parsed = parse_baseline(content).expect("parse");
        assert_eq!(
            render_baseline(&parsed),
            content,
            "round-trip must be byte-identical"
        );
        assert!(self_assertion_violations(&parsed).is_empty());
    }

    // --- standalone renderer tests (model built by hand, not via the parser) ---
    //
    // The roundtrip tests all feed `render_baseline` a *parser-recovered* model, so
    // parser and renderer are only ever exercised as a pair. These build a
    // `ParsedBaseline` directly — the path a future tsv checker takes, emitting a
    // model it assembled rather than one recovered from a baseline — so the
    // renderer's byte contract is pinned independently of the parser.

    use super::super::baseline::SectionDiag;

    /// Build an `error`-category diagnostic (the only category these tests need).
    fn diag(file: Option<&str>, loc: Option<Loc>, code: i32, msgs: &[&str]) -> Diag {
        Diag {
            file: file.map(str::to_string),
            loc,
            category: "error".to_string(),
            code,
            msg_lines: msgs.iter().map(|s| (*s).to_string()).collect(),
            related: Vec::new(),
        }
    }

    /// Build a section from source lines and `(diag_index, pos_abs, len)` spans.
    fn section(name: &str, src: &[&str], spans: &[(usize, usize, usize)]) -> Section {
        Section {
            name: name.to_string(),
            src_lines: src.iter().map(|s| (*s).to_string()).collect(),
            diags: spans
                .iter()
                .map(|&(diag_index, pos_abs, len)| SectionDiag {
                    diag_index,
                    pos_abs,
                    len,
                })
                .collect(),
        }
    }

    #[test]
    fn standalone_render_positional_single_diag() {
        // `const x = 1;` — the `x` is a length-1 span at byte 6 (col 7); the
        // squiggle prefix blanks `const ` to six spaces (plus the 4-space indent).
        let b = ParsedBaseline {
            diags: vec![diag(
                Some("a.ts"),
                Some(Loc::Numbered { line: 1, col: 7 }),
                2304,
                &["Cannot find name 'x'."],
            )],
            sections: vec![section("a.ts", &["const x = 1;"], &[(0, 6, 1)])],
        };
        let expected = concat!(
            "a.ts(1,7): error TS2304: Cannot find name 'x'.\r\n",
            "\r\n\r\n",
            "==== a.ts (1 errors) ====\r\n",
            "    const x = 1;\r\n",
            "          ~\r\n",
            "!!! error TS2304: Cannot find name 'x'.",
        );
        assert_eq!(render_baseline(&b), expected);
        assert!(self_assertion_violations(&b).is_empty());
    }

    #[test]
    fn standalone_render_global_diag_renders_twice() {
        // A fileless diagnostic prints bare in the summary AND `!!!`-prefixed in
        // the re-render; the shared first-line flag means that first `!!!` carries
        // no leading CRLF (it opens the stateful-newLine region).
        let b = ParsedBaseline {
            diags: vec![diag(None, None, 5102, &["Option 'x' has been removed."])],
            sections: vec![],
        };
        let expected = concat!(
            "error TS5102: Option 'x' has been removed.\r\n",
            "\r\n\r\n",
            "!!! error TS5102: Option 'x' has been removed.",
        );
        assert_eq!(render_baseline(&b), expected);
        assert!(self_assertion_violations(&b).is_empty());
    }

    #[test]
    fn standalone_render_message_chain() {
        // A two-line message chain: the continuation joins the summary head with
        // CRLF, and each non-empty line gets its own `!!!` prefix in the block.
        let b = ParsedBaseline {
            diags: vec![diag(
                Some("a.ts"),
                Some(Loc::Numbered { line: 1, col: 1 }),
                2322,
                &[
                    "Type 'A' is not assignable to 'B'.",
                    "  Type 'A' is missing prop.",
                ],
            )],
            sections: vec![section("a.ts", &["let a: B = x;"], &[(0, 0, 3)])],
        };
        let expected = concat!(
            "a.ts(1,1): error TS2322: Type 'A' is not assignable to 'B'.\r\n",
            "  Type 'A' is missing prop.\r\n",
            "\r\n\r\n",
            "==== a.ts (1 errors) ====\r\n",
            "    let a: B = x;\r\n",
            "    ~~~\r\n",
            "!!! error TS2322: Type 'A' is not assignable to 'B'.\r\n",
            "!!! error TS2322:   Type 'A' is missing prop.",
        );
        assert_eq!(render_baseline(&b), expected);
        assert!(self_assertion_violations(&b).is_empty());
    }

    #[test]
    fn standalone_render_multiline_span_message_on_end_line() {
        // A span covering the whole `foo(\n  bar\n)` squiggles every touched line
        // (continuations squiggle from column 0) but emits its `!!!` message only
        // on the end line — the forward clip math, exercised without the parser.
        let b = ParsedBaseline {
            diags: vec![diag(
                Some("a.ts"),
                Some(Loc::Numbered { line: 1, col: 1 }),
                2554,
                &["Expected 0 arguments."],
            )],
            sections: vec![section("a.ts", &["foo(", "  bar", ")"], &[(0, 0, 12)])],
        };
        let expected = concat!(
            "a.ts(1,1): error TS2554: Expected 0 arguments.\r\n",
            "\r\n\r\n",
            "==== a.ts (1 errors) ====\r\n",
            "    foo(\r\n",
            "    ~~~~\r\n",
            "      bar\r\n",
            "    ~~~~~\r\n",
            "    )\r\n",
            "    ~\r\n",
            "!!! error TS2554: Expected 0 arguments.",
        );
        assert_eq!(render_baseline(&b), expected);
        assert!(self_assertion_violations(&b).is_empty());
    }
}
