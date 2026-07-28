//! Parse the summary block of a tsgo `.errors.txt` baseline.
//!
//! A baseline file has a leading **summary block** — one line per diagnostic,
//! canonically sorted — followed by per-file sections that reprint the source
//! with `!!!`-prefixed diagnostic bodies:
//!
//! ```text
//! <file>(<line>,<col>): error TS<code>: <message>   ← positional summary line
//! error TS<code>: <message>                          ← global (fileless) summary line
//! <blank>
//! <blank>
//! !!! error TS<code>: <message>                      ← global re-render (skip)
//! ==== <file> (<N> errors) ====                      ← first per-file section
//!     <source line>                                  ← 4-space-indented, verbatim
//! ```
//!
//! The counting rule (one diagnostic **instance** per summary line): read only
//! the lines **before** the first `==== ` header, skipping blank lines and any
//! `!!!` line (the global re-render, which would otherwise double-count). Source
//! lines can contain the literal text `error TS`, so the scan must never enter a
//! `==== ` section — hence the hard stop at the first header.
//!
//! The full-baseline model ([`ParsedBaseline`]) and its parser
//! ([`parse_baseline`]) below extend that seed into the round-trip surface: a
//! structured recovery of the summary diagnostics, the global `!!!` re-render,
//! and every `==== ` file section (source + recovered spans), rendered back by
//! [`super::render`] and byte-compared by [`super::roundtrip`].

use super::render::{advance_runes, col_to_byte, lf_line_starts};

/// One diagnostic parsed from a baseline's summary block.
///
/// A line **with** the `(<line>,<col>)` prefix is *positional* (`file`/`line`/
/// `col` are `Some`); a line **without** it is a *global* (fileless) diagnostic
/// (all three are `None`). `code` is the numeric `TS<code>` in both cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryDiagnostic {
    /// The source file the diagnostic points at, or `None` for a global
    /// (fileless) diagnostic. Extracted verbatim from the summary line.
    pub file: Option<String>,
    /// 1-based line number, or `None` for a global diagnostic.
    pub line: Option<u32>,
    /// 1-based column number, or `None` for a global diagnostic.
    pub col: Option<u32>,
    /// The `TS<code>` number (e.g. `2454`).
    pub code: u32,
}

/// Parse a baseline file's summary block into its diagnostic instances.
///
/// Handles CRLF baselines: [`str::lines`] strips the trailing `\r`. The scan
/// stops at the first `==== ` section header so source text is never parsed.
pub fn parse_summary_block(content: &str) -> Vec<SummaryDiagnostic> {
    let mut diagnostics = Vec::new();

    for line in content.lines() {
        // The summary block ends at the first per-file section header.
        if line.starts_with("==== ") {
            break;
        }
        // Skip blank lines and the `!!!` global re-render (the bare summary line
        // above it is the one instance we count).
        if line.trim().is_empty() || line.starts_with("!!!") {
            continue;
        }
        if let Some(diag) = parse_summary_line(line) {
            diagnostics.push(diag);
        }
    }

    diagnostics
}

/// Parse one summary-block line, or `None` if it is not a diagnostic line.
///
/// Two shapes:
/// - global: `error TS<code>: <message>`
/// - positional: `<file>(<line>,<col>): error TS<code>: <message>`
fn parse_summary_line(line: &str) -> Option<SummaryDiagnostic> {
    // Global (fileless) diagnostic: `error TS<code>: <message>`.
    if let Some(rest) = line.strip_prefix("error TS") {
        let code = leading_code(rest)?;
        return Some(SummaryDiagnostic {
            file: None,
            line: None,
            col: None,
            code,
        });
    }

    // Positional diagnostic: the `): error TS` marker separates the
    // `<file>(<line>,<col>)` prefix from the code. Filenames don't contain this
    // marker, so the first occurrence is the boundary.
    let marker = "): error TS";
    let idx = line.find(marker)?;
    let code = leading_code(&line[idx + marker.len()..])?;

    let head = &line[..idx]; // `<file>(<line>,<col>`
    let open = head.rfind('(')?;
    let file = &head[..open];
    let (l, c) = head[open + 1..].split_once(',')?; // `<line>,<col>`
    let line_no: u32 = l.parse().ok()?;
    let col_no: u32 = c.parse().ok()?;

    Some(SummaryDiagnostic {
        file: Some(file.to_string()),
        line: Some(line_no),
        col: Some(col_no),
        code,
    })
}

/// Read the leading run of ASCII digits from `s`, requiring a `:` terminator
/// (the `error TS<digits>:` shape). Returns `None` if `s` doesn't start with a
/// digit or the digits aren't followed by `:`.
fn leading_code(s: &str) -> Option<u32> {
    let end = s.find(|ch: char| !ch.is_ascii_digit())?;
    if end == 0 {
        return None; // no leading digits
    }
    if s.as_bytes().get(end) != Some(&b':') {
        return None; // digits not terminated by ':'
    }
    s[..end].parse().ok()
}

// ===========================================================================
// Full-baseline model + parser (the round-trip surface)
// ===========================================================================

/// A summary-line location token: `(<line>,<col>)`, or the masked default-
/// library form `(--,--)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loc {
    /// `(<line>,<col>)` — 1-based line and UTF-16 column.
    Numbered {
        /// 1-based line number.
        line: u32,
        /// 1-based UTF-16 column.
        col: u32,
    },
    /// `(--,--)` — a default-library position the harness masks.
    Masked,
}

/// One recovered diagnostic (its summary entry plus, where present, the related
/// info from its `!!!` re-render).
#[derive(Debug, Clone)]
pub struct Diag {
    /// Display path the diagnostic points at, or `None` for a global (fileless)
    /// diagnostic.
    pub file: Option<String>,
    /// The summary location token; `None` for a global diagnostic.
    pub loc: Option<Loc>,
    /// The diagnostic category (`error`, `warning`, `message`, `suggestion`).
    pub category: String,
    /// The `TS<code>` number (can be negative, e.g. the harness's `TS-1`).
    pub code: i32,
    /// The flattened message as physical lines (main text, then each message-
    /// chain line with its 2-space-per-level indent preserved).
    pub msg_lines: Vec<String>,
    /// The diagnostic's `!!! related …` lines, stored verbatim (including any
    /// rare chain-continuation lines), re-emitted as-is.
    pub related: Vec<String>,
}

/// A diagnostic squiggled inside a file section, with its span recovered in the
/// section's LF-content byte coordinate.
#[derive(Debug, Clone)]
pub struct SectionDiag {
    /// Index into [`ParsedBaseline::diags`].
    pub diag_index: usize,
    /// Byte offset of the span start in the LF-joined section content.
    pub pos_abs: usize,
    /// Byte length of the span.
    pub len: usize,
}

/// A recovered `==== ` file section: the reprinted source and the diagnostics
/// squiggled in it (in canonical / `fileErrors` order).
#[derive(Debug, Clone)]
pub struct Section {
    /// The section's display file name (from the `==== <name> …` header).
    pub name: String,
    /// The de-indented source lines (the LF-content view the spans index into).
    pub src_lines: Vec<String>,
    /// This file's diagnostics with recovered spans.
    pub diags: Vec<SectionDiag>,
}

/// A fully parsed baseline: every diagnostic (summary order) plus every file
/// section (input order). [`super::render::render_baseline`] turns it back into
/// the original byte stream.
#[derive(Debug, Clone)]
pub struct ParsedBaseline {
    /// All diagnostics, in the baseline's (already-sorted) summary order.
    pub diags: Vec<Diag>,
    /// The `==== ` file sections, in input order.
    pub sections: Vec<Section>,
}

/// The diagnostic categories that can head a summary line. Shared with the
/// pretty head parser.
pub(super) const CATEGORIES: [&str; 4] = ["error", "warning", "message", "suggestion"];

/// Parse a whole `.errors.txt` baseline into its [`ParsedBaseline`] model.
///
/// The baseline is split on `CRLF` (`str::split("\r\n")`), which is an exact
/// inverse of `join("\r\n")` — so round-tripping the model is a byte question.
/// The layout parsed:
///
/// 1. the **summary block** — physical lines until the first blank line; a line
///    with a leading space is a message-chain continuation of the previous
///    diagnostic, else a new diagnostic head;
/// 2. two blank lines;
/// 3. the **global `!!!` re-render** — for each global (fileless) diagnostic in
///    order, its message lines then its related lines;
/// 4. the **`==== ` sections** — each parsed by [`parse_section_body`], which
///    recovers the source and each diagnostic's span.
///
/// # Errors
///
/// Returns a short reason string on any structural surprise (an unparsable
/// summary line, a missing blank separator, a section-body desync, an
/// unrecoverable span). The round-trip driver buckets these; a returned `Err`
/// is a parse failure, a rendered-bytes mismatch is a render failure.
pub fn parse_baseline(content: &str) -> Result<ParsedBaseline, String> {
    let lines: Vec<&str> = content.split("\r\n").collect();

    // --- 1. summary block ---
    let mut diags: Vec<Diag> = Vec::new();
    let mut i = 0usize;
    while i < lines.len() && !lines[i].is_empty() {
        let line = lines[i];
        if line.starts_with(' ') {
            let d = diags
                .last_mut()
                .ok_or("chain continuation before any diagnostic")?;
            d.msg_lines.push(line.to_string());
        } else {
            let head = parse_summary_head(line)
                .ok_or_else(|| format!("unparsable summary line: {line:?}"))?;
            diags.push(Diag {
                file: head.file,
                loc: head.loc,
                category: head.category,
                code: head.code,
                msg_lines: vec![head.first_msg],
                related: Vec::new(),
            });
        }
        i += 1;
    }
    if diags.is_empty() {
        return Err("empty summary block".to_string());
    }
    let summary_end = i;

    // --- 2. the two blank lines ---
    if lines.get(summary_end) != Some(&"") || lines.get(summary_end + 1) != Some(&"") {
        return Err("expected two blank lines after summary block".to_string());
    }

    // --- 3 & 4: the global `!!!` re-render and `==== ` sections ---
    let sections = parse_middle(&lines, summary_end + 2, lines.len(), &mut diags)?;

    Ok(ParsedBaseline { diags, sections })
}

/// Parse the middle region — the global (fileless) `!!!` re-render followed by
/// the `==== ` file sections — from `lines[start..end]`, filling each
/// diagnostic's `related` and recovering every section's source and spans.
///
/// Shared by the plain [`parse_baseline`] (which reaches it after the summary
/// block, with `end == lines.len()`) and the pretty parser (which reaches it
/// after the colored top block, with `end` bounded at the summary trailer). The
/// diagnostics' heads (file/loc/category/code/message) must already be present
/// in `diags`, in canonical summary order; this recovers only the middle.
///
/// # Errors
///
/// Returns a short reason on a `!!!` underflow, a bad section header, or a
/// section-body desync (the same buckets the round-trip driver reports).
pub(super) fn parse_middle(
    lines: &[&str],
    start: usize,
    end: usize,
    diags: &mut [Diag],
) -> Result<Vec<Section>, String> {
    let mut i = start;

    // --- 3. global (fileless) `!!!` re-render ---
    for d in diags.iter_mut() {
        if d.file.is_some() {
            continue;
        }
        let msg_count = d.msg_lines.len();
        for _ in 0..msg_count {
            match lines.get(i) {
                Some(l) if l.starts_with("!!! ") => i += 1,
                _ => return Err("global !!! message underflow".to_string()),
            }
        }
        d.related = collect_related(lines, &mut i);
    }

    // --- 4. `==== ` sections ---
    let mut sections = Vec::new();
    while i < end {
        let header = lines[i];
        let (name, _n_errors) = parse_section_header(header)
            .ok_or_else(|| format!("bad section header: {header:?}"))?;
        i += 1;
        let body_start = i;
        while i < end && !lines[i].starts_with("==== ") {
            i += 1;
        }
        let body = &lines[body_start..i];
        let section = parse_section_body(name, body, diags)?;
        sections.push(section);
    }

    Ok(sections)
}

/// Collect a diagnostic's related-info block from `lines` starting at `*i`: each
/// `!!! related …` line, plus the raw continuation lines of that related info's
/// **own** message chain.
///
/// A related info can itself be a message chain; `FlattenDiagnosticMessage`
/// renders its first line onto the `!!! related …:` line and its deeper levels as
/// bare continuation lines — 2-space-per-level indent, **no** `!!!` prefix and no
/// 4-space section indent (`diagnosticwriter.go`'s chain flattening). Those lines
/// are re-emitted verbatim by the renderer, so recovering them is a capture
/// question. The chain opens at indent 2 (level 1) and increases by exactly 2 per
/// level, which disambiguates it from a section source line — every source line
/// carries the ≥4-space section indent, so a line at the next expected chain
/// indent that also holds content is a continuation. The strict `+2` run stops the
/// moment a line breaks the pattern (e.g. the next source line, `!!!` block, or a
/// blank line), so a source line can't be swallowed unless it sits at exactly the
/// pending chain indent *and* the chain didn't already terminate — a case the
/// round-trip over every baseline confirms does not arise.
fn collect_related(lines: &[&str], i: &mut usize) -> Vec<String> {
    let mut related = Vec::new();
    while lines.get(*i).is_some_and(|l| l.starts_with("!!! related ")) {
        related.push(lines[*i].to_string());
        *i += 1;
        // The related info's own message-chain continuation lines.
        let mut expected = 2usize;
        while let Some(cont) = lines.get(*i) {
            if cont.starts_with("!!! ") {
                break;
            }
            let indent = cont.bytes().take_while(|&b| b == b' ').count();
            if indent == expected && cont.len() > indent {
                related.push((*cont).to_string());
                *i += 1;
                expected += 2;
            } else {
                break;
            }
        }
    }
    related
}

/// The head fields of a summary line, before message text.
struct SummaryHead {
    file: Option<String>,
    loc: Option<Loc>,
    category: String,
    code: i32,
    first_msg: String,
}

/// Parse one summary-line head, or `None` for a chain continuation / non-head.
///
/// Global: `{category} TS{code}: {msg}`. Positional:
/// `{file}({loc}): {category} TS{code}: {msg}` — located by the earliest
/// `): {category} TS` marker across categories (a filename never contains it, so
/// the first is the loc boundary).
fn parse_summary_head(line: &str) -> Option<SummaryHead> {
    // Global head: `{category} TS…` at the very start.
    for cat in CATEGORIES {
        let prefix = format!("{cat} TS");
        if let Some(rest) = line.strip_prefix(&prefix) {
            let (code, consumed) = read_code(rest)?;
            let msg = rest.get(consumed..)?.strip_prefix(": ")?;
            return Some(SummaryHead {
                file: None,
                loc: None,
                category: cat.to_string(),
                code,
                first_msg: msg.to_string(),
            });
        }
    }

    // Positional head: earliest `): {category} TS` marker.
    let mut best: Option<(usize, &str)> = None;
    for cat in CATEGORIES {
        let marker = format!("): {cat} TS");
        if let Some(idx) = line.find(&marker)
            && best.is_none_or(|(b, _)| idx < b)
        {
            best = Some((idx, cat));
        }
    }
    let (idx, cat) = best?;
    let head = line.get(..idx)?; // `{file}({loc}`
    let open = head.rfind('(')?;
    let file = head.get(..open)?.to_string();
    let loc = parse_loc(head.get(open + 1..)?)?;

    let marker_len = "): ".len() + cat.len() + " TS".len();
    let rest = line.get(idx + marker_len..)?;
    let (code, consumed) = read_code(rest)?;
    let msg = rest.get(consumed..)?.strip_prefix(": ")?;
    Some(SummaryHead {
        file: Some(file),
        loc: Some(loc),
        category: cat.to_string(),
        code,
        first_msg: msg.to_string(),
    })
}

/// Read a `-?\d+` code from the start of `s`, returning `(code, bytes_consumed)`.
/// Shared with the pretty head parser.
pub(super) fn read_code(s: &str) -> Option<(i32, usize)> {
    let bytes = s.as_bytes();
    let mut i = usize::from(bytes.first() == Some(&b'-'));
    let digits_start = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if i == digits_start {
        return None;
    }
    let code: i32 = s.get(..i)?.parse().ok()?;
    Some((code, i))
}

/// Parse a location token's inner text (`<line>,<col>` or `--,--`).
fn parse_loc(inner: &str) -> Option<Loc> {
    if inner == "--,--" {
        return Some(Loc::Masked);
    }
    let (l, c) = inner.split_once(',')?;
    Some(Loc::Numbered {
        line: l.parse().ok()?,
        col: c.parse().ok()?,
    })
}

/// Parse `==== {name} ({N} errors) ====` into `(name, N)`.
fn parse_section_header(line: &str) -> Option<(String, usize)> {
    let inner = line.strip_prefix("==== ")?.strip_suffix(" ====")?;
    // `{name} ({N} errors)` — split at the last ` (` (a filename may contain
    // one, so the last is the error-count group).
    let open = inner.rfind(" (")?;
    let name = inner.get(..open)?.to_string();
    let count = inner.get(open + 2..)?.strip_suffix(" errors)")?;
    Some((name, count.parse().ok()?))
}

/// In-flight span-recovery state for one section diagnostic.
struct Work {
    diag_index: usize,
    start_line0: usize,
    col: u32,
    ended: bool,
    end_line0: usize,
    end_tildes: usize,
}

/// Parse a `==== ` section body, recovering its source lines and each
/// diagnostic's span.
///
/// Drives the same structure `iterateErrorBaseline`'s inner loop generates, in
/// reverse: after each source line, exactly the diagnostics **touching** it
/// (spans already open, plus spans starting on this line) emit a squiggle, in
/// `Pos` order; a diagnostic's `!!!` block follows its squiggle iff it ends on
/// this line (detected by the next line being `!!! …`). Because the touching set
/// and its order are known from the summary, the source/squiggle boundary is
/// recovered without content heuristics. The end line's tilde count then gives
/// the span's end offset, closing the clip math in reverse.
fn parse_section_body(name: String, body: &[&str], diags: &mut [Diag]) -> Result<Section, String> {
    // This file's diagnostics, in summary (== fileErrors) order.
    let mut works: Vec<Work> = Vec::new();
    for (idx, d) in diags.iter().enumerate() {
        if d.file.as_deref() == Some(name.as_str()) {
            match d.loc {
                Some(Loc::Numbered { line, col }) => works.push(Work {
                    diag_index: idx,
                    start_line0: (line as usize).saturating_sub(1),
                    col,
                    ended: false,
                    end_line0: 0,
                    end_tildes: 0,
                }),
                _ => {
                    return Err(format!(
                        "section {name}: diagnostic has no numbered location"
                    ));
                }
            }
        }
    }

    let mut src_lines: Vec<String> = Vec::new();
    let mut active: Vec<usize> = Vec::new(); // indices into `works`, spans in progress
    let mut bi = 0usize;
    let mut src_idx = 0usize;

    while bi < body.len() {
        // Consume the source line (always 4-space-indented).
        let src = body[bi].strip_prefix("    ").ok_or_else(|| {
            format!(
                "section {name}: source line missing 4-space indent: {:?}",
                body[bi]
            )
        })?;
        src_lines.push(src.to_string());
        bi += 1;

        // touching = open spans (Pos order) then spans starting on this line.
        let starting: Vec<usize> = (0..works.len())
            .filter(|&w| !works[w].ended && !active.contains(&w) && works[w].start_line0 == src_idx)
            .collect();
        let touching: Vec<usize> = active.iter().copied().chain(starting).collect();

        let mut newly_active: Vec<usize> = Vec::new();
        for w in touching {
            let sq = body
                .get(bi)
                .ok_or_else(|| format!("section {name}: squiggle underflow"))?;
            let sq_body = sq.strip_prefix("    ").ok_or_else(|| {
                format!("section {name}: squiggle missing 4-space indent: {sq:?}")
            })?;
            bi += 1;
            let tildes = sq_body.bytes().filter(|&b| b == b'~').count();

            // A diagnostic ends on this line iff its `!!!` block follows its
            // squiggle (the renderer emits them adjacently).
            if body.get(bi).is_some_and(|l| l.starts_with("!!! ")) {
                works[w].ended = true;
                works[w].end_line0 = src_idx;
                works[w].end_tildes = tildes;

                // Consume the `!!!` block: message lines then related lines.
                let msg_count = diags[works[w].diag_index].msg_lines.len();
                for _ in 0..msg_count {
                    match body.get(bi) {
                        Some(l) if l.starts_with("!!! ") => bi += 1,
                        _ => return Err(format!("section {name}: !!! message underflow")),
                    }
                }
                diags[works[w].diag_index].related = collect_related(body, &mut bi);
            } else if !active.contains(&w) {
                newly_active.push(w);
            }
        }

        active.retain(|&w| !works[w].ended);
        active.extend(newly_active);
        src_idx += 1;
    }

    if works.iter().any(|w| !w.ended) {
        return Err(format!("section {name}: unclosed diagnostic(s)"));
    }

    // Recover byte spans in the LF-content coordinate.
    let starts = lf_line_starts(&src_lines);
    let mut section_diags = Vec::with_capacity(works.len());
    for w in &works {
        let start_line = src_lines
            .get(w.start_line0)
            .ok_or_else(|| format!("section {name}: start line out of range"))?;
        let start_byte = col_to_byte(start_line, w.col);
        let pos_abs = starts[w.start_line0] + start_byte;

        let end_line = src_lines
            .get(w.end_line0)
            .ok_or_else(|| format!("section {name}: end line out of range"))?;
        // On the span's start line the squiggle begins at the column; on a
        // continuation (multi-line) end line it begins at 0.
        let sq_start = if w.end_line0 == w.start_line0 {
            start_byte
        } else {
            0
        };
        let end_abs =
            starts[w.end_line0] + sq_start + advance_runes(end_line, sq_start, w.end_tildes);
        let len = end_abs.saturating_sub(pos_abs);

        section_diags.push(SectionDiag {
            diag_index: w.diag_index,
            pos_abs,
            len,
        });
    }

    Ok(Section {
        name,
        src_lines,
        diags: section_diags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positional_and_stops_at_section() {
        // CRLF, exactly as the baselines are stored. The `!!!` body inside the
        // section must not be reached (we stop at `==== `).
        let content = "foo.ts(3,1): error TS1206: Decorators are not valid here.\r\n\
                       \r\n\r\n\
                       ==== foo.ts (1 errors) ====\r\n\
                       \t@dec\r\n\
                       \t~\r\n\
                       !!! error TS1206: Decorators are not valid here.\r\n";
        let diags = parse_summary_block(content);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0],
            SummaryDiagnostic {
                file: Some("foo.ts".to_string()),
                line: Some(3),
                col: Some(1),
                code: 1206,
            }
        );
    }

    #[test]
    fn global_diagnostic_counted_once_not_from_bang_render() {
        // A bare global line plus its `!!!` re-render, both before the first
        // `====`. The re-render must not double-count.
        let content = "error TS5102: Option 'downlevelIteration' has been removed.\r\n\
                       \r\n\r\n\
                       !!! error TS5102: Option 'downlevelIteration' has been removed.\r\n\
                       ==== a.ts (0 errors) ====\r\n\
                       \tvar a: any;\r\n";
        let diags = parse_summary_block(content);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, 5102);
        assert!(diags[0].file.is_none());
        assert!(diags[0].line.is_none());
        assert!(diags[0].col.is_none());
    }

    #[test]
    fn source_lines_after_section_are_not_parsed() {
        // A source line inside a `====` section literally contains `error TS`;
        // it must be ignored because the scan stops at the section header.
        let content = "a.ts(1,1): error TS2304: Cannot find name 'x'.\r\n\
                       \r\n\r\n\
                       ==== a.ts (1 errors) ====\r\n\
                       \t// error TS9999: not a real diagnostic\r\n";
        let diags = parse_summary_block(content);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, 2304);
    }

    #[test]
    fn multiple_summary_lines_split_positional_and_global() {
        let content = "a.ts(1,1): error TS2304: Cannot find name 'x'.\r\n\
                       error TS5102: Option 'downlevelIteration' has been removed.\r\n\
                       a.ts(2,5): error TS2454: Variable 'y' is used before being assigned.\r\n\
                       ==== a.ts (2 errors) ====\r\n";
        let diags = parse_summary_block(content);
        assert_eq!(diags.len(), 3);
        assert_eq!(diags.iter().filter(|d| d.file.is_some()).count(), 2);
        assert_eq!(diags.iter().filter(|d| d.file.is_none()).count(), 1);
        assert_eq!(diags[1].code, 5102);
    }

    #[test]
    fn summary_head_positional_global_masked_negative() {
        let pos = parse_summary_head("foo.ts(3,1): error TS1206: Decorators are not valid here.")
            .expect("positional");
        assert_eq!(pos.file.as_deref(), Some("foo.ts"));
        assert_eq!(pos.loc, Some(Loc::Numbered { line: 3, col: 1 }));
        assert_eq!(pos.category, "error");
        assert_eq!(pos.code, 1206);
        assert_eq!(pos.first_msg, "Decorators are not valid here.");

        let global = parse_summary_head("error TS5102: Option removed.").expect("global");
        assert!(global.file.is_none() && global.loc.is_none() && global.code == 5102);

        let masked = parse_summary_head("lib.es5.d.ts(--,--): error TS2411: bad.").expect("masked");
        assert_eq!(masked.file.as_deref(), Some("lib.es5.d.ts"));
        assert_eq!(masked.loc, Some(Loc::Masked));

        // Negative harness code TS-1.
        let neg = parse_summary_head("error TS-1: Pre-emit mismatch!").expect("negative");
        assert_eq!(neg.code, -1);

        // Non-error categories.
        let sugg = parse_summary_head("a.ts(1,1): suggestion TS6133: unused.").expect("suggestion");
        assert_eq!(sugg.category, "suggestion");
    }

    #[test]
    fn summary_head_rejects_continuation() {
        // A leading-space chain-continuation line is not a head.
        assert!(parse_summary_head("  Type 'U' is not assignable.").is_none());
    }

    #[test]
    fn section_header_parses_name_and_count() {
        assert_eq!(
            parse_section_header("==== foo.ts (2 errors) ===="),
            Some(("foo.ts".to_string(), 2))
        );
        // A filename containing " (" keeps its parens; the last group is the count.
        assert_eq!(
            parse_section_header("==== a (b).ts (0 errors) ===="),
            Some(("a (b).ts".to_string(), 0))
        );
        assert_eq!(parse_section_header("not a header"), None);
    }

    #[test]
    fn parse_baseline_recovers_summary_and_section_span() {
        // A single positional diagnostic; the squiggle recovers a length-4 span.
        let content = "a.ts(1,1): error TS2304: Cannot find name 'test'.\r\n\
                       \r\n\r\n\
                       ==== a.ts (1 errors) ====\r\n\
                       \x20\x20\x20\x20test;\r\n\
                       \x20\x20\x20\x20~~~~\r\n\
                       !!! error TS2304: Cannot find name 'test'.";
        let parsed = parse_baseline(content).expect("parse");
        assert_eq!(parsed.diags.len(), 1);
        assert_eq!(parsed.sections.len(), 1);
        let sec = &parsed.sections[0];
        assert_eq!(sec.name, "a.ts");
        assert_eq!(sec.src_lines, vec!["test;".to_string()]);
        assert_eq!(sec.diags.len(), 1);
        // Start at byte 0 (col 1), 4 tildes → length 4.
        assert_eq!(sec.diags[0].pos_abs, 0);
        assert_eq!(sec.diags[0].len, 4);
    }

    #[test]
    fn parse_baseline_recovers_nested_related_message_chain() {
        // A `!!! related` diagnostic that is itself a message chain: its deeper
        // levels render as bare 2/4/6-space continuation lines (no `!!!` prefix,
        // no section indent). The parser must capture them into `related` so the
        // baseline round-trips — this is the async/iterator case that was once the
        // last in-scope residual. The trailing `    next;` (4-space source indent)
        // must NOT be swallowed: the chain has terminated at level 3 (expected
        // indent 8), so a 4-space line breaks the run.
        let content = concat!(
            "a.ts(1,1): error TS2504: bad iter.\r\n",
            "\r\n\r\n",
            "==== a.ts (1 errors) ====\r\n",
            "    iter;\r\n",
            "    ~~~~\r\n",
            "!!! error TS2504: bad iter.\r\n",
            "!!! related TS2322 a.ts:1:1: Type X not assignable to Y.\r\n",
            "  Types of property 'p' are incompatible.\r\n",
            "    Type A is not assignable to type B.\r\n",
            "      Deep level three.\r\n",
            "    next;",
        );
        let parsed = parse_baseline(content).expect("parse");
        assert_eq!(parsed.diags.len(), 1);
        // 1 `!!! related` line + 3 chain-continuation lines.
        assert_eq!(parsed.diags[0].related.len(), 4);
        // The trailing source line survived as its own section source line.
        assert_eq!(parsed.sections[0].src_lines, vec!["iter;", "next;"]);
        assert_eq!(
            super::super::render::render_baseline(&parsed),
            content,
            "nested related chain must round-trip byte-identically"
        );
    }
}
