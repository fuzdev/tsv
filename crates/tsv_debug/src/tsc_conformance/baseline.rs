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
//! A later slice extends this module with full section parsing + rendering for a
//! round-trip check against tsgo; the summary parser here is the shared seed.

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
}
