//! Parse a corpus test's `// @key: value` directives and split it into file
//! units, faithful to tsgo's `test_case_parser.go`.
//!
//! Two products: the flat **settings** map (`extractCompilerSettings` — every
//! directive anywhere in the file, keys lowercased, last write wins) that drives
//! variant expansion; and the ordered **units** (`makeUnitsFromTest` /
//! `ParseTestFilesAndSymlinks` in non-implicit mode) that `@filename` splits.
//!
//! The directive grammar is tsgo's `optionRegex`
//! (`(?m)^//\s*@(\w+)\s*:\s*([^\r\n]*)`), reproduced without a regex engine: a
//! match anchors at a physical line start (after `\n`, never a lone `\r`), the
//! value runs to the next `\r`/`\n`. This hand port is byte-for-byte equivalent to
//! the regex over the pinned corpus.
//
// tsgo: internal/testrunner/test_case_parser.go optionRegex / extractCompilerSettings
// tsgo: internal/testrunner/test_case_parser.go ParseTestFilesAndSymlinksWithOptions
// tsgo: internal/testrunner/compiler_runner.go newCompilerTest (root-vs-otherFiles rule)

use crate::tsc_conformance::options_meta::is_config_file_name;
use std::collections::BTreeMap;

/// One file unit split out of a test (`@filename` boundaries), or the whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// The unit's declared name (from `@filename`, or the test basename).
    pub name: String,
    /// The unit's source text — physical lines joined with `\n`, leading blank
    /// lines dropped (tsgo's `Len() != 0` accumulation).
    pub content: String,
}

/// Split `content` into physical lines on `\r?\n` (tsgo's `lineDelimiter`). The
/// `\r` before a `\n` is part of the separator; a lone `\r` is not — so a
/// CR-only file is a single line.
fn split_lines(content: &str) -> Vec<&str> {
    let bytes = content.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            lines.push(&content[start..end]);
            start = i + 1;
        }
        i += 1;
    }
    lines.push(&content[start..]);
    lines
}

/// Skip a run of horizontal whitespace (space, tab, form-feed) — the harness's
/// intra-line `\s*`. `\r`/`\n` never appear here in the corpus, so restricting to
/// horizontal whitespace is exact.
fn skip_hspace(s: &str) -> &str {
    s.trim_start_matches([' ', '\t', '\u{0c}'])
}

/// Parse a physical line as a directive, returning `(lowercased key, raw value)`
/// where the value runs from after `:` to the next `\r`. `None` if the line is
/// not a directive.
fn parse_directive(seg: &str) -> Option<(String, &str)> {
    let rest = skip_hspace(seg.strip_prefix("//")?);
    let rest = rest.strip_prefix('@')?;
    let key_end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if key_end == 0 {
        return None;
    }
    let key = &rest[..key_end];
    let rest = skip_hspace(&rest[key_end..]);
    let rest = rest.strip_prefix(':')?;
    let rest = skip_hspace(rest);
    let value = rest.split_once('\r').map_or(rest, |(v, _)| v);
    Some((key.to_ascii_lowercase(), value))
}

/// Extract the flat compiler-settings map (`extractCompilerSettings`): every
/// directive in the file, keys lowercased, value `TrimSpace` then one trailing
/// `;` stripped, last write winning.
#[must_use]
pub fn extract_settings(content: &str) -> BTreeMap<String, String> {
    let mut settings = BTreeMap::new();
    for seg in split_lines(content) {
        if let Some((key, value)) = parse_directive(seg) {
            let trimmed = value.trim();
            let cleaned = trimmed.strip_suffix(';').unwrap_or(trimmed);
            settings.insert(key, cleaned.to_string());
        }
    }
    settings
}

/// The base file name (final `/`-separated component).
fn base_file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Split a compiler test into file units (`makeUnitsFromTest` in non-implicit
/// mode). `@filename` starts a new unit; every other directive line is consumed
/// (not content); content accumulates with `\n`, leading blanks dropped. A test
/// with no `@filename` yields one unit named after the test file.
///
/// `test_filename` is the corpus file's path (or basename).
#[must_use]
pub fn split_units(content: &str, test_filename: &str) -> Vec<Unit> {
    let mut units: Vec<Unit> = Vec::new();
    let mut cur_name = String::new();
    let mut cur = String::new();

    for seg in split_lines(content) {
        if let Some((key, value)) = parse_directive(seg) {
            if key != "filename" {
                // currentDirectory / symlink / link / global options are all
                // consumed here — never part of a unit's content.
                continue;
            }
            let name = value.trim().to_string();
            if cur_name.is_empty() {
                // First `@filename`: any accumulated (comment-only) content is
                // discarded, matching the harness's Reset.
                cur.clear();
            } else {
                units.push(Unit {
                    name: cur_name.clone(),
                    content: std::mem::take(&mut cur),
                });
            }
            cur_name = name;
        } else {
            if !cur.is_empty() {
                cur.push('\n');
            }
            cur.push_str(seg);
        }
    }

    if units.is_empty() && cur_name.is_empty() {
        cur_name = base_file_name(test_filename);
    }
    units.push(Unit {
        name: cur_name,
        content: cur,
    });
    units
}

/// A test's units classified into baseline-section order
/// (`Concatenate(tsConfigFiles, toBeCompiled, otherFiles)`).
#[derive(Debug, Clone)]
pub struct Classified {
    /// The recognized tsconfig/jsconfig unit, if any (emitted first).
    pub tsconfig: Option<Unit>,
    /// Units compiled directly.
    pub to_be_compiled: Vec<Unit>,
    /// Other files brought in by reference.
    pub other_files: Vec<Unit>,
    /// Whether a tsconfig unit was present (its `FileNames` glob resolution is out
    /// of scope, so `to_be_compiled`/`other_files` split is not authoritative in
    /// that case — all non-config units land in `to_be_compiled`).
    pub tsconfig_unresolved: bool,
}

impl Classified {
    /// The units in baseline-section order.
    #[must_use]
    pub fn section_order(&self) -> Vec<&Unit> {
        self.tsconfig
            .iter()
            .chain(self.to_be_compiled.iter())
            .chain(self.other_files.iter())
            .collect()
    }
}

/// Whether `content` contains a triple-slash reference (`reference` + one
/// whitespace + `path`), tsgo's `referencesRegex`.
fn contains_reference_path(content: &str) -> bool {
    let bytes = content.as_bytes();
    let mut from = 0;
    while let Some(pos) = content[from..].find("reference") {
        let after = from + pos + "reference".len();
        if let Some(&c) = bytes.get(after)
            && (c as char).is_ascii_whitespace()
            && content[after + 1..].starts_with("path")
        {
            return true;
        }
        from += pos + 1;
    }
    false
}

/// Classify a test's units into baseline-section order, applying the last-unit
/// `require(` / triple-slash-reference heuristic (`newCompilerTest`). A tsconfig
/// unit is pulled out first; its `FileNames` resolution is out of scope, so with a
/// tsconfig every remaining unit is reported as `to_be_compiled`.
///
/// `settings` supplies `noImplicitReferences` (its presence forces the last-unit
/// split).
#[must_use]
pub fn classify_units(units: Vec<Unit>, settings: &BTreeMap<String, String>) -> Classified {
    // Pull out the first tsconfig/jsconfig unit.
    let mut tsconfig = None;
    let mut rest: Vec<Unit> = Vec::with_capacity(units.len());
    for unit in units {
        if tsconfig.is_none() && is_config_file_name(&unit.name) {
            tsconfig = Some(unit);
        } else {
            rest.push(unit);
        }
    }

    if tsconfig.is_some() {
        return Classified {
            tsconfig,
            to_be_compiled: rest,
            other_files: Vec::new(),
            tsconfig_unresolved: true,
        };
    }

    let force_last = settings
        .get("noimplicitreferences")
        .is_some_and(|v| !v.is_empty())
        || rest
            .last()
            .is_some_and(|u| u.content.contains("require(") || contains_reference_path(&u.content));

    if force_last && rest.len() > 1 {
        let last = rest.pop().unwrap_or_else(|| Unit {
            name: String::new(),
            content: String::new(),
        });
        Classified {
            tsconfig: None,
            to_be_compiled: vec![last],
            other_files: rest,
            tsconfig_unresolved: false,
        }
    } else {
        Classified {
            tsconfig: None,
            to_be_compiled: rest,
            other_files: Vec::new(),
            tsconfig_unresolved: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_lines_crlf_lf_and_cr() {
        assert_eq!(split_lines("a\r\nb\nc"), vec!["a", "b", "c"]);
        assert_eq!(split_lines("a\nb\n"), vec!["a", "b", ""]);
        // A lone CR is not a separator.
        assert_eq!(split_lines("a\rb"), vec!["a\rb"]);
    }

    #[test]
    fn directive_parsing() {
        assert_eq!(
            parse_directive("// @target: es5"),
            Some(("target".to_string(), "es5"))
        );
        assert_eq!(
            parse_directive("//@Module:  CommonJS  "),
            Some(("module".to_string(), "CommonJS  "))
        );
        assert_eq!(parse_directive("const x = 1;"), None);
        assert_eq!(parse_directive("// not a directive"), None);
        // Value stops at a CR inside a CR-only line.
        assert_eq!(
            parse_directive("//@target: es6\r// rest"),
            Some(("target".to_string(), "es6"))
        );
    }

    #[test]
    fn settings_last_wins_and_trims() {
        // Last write wins; TrimSpace then one trailing `;` stripped.
        let s = extract_settings("// @target: es5\n// @target: es2015;\ncode;");
        assert_eq!(s.get("target").map(String::as_str), Some("es2015"));
    }

    #[test]
    fn settings_strip_is_trim_then_semicolon_only() {
        // Faithful to tsgo: TrimSuffix(TrimSpace(value), ";") does NOT re-trim, so
        // a space before the `;` survives (harmless — variant splitting re-trims).
        let s = extract_settings("// @target: es2015 ;\n");
        assert_eq!(s.get("target").map(String::as_str), Some("es2015 "));
    }

    #[test]
    fn single_file_names_after_test() {
        let units = split_units("const x = 1;", "foo.ts");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].name, "foo.ts");
        assert_eq!(units[0].content, "const x = 1;");
    }

    #[test]
    fn multi_file_split_and_leading_blank_drop() {
        // A leading blank line before the first content line is dropped.
        let src = "// @filename: a.ts\n\nlet a = 1;\n// @filename: b.ts\nlet b = 2;";
        let units = split_units(src, "test.ts");
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].name, "a.ts");
        assert_eq!(units[0].content, "let a = 1;");
        assert_eq!(units[1].name, "b.ts");
        assert_eq!(units[1].content, "let b = 2;");
    }

    #[test]
    fn classify_last_unit_on_reference() {
        let src = "// @filename: a.ts\nlet a = 1;\n// @filename: b.ts\n/// <reference path=\"a.ts\" />\nlet b = 2;";
        let units = split_units(src, "test.ts");
        let c = classify_units(units, &BTreeMap::new());
        assert!(c.tsconfig.is_none());
        assert_eq!(c.to_be_compiled.len(), 1);
        assert_eq!(c.to_be_compiled[0].name, "b.ts");
        assert_eq!(c.other_files.len(), 1);
    }

    #[test]
    fn classify_all_when_no_reference() {
        let src = "// @filename: a.ts\nlet a = 1;\n// @filename: b.ts\nlet b = 2;";
        let units = split_units(src, "test.ts");
        let c = classify_units(units, &BTreeMap::new());
        assert_eq!(c.to_be_compiled.len(), 2);
        assert!(c.other_files.is_empty());
    }

    #[test]
    fn classify_pulls_tsconfig_first() {
        let src = "// @filename: tsconfig.json\n{}\n// @filename: a.ts\nlet a = 1;";
        let units = split_units(src, "test.ts");
        let c = classify_units(units, &BTreeMap::new());
        assert!(c.tsconfig.is_some());
        assert!(c.tsconfig_unresolved);
        assert_eq!(c.section_order().len(), 2);
    }
}
