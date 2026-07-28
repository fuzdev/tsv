//! Parse + render the ANSI-colored `pretty=true` `.errors.txt` baselines.
//!
//! A faithful port of typescript-go's colored diagnostic renderer — the pretty
//! sibling of [`super::render`]'s plain path, and the second seam a future tsv
//! checker will emit its diagnostics through. The round-trip check
//! (`super::roundtrip`) parses a real pretty baseline into [`PrettyBaseline`]
//! and byte-compares the re-render, so the port has to reproduce every colored
//! subtlety of the original.
//!
//! Reference (censused vs pin `168e7015`):
//! - tsgo: `internal/testutil/tsbaseline/error_baseline.go`
//!   (`GetErrorBaseline` pretty branch, `iterateErrorBaseline`)
//! - tsgo: `internal/diagnosticwriter/diagnosticwriter.go`
//!   (`FormatDiagnosticWithColorAndContext`, `writeCodeSnippet`, `WriteLocation`,
//!   `WriteErrorSummaryText`, `writeTabularErrorsDisplay`, `prettyPathForFileError`)
//!
//! Structure of a pretty baseline (`GetErrorBaseline` with `pretty=true`):
//!
//! ```text
//! <colored top block>          ← FormatDiagnosticsWithColorAndContext (per-diag)
//!                              ← + harnessNewLine + harnessNewLine
//! <global !!! re-render>       ← identical to the plain path (fileless diags)
//! ==== <file> (N errors) ====  ← identical to the plain path (source + squiggles + !!!)
//! Found N errors …             ← WriteErrorSummaryText (message + optional table)
//! ```
//!
//! The **middle** (global re-render + `==== ` sections) is byte-for-byte the
//! same stream the plain renderer produces, so it is recovered by
//! [`super::baseline::parse_middle`] and re-emitted by
//! [`super::render::render_middle`]. Only the colored **top block** and the
//! **summary trailer** are pretty-specific, and only one datum is absent from
//! the plain middle: each file-bearing related-info's span **length**, which
//! appears solely in the top block's related code-frame (its tilde run). That is
//! recovered here and carried in [`PrettyBaseline::related_lens`].
//!
//! Two unit systems, kept deliberately distinct from the plain rune path:
//! - the top block measures columns and squiggle widths in **UTF-16 code units**
//!   (`scanner.GetECMALineAndUTF16CharacterOfPosition` / `core.UTF16Len`), where
//!   the plain path counts runes;
//! - `writeCodeSnippet` **destructively converts tabs to single spaces** in the
//!   displayed line (`diagnosticwriter.go:208`), where the plain path preserves
//!   tabs in the reprinted source.

use super::baseline::{CATEGORIES, Diag, Loc, ParsedBaseline, Section, read_code};
use super::render::{CRLF, col_to_byte, lf_line_starts, push_code, render_middle};
use std::collections::BTreeMap;
use std::fmt::Write as _;

// tsgo: diagnosticwriter.go:107-120 — the ANSI color / style sequences.
const GREY: &str = "\u{1b}[90m";
const RED: &str = "\u{1b}[91m";
const YELLOW: &str = "\u{1b}[93m";
const BLUE: &str = "\u{1b}[94m";
const CYAN: &str = "\u{1b}[96m";
const GUTTER_STYLE: &str = "\u{1b}[7m";
const RESET: &str = "\u{1b}[0m";
const GUTTER_SEPARATOR: &str = " ";
const ELLIPSIS: &str = "...";

/// tsgo: diagnostics_generated.go — `File_appears_to_be_binary` (code 1490); the
/// one diagnostic whose colored form omits the code snippet
/// (`diagnosticwriter.go:146`).
const FILE_BINARY_CODE: i32 = 1490;

/// A fully parsed ANSI `pretty=true` baseline: the plain middle model plus the
/// one datum the middle can't carry — related-info span lengths.
///
/// [`render_pretty`] turns it back into the original byte stream.
#[derive(Debug, Clone)]
pub struct PrettyBaseline {
    /// The plain model recovered from the middle (global re-render + `==== `
    /// sections) — drives both the middle re-render and the derived top block +
    /// summary.
    pub base: ParsedBaseline,
    /// For each diagnostic (parallel to `base.diags`), the UTF-16 span lengths of
    /// its **file-bearing** related infos, in `RelatedInformation` order.
    /// Recovered from the top block's related code-frames (their tilde runs) —
    /// the lengths the plain `!!! related` lines omit.
    pub related_lens: Vec<Vec<u32>>,
}

// ===========================================================================
// Parser
// ===========================================================================

/// Parse a whole ANSI `pretty=true` baseline into its [`PrettyBaseline`] model.
///
/// The baseline is split on `CRLF` and cut into three regions by structural
/// markers (no ANSI-content heuristics):
/// 1. **top block** — up to the first `!!! ` / `==== ` line; parsed for the
///    diagnostic heads and the related span lengths;
/// 2. **middle** — the global re-render + `==== ` sections, bounded below by the
///    summary trailer; recovered by [`super::baseline::parse_middle`];
/// 3. **summary trailer** — the first line that is neither 4-space-indented nor
///    `!!! ` / `==== ` (the `Found …` line) onward; re-derived at render time.
///
/// # Errors
///
/// Returns a short reason on any structural surprise (an unparsable head, a
/// missing middle region, a section desync) — the buckets the round-trip driver
/// reports.
pub fn parse_pretty(content: &str) -> Result<PrettyBaseline, String> {
    let lines: Vec<&str> = content.split(CRLF).collect();

    // Region 1/2 boundary: the first middle line (global re-render or section).
    let mid_start = lines
        .iter()
        .position(|l| l.starts_with("!!! ") || l.starts_with("==== "))
        .ok_or("pretty baseline has no middle region")?;

    // Region 2/3 boundary: the first line that is not part of the middle (the
    // summary's `Found …` line), or the end of the file.
    let summ_start = (mid_start..lines.len())
        .find(|&k| !is_middle_line(lines[k]))
        .unwrap_or(lines.len());

    let (mut diags, related_lens) = parse_pretty_top(&lines[..mid_start])?;
    if diags.is_empty() {
        return Err("pretty top block has no diagnostics".to_string());
    }

    let sections = super::baseline::parse_middle(&lines, mid_start, summ_start, &mut diags)?;

    Ok(PrettyBaseline {
        base: ParsedBaseline { diags, sections },
        related_lens,
    })
}

/// A middle-region line: a 4-space-indented source/squiggle line, a `!!! ` body
/// line, or a `==== ` section header. Everything else after the sections start is
/// the summary trailer.
fn is_middle_line(l: &str) -> bool {
    l.starts_with("    ") || l.starts_with("!!! ") || l.starts_with("==== ")
}

/// Parse the colored top block into diagnostic heads plus, per diagnostic, the
/// UTF-16 lengths of its file-bearing related infos (from the related
/// code-frames' tilde runs).
///
/// The heads mirror `FlattenDiagnosticMessage`'s output: a positional head line
/// (cyan file / yellow line:col), an optional global head (category-colored,
/// no location), 0+ 2-space message-chain continuation lines, then the code
/// frames. Only the related squiggle lines carry data we can't recover from the
/// middle; every other top line is skipped.
fn parse_pretty_top(top: &[&str]) -> Result<(Vec<Diag>, Vec<Vec<u32>>), String> {
    let mut diags: Vec<Diag> = Vec::new();
    let mut related_lens: Vec<Vec<u32>> = Vec::new();

    for &line in top {
        if line.is_empty() {
            continue;
        }

        // Related code-frame line (4-space indent + gutter). Capture the tilde
        // run on the squiggle line (all-blank gutter); skip the content line.
        if let Some(after_indent) = line.strip_prefix("    ")
            && after_indent.starts_with(GUTTER_STYLE)
        {
            if gutter_is_blank(after_indent) {
                let lens = related_lens
                    .last_mut()
                    .ok_or("related squiggle before any diagnostic")?;
                lens.push(count_tildes(line));
            }
            continue;
        }

        // Main code-frame line (gutter at column 0): span length comes from the
        // section squiggle, so nothing to capture.
        if line.starts_with(GUTTER_STYLE) {
            continue;
        }

        // Related head (2-space indent + cyan location): the length comes from
        // its squiggle line, so skip the head.
        if line.strip_prefix("  ").is_some_and(|r| r.starts_with(CYAN)) {
            continue;
        }

        // Diagnostic head — positional (cyan file at column 0) or global
        // (category color at column 0).
        if line.starts_with(CYAN) || starts_with_category_color(line) {
            let head = parse_pretty_head(line)?;
            diags.push(Diag {
                file: head.file,
                loc: head.loc,
                category: head.category,
                code: head.code,
                msg_lines: vec![head.first_msg],
                related: Vec::new(),
            });
            related_lens.push(Vec::new());
            continue;
        }

        // Message-chain continuation (2-space indent + text): append to the
        // current diagnostic's message.
        if line.starts_with("  ") {
            let d = diags
                .last_mut()
                .ok_or("message continuation before any diagnostic")?;
            d.msg_lines.push(line.to_string());
            continue;
        }

        return Err(format!("unrecognized pretty top-block line: {line:?}"));
    }

    Ok((diags, related_lens))
}

/// True if the gutter of `after_indent` (which begins with [`GUTTER_STYLE`]) is
/// all spaces — the squiggle line's `%*s` gutter, versus the content line's
/// `%*d` line-number gutter.
fn gutter_is_blank(after_indent: &str) -> bool {
    let Some(rest) = after_indent.strip_prefix(GUTTER_STYLE) else {
        return false;
    };
    // The gutter never contains ESC, so the first RESET terminates it.
    rest.find(RESET)
        .and_then(|end| rest.get(..end))
        .is_some_and(|g| g.bytes().all(|b| b == b' '))
}

/// Count the tilde run on a squiggle line (its UTF-16 span length). Tildes appear
/// only as the squiggle, so a raw byte count is exact.
fn count_tildes(line: &str) -> u32 {
    line.bytes().filter(|&b| b == b'~').count() as u32
}

/// True if `line` begins with a category color (a global head with no location).
/// The positional file color (cyan) is deliberately excluded.
fn starts_with_category_color(line: &str) -> bool {
    line.starts_with(RED)
        || line.starts_with(YELLOW)
        || line.starts_with(GREY)
        || line.starts_with(BLUE)
}

/// The head fields of a top-block diagnostic, before its code frame.
struct PrettyHead {
    file: Option<String>,
    loc: Option<Loc>,
    category: String,
    code: i32,
    first_msg: String,
}

/// Parse one top-block head line (ANSI-stripped): positional
/// `{file}:{line}:{col} - {category} TS{code}: {message}` or global
/// `{category} TS{code}: {message}`.
fn parse_pretty_head(line: &str) -> Result<PrettyHead, String> {
    let stripped = ansi_strip(line);
    if line.starts_with(CYAN) {
        // Positional: split off the location at the first ` - ` (the ` - ` the
        // renderer inserts after WriteLocation; a message may contain more).
        let (locpart, rest) = stripped
            .split_once(" - ")
            .ok_or_else(|| format!("pretty head missing ' - ': {stripped:?}"))?;
        let (file, loc) = parse_pretty_location(locpart)
            .ok_or_else(|| format!("bad pretty location: {locpart:?}"))?;
        let (category, code, msg) = parse_category_code_msg(rest)
            .ok_or_else(|| format!("bad pretty head body: {rest:?}"))?;
        Ok(PrettyHead {
            file: Some(file),
            loc: Some(loc),
            category,
            code,
            first_msg: msg,
        })
    } else {
        let (category, code, msg) = parse_category_code_msg(&stripped)
            .ok_or_else(|| format!("bad global pretty head: {stripped:?}"))?;
        Ok(PrettyHead {
            file: None,
            loc: None,
            category,
            code,
            first_msg: msg,
        })
    }
}

/// Parse a top-block location `{file}:{line}:{col}` (colon form, always numbered
/// — the harness's `(--,--)` masking never reaches the colored path).
fn parse_pretty_location(s: &str) -> Option<(String, Loc)> {
    let (rest, col) = s.rsplit_once(':')?;
    let (file, line) = rest.rsplit_once(':')?;
    Some((
        file.to_string(),
        Loc::Numbered {
            line: line.parse().ok()?,
            col: col.parse().ok()?,
        },
    ))
}

/// Parse `{category} TS{code}: {message}` at the start of `s`.
fn parse_category_code_msg(s: &str) -> Option<(String, i32, String)> {
    for cat in CATEGORIES {
        if let Some(rest) = s.strip_prefix(cat).and_then(|r| r.strip_prefix(" TS")) {
            let (code, consumed) = read_code(rest)?;
            let msg = rest.get(consumed..)?.strip_prefix(": ")?;
            return Some((cat.to_string(), code, msg.to_string()));
        }
    }
    None
}

/// Strip SGR ANSI escapes (`ESC [ … m`) from `line`. Every sequence the colored
/// renderer emits is an SGR terminating in `m`.
fn ansi_strip(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(esc) = rest.find('\u{1b}') {
        out.push_str(rest.get(..esc).unwrap_or(""));
        let after = rest.get(esc..).unwrap_or("");
        match after.find('m') {
            Some(m) => rest = after.get(m + 1..).unwrap_or(""),
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

// ===========================================================================
// Renderer
// ===========================================================================

/// Render a [`PrettyBaseline`] back to its `.errors.txt` byte stream: the colored
/// top block, the plain middle, then the summary trailer.
///
/// Mirrors `GetErrorBaseline` (pretty branch) + `iterateErrorBaseline`.
///
/// # Errors
///
/// Returns a short reason if a diagnostic references a section/source that the
/// model doesn't carry (an internal inconsistency); the round-trip driver
/// buckets it.
pub fn render_pretty(b: &PrettyBaseline) -> Result<String, String> {
    let mut out = String::new();

    // 1. colored top block (FormatDiagnosticsWithColorAndContext) …
    render_pretty_top(&mut out, b)?;
    // … + harnessNewLine + harnessNewLine (error_baseline.go:142).
    out.push_str(CRLF);
    out.push_str(CRLF);

    // 2 & 3. the plain middle (global re-render + sections).
    render_middle(&mut out, &b.base.diags, &b.base.sections);

    // 4. the colored summary trailer (WriteErrorSummaryText).
    render_pretty_summary(&mut out, &b.base);

    Ok(out)
}

/// Render the top block: each diagnostic in order, joined by a single `newLine`
/// (`FormatDiagnosticsWithColorAndContext`, so between two diagnostics the prior
/// one's own trailing newline plus this separator make a blank line).
fn render_pretty_top(out: &mut String, b: &PrettyBaseline) -> Result<(), String> {
    for (i, d) in b.base.diags.iter().enumerate() {
        if i > 0 {
            out.push_str(CRLF);
        }
        render_pretty_diagnostic(out, b, i, d)?;
    }
    Ok(())
}

/// Render one colored diagnostic (`FormatDiagnosticWithColorAndContext`):
/// optional location, category + code, flattened message, code frame, and
/// related-info blocks.
fn render_pretty_diagnostic(
    out: &mut String,
    b: &PrettyBaseline,
    i: usize,
    d: &Diag,
) -> Result<(), String> {
    if let Some(file) = &d.file {
        let (line, col) = numbered_loc(d.loc)
            .ok_or_else(|| format!("pretty diagnostic for {file} has no numbered location"))?;
        write_location(out, file, line, col);
        out.push_str(" - ");
    }

    // {categoryColor}{category}{reset}{grey} TS{code}: {reset}{message}
    out.push_str(category_color(&d.category));
    out.push_str(&d.category);
    out.push_str(RESET);
    out.push_str(GREY);
    out.push_str(" TS");
    push_code(out, d.code);
    out.push_str(": ");
    out.push_str(RESET);
    write_flattened_message(out, &d.msg_lines);

    // Code frame for a file-bearing, non-binary diagnostic.
    if d.file.is_some() && d.code != FILE_BINARY_CODE {
        let (sec, pos, len) =
            diag_span(b, i).ok_or_else(|| format!("no section span for diagnostic {i}"))?;
        out.push_str(CRLF);
        write_code_snippet(
            out,
            &sec.src_lines,
            pos,
            len,
            category_color(&d.category),
            "",
        );
        out.push_str(CRLF);
    }

    render_pretty_related(out, b, i, d)
}

/// Render a diagnostic's related-info blocks (`diagnosticwriter.go:152-166`).
/// A file-bearing related emits `  {location} - {message}` + a cyan code frame;
/// a fileless related emits only the trailing `newLine`. Each contributes one
/// trailing `newLine` regardless.
fn render_pretty_related(
    out: &mut String,
    b: &PrettyBaseline,
    i: usize,
    d: &Diag,
) -> Result<(), String> {
    let entries = parse_related_entries(&d.related);
    if entries.is_empty() {
        return Ok(());
    }
    let lens = b.related_lens.get(i).map_or(&[][..], Vec::as_slice);
    let mut len_idx = 0usize;
    for e in &entries {
        if let Some(loc) = &e.loc {
            out.push_str(CRLF);
            out.push_str("  ");
            write_location(out, &loc.file, loc.line, loc.col);
            out.push_str(" - ");
            write_flattened_message(out, &e.msg_lines);
            let len_utf16 = *lens
                .get(len_idx)
                .ok_or_else(|| format!("related span-length underflow for diagnostic {i}"))?;
            len_idx += 1;
            let (sec, pos, byte_len) = related_span(b, &loc.file, loc.line, loc.col, len_utf16)
                .ok_or_else(|| {
                    format!(
                        "no related source for {}:{}:{}",
                        loc.file, loc.line, loc.col
                    )
                })?;
            write_code_snippet(out, &sec.src_lines, pos, byte_len, CYAN, "    ");
        }
        out.push_str(CRLF);
    }
    Ok(())
}

/// tsgo: diagnosticwriter.go:305-319 — `WriteLocation`, the colored
/// `{cyan}file{reset}:{yellow}line{reset}:{yellow}col{reset}` (line/col 1-based).
fn write_location(out: &mut String, file: &str, line: u32, col: u32) {
    out.push_str(CYAN);
    out.push_str(file);
    out.push_str(RESET);
    out.push(':');
    out.push_str(YELLOW);
    let _ = write!(out, "{line}");
    out.push_str(RESET);
    out.push(':');
    out.push_str(YELLOW);
    let _ = write!(out, "{col}");
    out.push_str(RESET);
}

/// tsgo: diagnosticwriter.go:263-281 — `WriteFlattenedDiagnosticMessage`. The
/// first message line then each chain line, joined by `newLine`; the stored
/// continuation lines already carry their 2-space-per-level indent.
fn write_flattened_message(out: &mut String, msg_lines: &[String]) {
    for (k, m) in msg_lines.iter().enumerate() {
        if k > 0 {
            out.push_str(CRLF);
        }
        out.push_str(m);
    }
}

/// tsgo: diagnosticwriter.go:283-295 — `getCategoryFormat`. Unknown categories
/// never reach here (the set is fixed); default to the error color rather than
/// panic as the Go source does.
fn category_color(category: &str) -> &'static str {
    match category {
        "warning" => YELLOW,
        "suggestion" => GREY,
        "message" => BLUE,
        _ => RED,
    }
}

/// tsgo: diagnosticwriter.go:169-251 — `writeCodeSnippet`. Emits the gutter-style
/// line-number + content lines and their squiggle lines for `[start, start+length)`,
/// eliding the interior of a >5-line span with an ellipsis gutter. Columns and
/// squiggle widths are **UTF-16** code units; the displayed content has tabs
/// converted to single spaces and trailing whitespace trimmed.
fn write_code_snippet(
    out: &mut String,
    src_lines: &[String],
    start: usize,
    length: usize,
    squiggle_color: &str,
    indent: &str,
) {
    let starts = lf_line_starts(src_lines);
    let (first_line, first_char) = line_and_utf16col(src_lines, &starts, start);
    let (last_line, mut last_char) = line_and_utf16col(src_lines, &starts, start + length);
    if length == 0 {
        // When length is zero, squiggle the character right after the start.
        last_char += 1;
    }

    let has_more_than_five = last_line.saturating_sub(first_line) >= 4;
    let mut gutter_width = digits(last_line + 1);
    if has_more_than_five {
        gutter_width = gutter_width.max(ELLIPSIS.len());
    }

    let mut i = first_line;
    while i <= last_line {
        out.push_str(CRLF);

        // Over 5 lines: show the first 2 and last 2, eliding the middle.
        if has_more_than_five && first_line + 1 < i && i + 1 < last_line {
            out.push_str(indent);
            out.push_str(GUTTER_STYLE);
            let _ = write!(out, "{ELLIPSIS:>gutter_width$}");
            out.push_str(RESET);
            out.push_str(GUTTER_SEPARATOR);
            out.push_str(CRLF);
            i = last_line.saturating_sub(1);
        }

        let content = trim_end_ws(src_lines.get(i).map_or("", String::as_str)).replace('\t', " ");

        // Gutter + content line.
        out.push_str(indent);
        out.push_str(GUTTER_STYLE);
        let line_no = i + 1;
        let _ = write!(out, "{line_no:>gutter_width$}");
        out.push_str(RESET);
        out.push_str(GUTTER_SEPARATOR);
        out.push_str(&content);
        out.push_str(CRLF);

        // Gutter + squiggle line.
        out.push_str(indent);
        out.push_str(GUTTER_STYLE);
        let _ = write!(out, "{blank:>gutter_width$}", blank = "");
        out.push_str(RESET);
        out.push_str(GUTTER_SEPARATOR);
        out.push_str(squiggle_color);
        let content_units = utf16_len(&content);
        if i == first_line {
            let last_char_for_line = if i == last_line {
                last_char
            } else {
                content_units
            };
            push_repeat(out, ' ', first_char);
            push_repeat(out, '~', last_char_for_line.saturating_sub(first_char));
        } else if i == last_line {
            push_repeat(out, '~', last_char);
        } else {
            push_repeat(out, '~', content_units);
        }
        out.push_str(RESET);

        i += 1;
    }
}

/// tsgo: diagnosticwriter.go:330-375 — `WriteErrorSummaryText`. Counts
/// error-category diagnostics, then emits the `Found …` message and, for >1
/// erroring file, the tabular file/count display.
fn render_pretty_summary(out: &mut String, base: &ParsedBaseline) {
    // getErrorSummary: error-category diagnostics only, grouped by file (sorted).
    let mut total = 0usize;
    let mut global_errors = 0usize;
    let mut by_file: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, d) in base.diags.iter().enumerate() {
        if d.category != "error" {
            continue;
        }
        total += 1;
        match &d.file {
            None => global_errors += 1,
            Some(f) => by_file.entry(f.as_str()).or_default().push(i),
        }
    }
    if total == 0 {
        return;
    }

    let num_erroring_files = by_file.len();
    // sortedFiles[0] — BTreeMap iterates keys in sorted (byte) order.
    let first_file_name = by_file
        .iter()
        .next()
        .map(|(file, idxs)| pretty_path_for_file_error(base, file, idxs))
        .unwrap_or_default();

    let message = if total == 1 {
        if global_errors > 0 || first_file_name.is_empty() {
            "Found 1 error.".to_string()
        } else {
            format!("Found 1 error in {first_file_name}")
        }
    } else {
        match num_erroring_files {
            0 => format!("Found {total} errors."),
            1 => format!("Found {total} errors in the same file, starting at: {first_file_name}"),
            _ => format!("Found {total} errors in {num_erroring_files} files."),
        }
    };

    out.push_str(CRLF);
    out.push_str(&message);
    out.push_str(CRLF);
    out.push_str(CRLF);
    if num_erroring_files > 1 {
        write_tabular(out, base, &by_file);
        out.push_str(CRLF);
    }
}

/// tsgo: diagnosticwriter.go:412-441 — `writeTabularErrorsDisplay`. The
/// `Errors  Files` header then one right-justified count + pretty path per file.
fn write_tabular(out: &mut String, base: &ParsedBaseline, by_file: &BTreeMap<&str, Vec<usize>>) {
    let max_errors = by_file.values().map(Vec::len).max().unwrap_or(0);
    let header_row = "Errors  Files";
    let left_heading_len = header_row.split(' ').next().map_or(0, str::len);
    let biggest_count_len = digits(max_errors);
    let left_padding_goal = left_heading_len.max(biggest_count_len);
    let header_padding = biggest_count_len.saturating_sub(left_heading_len);

    push_repeat(out, ' ', header_padding as u32);
    out.push_str(header_row);
    out.push_str(CRLF);

    for (file, idxs) in by_file {
        let count = idxs.len();
        let _ = write!(out, "{count:>left_padding_goal$}  ");
        out.push_str(&pretty_path_for_file_error(base, file, idxs));
        out.push_str(CRLF);
    }
}

/// tsgo: diagnosticwriter.go:443-459 — `prettyPathForFileError`. The file name
/// plus a grey `:{line}` of its first error (1-based). Relative-path collapsing
/// is a no-op for the baselines' already-relative names.
fn pretty_path_for_file_error(base: &ParsedBaseline, file: &str, idxs: &[usize]) -> String {
    let line = idxs
        .first()
        .and_then(|&i| base.diags.get(i))
        .and_then(|d| numbered_loc(d.loc))
        .map_or(0, |(l, _)| l);
    format!("{file}{GREY}:{line}{RESET}")
}

// ===========================================================================
// Related-line + span helpers
// ===========================================================================

/// One recovered related-info entry from a diagnostic's verbatim `related` lines.
struct RelatedEntry {
    /// `Some` for a file-bearing related, `None` for a fileless one.
    loc: Option<RelatedLoc>,
    /// The related's flattened message (first line, then any chain lines).
    msg_lines: Vec<String>,
}

/// A file-bearing related location.
struct RelatedLoc {
    file: String,
    line: u32,
    col: u32,
}

/// Parse a diagnostic's verbatim `related` lines (`!!! related …` heads plus any
/// bare message-chain continuation lines) into structured entries.
fn parse_related_entries(related: &[String]) -> Vec<RelatedEntry> {
    let mut entries: Vec<RelatedEntry> = Vec::new();
    for line in related {
        if let Some(rest) = line.strip_prefix("!!! related ") {
            let (loc, msg) = parse_related_verbatim(rest);
            entries.push(RelatedEntry {
                loc,
                msg_lines: vec![msg],
            });
        } else if let Some(last) = entries.last_mut() {
            // A chain-continuation line of the previous related's own message.
            last.msg_lines.push(line.clone());
        }
    }
    entries
}

/// Parse the body of a `!!! related ` line (after that prefix):
/// `TS{code} {file}:{line}:{col}: {message}` or `TS{code}: {message}` (fileless).
fn parse_related_verbatim(rest: &str) -> (Option<RelatedLoc>, String) {
    let after_ts = rest.strip_prefix("TS").unwrap_or(rest);
    let Some((_, consumed)) = read_code(after_ts) else {
        return (None, rest.to_string());
    };
    let tail = after_ts.get(consumed..).unwrap_or("");
    if let Some(msg) = tail.strip_prefix(": ") {
        return (None, msg.to_string()); // fileless related
    }
    if let Some(locmsg) = tail.strip_prefix(' ')
        && let Some((loc, msg)) = split_related_location(locmsg)
    {
        return (Some(loc), msg);
    }
    (None, tail.to_string())
}

/// Split `{file}:{line}:{col}: {message}` at the `: ` that terminates the
/// location (the first one whose preceding token is a valid `file:line:col`), so
/// a colon-bearing message can't confuse it.
fn split_related_location(s: &str) -> Option<(RelatedLoc, String)> {
    let mut from = 0usize;
    loop {
        let rel = s.get(from..)?.find(": ")?;
        let pos = from + rel;
        if let Some((file, line, col)) = s
            .get(..pos)
            .and_then(|locpart| locpart.rsplit_once(':'))
            .and_then(|(fl, col)| {
                let (file, line) = fl.rsplit_once(':')?;
                Some((file, line.parse::<u32>().ok()?, col.parse::<u32>().ok()?))
            })
            && !file.is_empty()
        {
            return Some((
                RelatedLoc {
                    file: file.to_string(),
                    line,
                    col,
                },
                s.get(pos + 2..)?.to_string(),
            ));
        }
        from = pos + 2;
    }
}

/// Find a diagnostic's `(section, pos, len)` span in its file section.
fn diag_span(b: &PrettyBaseline, diag_index: usize) -> Option<(&Section, usize, usize)> {
    let file = b.base.diags.get(diag_index)?.file.as_deref()?;
    let sec = b.base.sections.iter().find(|s| s.name == file)?;
    let sd = sec.diags.iter().find(|sd| sd.diag_index == diag_index)?;
    Some((sec, sd.pos_abs, sd.len))
}

/// Resolve a related location + UTF-16 length to `(section, byte_pos, byte_len)`
/// in the related file's source.
fn related_span<'a>(
    b: &'a PrettyBaseline,
    file: &str,
    line: u32,
    col: u32,
    len_utf16: u32,
) -> Option<(&'a Section, usize, usize)> {
    let sec = b.base.sections.iter().find(|s| s.name == file)?;
    let starts = lf_line_starts(&sec.src_lines);
    let line_idx = (line as usize).checked_sub(1)?;
    let src_line = sec.src_lines.get(line_idx)?;
    let start_in_line = col_to_byte(src_line, col);
    let pos_abs = starts.get(line_idx)? + start_in_line;
    let byte_len = advance_utf16(src_line, start_in_line, len_utf16);
    Some((sec, pos_abs, byte_len))
}

// ===========================================================================
// UTF-16 + small utilities (the pretty path's rune-distinct measurements)
// ===========================================================================

/// Extract `(line, col)` from a numbered [`Loc`]; `None` for masked/absent.
fn numbered_loc(loc: Option<Loc>) -> Option<(u32, u32)> {
    match loc {
        Some(Loc::Numbered { line, col }) => Some((line, col)),
        _ => None,
    }
}

/// UTF-16 code-unit length of `s` — the pretty path's width unit, kept distinct
/// from the plain path's rune count and never merged with it (see `advance_utf16`).
fn utf16_len(s: &str) -> u32 {
    s.chars().map(|c| c.len_utf16() as u32).sum()
}

/// Line index and UTF-16 column of the LF-content byte offset `pos` over
/// `src_lines` (mirrors `scanner.GetECMALineAndUTF16CharacterOfPosition`). Measures
/// in UTF-16 code units — the pretty path's unit, not interchangeable with the
/// plain rune path (see `advance_utf16`).
fn line_and_utf16col(src_lines: &[String], starts: &[usize], pos: usize) -> (usize, u32) {
    let line = match starts.binary_search(&pos) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    };
    let line_start = *starts.get(line).unwrap_or(&0);
    let in_line = pos.saturating_sub(line_start);
    let src = src_lines.get(line).map_or("", String::as_str);
    (
        line,
        utf16_len(src.get(..in_line.min(src.len())).unwrap_or("")),
    )
}

/// Advance `units` UTF-16 code units from `start_byte` in `line`, returning the
/// number of bytes covered.
///
/// Unit-system guard: this and `render.rs`'s `advance_runes` measure in different
/// units and must never be merged or deduplicated. The pretty path counts UTF-16
/// code units (tsgo's `writeCodeSnippet` / `core.UTF16Len`); the plain path counts
/// runes (`error_baseline.go`'s `utf8.RuneCountInString`). An astral char is two
/// units here but one rune there, so the same span squiggles a different width.
fn advance_utf16(line: &str, start_byte: usize, units: u32) -> usize {
    let Some(slice) = line.get(start_byte..) else {
        return 0;
    };
    let mut seen = 0u32;
    for (byte_idx, ch) in slice.char_indices() {
        if seen >= units {
            return byte_idx;
        }
        seen += ch.len_utf16() as u32;
    }
    slice.len()
}

/// Decimal digit count of `n` (`len(strconv.Itoa(n))` for `n >= 0`).
fn digits(n: usize) -> usize {
    n.checked_ilog10().map_or(1, |l| l as usize + 1)
}

/// Trailing-whitespace trim matching Go's `strings.TrimRightFunc(unicode.IsSpace)`.
fn trim_end_ws(s: &str) -> &str {
    s.trim_end_matches(char::is_whitespace)
}

/// Push `n` copies of `ch`.
fn push_repeat(out: &mut String, ch: char, n: u32) {
    for _ in 0..n {
        out.push(ch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tsc_conformance::baseline::SectionDiag;
    use crate::tsc_conformance::render::self_assertion_violations;

    // --- parser-unit coverage ---

    #[test]
    fn ansi_strip_removes_sgr_sequences() {
        assert_eq!(
            ansi_strip("\u{1b}[96mfile.ts\u{1b}[0m:\u{1b}[93m3\u{1b}[0m"),
            "file.ts:3"
        );
        assert_eq!(ansi_strip("plain"), "plain");
    }

    #[test]
    fn pretty_head_positional_and_global() {
        let pos = parse_pretty_head("\u{1b}[96ma.ts\u{1b}[0m:\u{1b}[93m3\u{1b}[0m:\u{1b}[93m8\u{1b}[0m - \u{1b}[91merror\u{1b}[0m\u{1b}[90m TS2345: \u{1b}[0mArgument bad.").expect("positional");
        assert_eq!(pos.file.as_deref(), Some("a.ts"));
        assert_eq!(pos.loc, Some(Loc::Numbered { line: 3, col: 8 }));
        assert_eq!(pos.category, "error");
        assert_eq!(pos.code, 2345);
        assert_eq!(pos.first_msg, "Argument bad.");

        let global = parse_pretty_head(
            "\u{1b}[91merror\u{1b}[0m\u{1b}[90m TS-1: \u{1b}[0mPre-emit mismatch!",
        )
        .expect("global");
        assert!(global.file.is_none() && global.loc.is_none());
        assert_eq!(global.code, -1);
        assert_eq!(global.first_msg, "Pre-emit mismatch!");
    }

    #[test]
    fn related_verbatim_file_and_fileless() {
        let (loc, msg) =
            parse_related_verbatim("TS6203 file2.ts:1:6: 'Foo' was also declared here.");
        let loc = loc.expect("file-bearing");
        assert_eq!((loc.file.as_str(), loc.line, loc.col), ("file2.ts", 1, 6));
        assert_eq!(msg, "'Foo' was also declared here.");

        let (loc2, msg2) = parse_related_verbatim("TS-1: The excess diagnostics are:");
        assert!(loc2.is_none());
        assert_eq!(msg2, "The excess diagnostics are:");
    }

    #[test]
    fn gutter_and_tilde_classification() {
        // Content gutter (digit) vs squiggle gutter (blank).
        assert!(!gutter_is_blank("\u{1b}[7m3\u{1b}[0m const x;"));
        assert!(gutter_is_blank(
            "\u{1b}[7m \u{1b}[0m \u{1b}[91m ~~~\u{1b}[0m"
        ));
        assert_eq!(
            count_tildes("    \u{1b}[7m \u{1b}[0m \u{1b}[96m     ~~~\u{1b}[0m"),
            3
        );
    }

    // --- standalone renderer coverage (model built by hand, not via the parser) ---
    //
    // Mirrors render.rs's plain standalone tests: assemble a `PrettyBaseline`
    // directly — the path a future tsv checker takes — so the colored renderer's
    // byte contract is pinned independently of the parser.

    /// Build an `error`-category diagnostic.
    fn diag(
        file: Option<&str>,
        loc: Option<Loc>,
        code: i32,
        msgs: &[&str],
        related: &[&str],
    ) -> Diag {
        Diag {
            file: file.map(str::to_string),
            loc,
            category: "error".to_string(),
            code,
            msg_lines: msgs.iter().map(|s| (*s).to_string()).collect(),
            related: related.iter().map(|s| (*s).to_string()).collect(),
        }
    }

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
        // `const x = 1;` — the `x` is a length-1 span at byte 6 (UTF-16 col 7).
        let b = PrettyBaseline {
            base: ParsedBaseline {
                diags: vec![diag(
                    Some("a.ts"),
                    Some(Loc::Numbered { line: 1, col: 7 }),
                    2304,
                    &["Cannot find name 'x'."],
                    &[],
                )],
                sections: vec![section("a.ts", &["const x = 1;"], &[(0, 6, 1)])],
            },
            related_lens: vec![vec![]],
        };
        let expected = format!(
            concat!(
                // colored top block
                "{c}a.ts{r}:{y}1{r}:{y}7{r} - {red}error{r}{g} TS2304: {r}Cannot find name 'x'.\r\n",
                "\r\n",
                "{gs}1{r} const x = 1;\r\n",
                "{gs} {r} {red}      ~{r}\r\n",
                // + harnessNewLine + harnessNewLine
                "\r\n\r\n",
                // plain middle
                "==== a.ts (1 errors) ====\r\n",
                "    const x = 1;\r\n",
                "          ~\r\n",
                "!!! error TS2304: Cannot find name 'x'.",
                // summary trailer
                "\r\nFound 1 error in a.ts{g}:1{r}\r\n\r\n",
            ),
            c = CYAN,
            r = RESET,
            y = YELLOW,
            red = RED,
            g = GREY,
            gs = GUTTER_STYLE
        );
        assert_eq!(render_pretty(&b).expect("render"), expected);
        assert!(self_assertion_violations(&b.base).is_empty());
    }

    #[test]
    fn standalone_render_related_and_message_chain() {
        // A two-line message plus one file-bearing related (its own file section).
        let b = PrettyBaseline {
            base: ParsedBaseline {
                diags: vec![diag(
                    Some("index.ts"),
                    Some(Loc::Numbered { line: 1, col: 8 }),
                    2345,
                    &["Argument bad.", "  Provides no match."],
                    &["!!! related TS7038 index.ts:1:1: Type originates here."],
                )],
                sections: vec![section("index.ts", &["invoke(foo);"], &[(0, 7, 3)])],
            },
            // The related span `invoke` is 6 UTF-16 units at index.ts:1:1.
            related_lens: vec![vec![6]],
        };
        let expected = format!(
            concat!(
                "{c}index.ts{r}:{y}1{r}:{y}8{r} - {red}error{r}{g} TS2345: {r}Argument bad.\r\n",
                "  Provides no match.\r\n",
                "\r\n",
                "{gs}1{r} invoke(foo);\r\n",
                "{gs} {r} {red}       ~~~{r}\r\n",
                "\r\n",
                "  {c}index.ts{r}:{y}1{r}:{y}1{r} - Type originates here.\r\n",
                "    {gs}1{r} invoke(foo);\r\n",
                "    {gs} {r} {c}~~~~~~{r}\r\n",
                "\r\n\r\n",
                "==== index.ts (1 errors) ====\r\n",
                "    invoke(foo);\r\n",
                "           ~~~\r\n",
                "!!! error TS2345: Argument bad.\r\n",
                "!!! error TS2345:   Provides no match.\r\n",
                "!!! related TS7038 index.ts:1:1: Type originates here.",
                "\r\nFound 1 error in index.ts{g}:1{r}\r\n\r\n",
            ),
            c = CYAN,
            r = RESET,
            y = YELLOW,
            red = RED,
            g = GREY,
            gs = GUTTER_STYLE
        );
        assert_eq!(render_pretty(&b).expect("render"), expected);
        assert!(self_assertion_violations(&b.base).is_empty());
    }

    #[test]
    fn standalone_render_global_diag_and_tabular_summary() {
        // A global (fileless) error, then a file error in each of two files —
        // exercises the global head (no location/snippet) and the tabular trailer.
        let b = PrettyBaseline {
            base: ParsedBaseline {
                diags: vec![
                    diag(None, None, -1, &["Count mismatch!"], &[]),
                    diag(
                        Some("a.ts"),
                        Some(Loc::Numbered { line: 1, col: 1 }),
                        2304,
                        &["Cannot find name 'x'."],
                        &[],
                    ),
                    diag(
                        Some("b.ts"),
                        Some(Loc::Numbered { line: 1, col: 1 }),
                        2304,
                        &["Cannot find name 'y'."],
                        &[],
                    ),
                ],
                sections: vec![
                    section("a.ts", &["x;"], &[(1, 0, 1)]),
                    section("b.ts", &["y;"], &[(2, 0, 1)]),
                ],
            },
            related_lens: vec![vec![], vec![], vec![]],
        };
        let expected = format!(
            concat!(
                // global head — no location, no code frame, no related, so it
                // ends at the message; only the inter-diagnostic separator
                // newline follows (no blank line, unlike a diag with a snippet).
                "{red}error{r}{g} TS-1: {r}Count mismatch!\r\n",
                // a.ts positional diag
                "{c}a.ts{r}:{y}1{r}:{y}1{r} - {red}error{r}{g} TS2304: {r}Cannot find name 'x'.\r\n",
                "\r\n",
                "{gs}1{r} x;\r\n",
                "{gs} {r} {red}~{r}\r\n",
                "\r\n",
                // b.ts positional diag
                "{c}b.ts{r}:{y}1{r}:{y}1{r} - {red}error{r}{g} TS2304: {r}Cannot find name 'y'.\r\n",
                "\r\n",
                "{gs}1{r} y;\r\n",
                "{gs} {r} {red}~{r}\r\n",
                "\r\n\r\n",
                // plain middle — global re-render then sections
                "!!! error TS-1: Count mismatch!\r\n",
                "==== a.ts (1 errors) ====\r\n",
                "    x;\r\n",
                "    ~\r\n",
                "!!! error TS2304: Cannot find name 'x'.\r\n",
                "==== b.ts (1 errors) ====\r\n",
                "    y;\r\n",
                "    ~\r\n",
                "!!! error TS2304: Cannot find name 'y'.",
                // summary — "in N files" + tabular
                "\r\nFound 3 errors in 2 files.\r\n\r\n",
                "Errors  Files\r\n",
                "     1  a.ts{g}:1{r}\r\n",
                "     1  b.ts{g}:1{r}\r\n",
                "\r\n",
            ),
            c = CYAN,
            r = RESET,
            y = YELLOW,
            red = RED,
            g = GREY,
            gs = GUTTER_STYLE
        );
        assert_eq!(render_pretty(&b).expect("render"), expected);
        assert!(self_assertion_violations(&b.base).is_empty());
    }

    #[test]
    fn standalone_render_multiline_snippet() {
        // A 3-line span exercises the multi-line code frame (first line squiggles
        // from the column to end-of-content, interior fully, last to its column)
        // — a path no in-corpus pretty baseline reaches, so pinned here.
        let src = ["foo(", "  bar,", ")"];
        // span covers `foo(\n  bar,\n)` = bytes 0..12 (LF-joined: 4 + 1 + 6 + 1 + ...).
        // "foo(" = 4, +LF = 5; "  bar," = 6 → 11, +LF = 12; ")" at 12. len to end of ")" = 13-0 = 13.
        let b = PrettyBaseline {
            base: ParsedBaseline {
                diags: vec![diag(
                    Some("a.ts"),
                    Some(Loc::Numbered { line: 1, col: 1 }),
                    2554,
                    &["Expected 0 arguments."],
                    &[],
                )],
                sections: vec![section("a.ts", &src, &[(0, 0, 13)])],
            },
            related_lens: vec![vec![]],
        };
        let expected = format!(
            concat!(
                "{c}a.ts{r}:{y}1{r}:{y}1{r} - {red}error{r}{g} TS2554: {r}Expected 0 arguments.\r\n",
                "\r\n",
                "{gs}1{r} foo(\r\n",
                "{gs} {r} {red}~~~~{r}\r\n",
                "{gs}2{r}   bar,\r\n",
                "{gs} {r} {red}~~~~~~{r}\r\n",
                "{gs}3{r} )\r\n",
                "{gs} {r} {red}~{r}\r\n",
                "\r\n\r\n",
                "==== a.ts (1 errors) ====\r\n",
                "    foo(\r\n",
                "    ~~~~\r\n",
                "      bar,\r\n",
                "    ~~~~~~\r\n",
                "    )\r\n",
                "    ~\r\n",
                "!!! error TS2554: Expected 0 arguments.",
                "\r\nFound 1 error in a.ts{g}:1{r}\r\n\r\n",
            ),
            c = CYAN,
            r = RESET,
            y = YELLOW,
            red = RED,
            g = GREY,
            gs = GUTTER_STYLE
        );
        assert_eq!(render_pretty(&b).expect("render"), expected);
    }

    // --- write_code_snippet branches no in-corpus pretty baseline reaches ---

    #[test]
    fn write_code_snippet_over_five_lines_elides_middle() {
        // A >5-line span (last_line - first_line >= 4) shows the first 2 and last 2
        // lines with an ellipsis gutter between them (diagnosticwriter.go:187-197).
        // gutter_width widens to max(len("..."), digits(last_line + 1)) = max(3, 1)
        // = 3 (:180-182), so the line numbers right-justify to width 3. Exercised on
        // write_code_snippet directly to isolate the elision branch.
        let src: Vec<String> = ["aaa", "bbb", "ccc", "ddd", "eee", "fff"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        // The span (0, 23) covers all six 3-char lines of "aaa\n…\nfff".
        let mut out = String::new();
        write_code_snippet(&mut out, &src, 0, 23, RED, "");
        let expected = format!(
            concat!(
                "\r\n",
                "{gs}  1{r} aaa\r\n",
                "{gs}   {r} {red}~~~{r}",
                "\r\n",
                "{gs}  2{r} bbb\r\n",
                "{gs}   {r} {red}~~~{r}",
                "\r\n",
                "{gs}...{r} \r\n",
                "{gs}  5{r} eee\r\n",
                "{gs}   {r} {red}~~~{r}",
                "\r\n",
                "{gs}  6{r} fff\r\n",
                "{gs}   {r} {red}~~~{r}",
            ),
            r = RESET,
            red = RED,
            gs = GUTTER_STYLE
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn write_code_snippet_zero_length_squiggles_one_char() {
        // A zero-length span squiggles a single character: last_char is bumped by
        // one (diagnosticwriter.go:172-174) so exactly one tilde is emitted, at the
        // start column (byte 1 of "abc" → one leading space, then one tilde).
        let src = vec!["abc".to_string()];
        let mut out = String::new();
        write_code_snippet(&mut out, &src, 1, 0, RED, "");
        let expected = format!(
            concat!("\r\n", "{gs}1{r} abc\r\n", "{gs} {r} {red} ~{r}",),
            r = RESET,
            red = RED,
            gs = GUTTER_STYLE
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn standalone_render_astral_utf16_vs_rune_squiggle() {
        // Crux unit-system distinction, end to end: U+10437 (𐐷) is one rune but two
        // UTF-16 code units. The colored top block counts the squiggle in UTF-16
        // units (writeCodeSnippet / core.UTF16Len → two tildes); the plain middle
        // counts runes (error_baseline.go's utf8.RuneCountInString → one tilde).
        let b = PrettyBaseline {
            base: ParsedBaseline {
                diags: vec![diag(
                    Some("a.ts"),
                    Some(Loc::Numbered { line: 1, col: 1 }),
                    2304,
                    &["Cannot find name."],
                    &[],
                )],
                // The astral char is 4 bytes; the span (0, 4) covers exactly it.
                sections: vec![section("a.ts", &["\u{10437};"], &[(0, 0, 4)])],
            },
            related_lens: vec![vec![]],
        };
        let expected = format!(
            concat!(
                "{c}a.ts{r}:{y}1{r}:{y}1{r} - {red}error{r}{g} TS2304: {r}Cannot find name.\r\n",
                "\r\n",
                "{gs}1{r} \u{10437};\r\n",
                "{gs} {r} {red}~~{r}\r\n",
                "\r\n\r\n",
                "==== a.ts (1 errors) ====\r\n",
                "    \u{10437};\r\n",
                "    ~\r\n",
                "!!! error TS2304: Cannot find name.",
                "\r\nFound 1 error in a.ts{g}:1{r}\r\n\r\n",
            ),
            c = CYAN,
            r = RESET,
            y = YELLOW,
            red = RED,
            g = GREY,
            gs = GUTTER_STYLE
        );
        assert_eq!(render_pretty(&b).expect("render"), expected);
        assert!(self_assertion_violations(&b.base).is_empty());
    }

    #[test]
    fn standalone_render_level_two_message_chain() {
        // A message chain nested two levels deep: the top block joins the head with
        // a 2-space (level-1) then 4-space (level-2) continuation via
        // write_flattened_message (diagnosticwriter.go:271-281, "  " per level), and
        // each non-empty line re-renders `!!!`-prefixed in the plain middle.
        let b = PrettyBaseline {
            base: ParsedBaseline {
                diags: vec![diag(
                    Some("a.ts"),
                    Some(Loc::Numbered { line: 1, col: 1 }),
                    2322,
                    &[
                        "Type 'A' is not assignable to type 'B'.",
                        "  The types are incompatible.",
                        "    Property 'x' is missing.",
                    ],
                    &[],
                )],
                sections: vec![section("a.ts", &["x;"], &[(0, 0, 1)])],
            },
            related_lens: vec![vec![]],
        };
        let expected = format!(
            concat!(
                "{c}a.ts{r}:{y}1{r}:{y}1{r} - {red}error{r}{g} TS2322: {r}Type 'A' is not assignable to type 'B'.\r\n",
                "  The types are incompatible.\r\n",
                "    Property 'x' is missing.\r\n",
                "\r\n",
                "{gs}1{r} x;\r\n",
                "{gs} {r} {red}~{r}\r\n",
                "\r\n\r\n",
                "==== a.ts (1 errors) ====\r\n",
                "    x;\r\n",
                "    ~\r\n",
                "!!! error TS2322: Type 'A' is not assignable to type 'B'.\r\n",
                "!!! error TS2322:   The types are incompatible.\r\n",
                "!!! error TS2322:     Property 'x' is missing.",
                "\r\nFound 1 error in a.ts{g}:1{r}\r\n\r\n",
            ),
            c = CYAN,
            r = RESET,
            y = YELLOW,
            red = RED,
            g = GREY,
            gs = GUTTER_STYLE
        );
        assert_eq!(render_pretty(&b).expect("render"), expected);
        assert!(self_assertion_violations(&b.base).is_empty());
    }

    #[test]
    fn standalone_summary_tabular_multi_digit_count() {
        // The tabular errors display right-justifies each file's error count in a
        // column max(len("Errors"), digits(maxCount)) = max(6, 2) = 6 wide
        // (diagnosticwriter.go:423-437). A 10-error file exercises the multi-digit
        // count "    10" (four spaces + two digits) beside the single-digit "     1".
        // Driven through render_pretty_summary (the summary never reads sections).
        let mut diags: Vec<Diag> = Vec::new();
        for _ in 0..10 {
            diags.push(diag(
                Some("a.ts"),
                Some(Loc::Numbered { line: 3, col: 1 }),
                2304,
                &["e"],
                &[],
            ));
        }
        // Only the first error's line drives each file's pretty path.
        diags.push(diag(
            Some("b.ts"),
            Some(Loc::Numbered { line: 7, col: 1 }),
            2304,
            &["e"],
            &[],
        ));
        let base = ParsedBaseline {
            diags,
            sections: vec![],
        };

        let mut out = String::new();
        render_pretty_summary(&mut out, &base);
        let expected = format!(
            concat!(
                "\r\n",
                "Found 11 errors in 2 files.\r\n",
                "\r\n",
                "Errors  Files\r\n",
                "    10  a.ts{g}:3{r}\r\n",
                "     1  b.ts{g}:7{r}\r\n",
                "\r\n",
            ),
            g = GREY,
            r = RESET
        );
        assert_eq!(out, expected);
    }
}
