//! Parse test262 YAML frontmatter from test files.
//!
//! Uses string operations instead of a YAML parser to avoid adding dependencies.
//! The frontmatter format is simple enough to parse manually.

#![allow(dead_code)] // Some methods are useful for future expansion

/// Syntactic proposals tsv deliberately does not parse, by their test262
/// `features:` name. A test requiring one of these is not a tsv conformance gap
/// — it exercises an unimplemented proposal — so the runner skips it instead of
/// scoring it as a positive failure (and the differential manifest drops it too,
/// since both share `classify`).
///
/// Currently empty: the import-phase proposals (`source-phase-imports`
/// with its `…-module-source` companion, and `import-defer`) are now parsed —
/// `import source …` / `import defer …` and `import.source(…)` / `import.defer(…)`
/// — so their tests are graded. Add a `features:` name here when tsv meets a new
/// proposal it doesn't yet parse; drop it again once the syntax lands.
const UNIMPLEMENTED_FEATURES: &[&str] = &[];

/// Parsed frontmatter from a test262 test file.
#[derive(Debug, Default)]
pub struct Frontmatter {
    /// Feature flags required by the test (e.g., "BigInt", "class-fields-private")
    pub features: Vec<String>,
    /// Execution flags (e.g., "async", "module", "onlyStrict")
    pub flags: Vec<String>,
    /// For negative tests: the phase where the error should occur
    pub negative_phase: Option<String>,
    /// For negative tests: the expected error type
    pub negative_type: Option<String>,
}

impl Frontmatter {
    /// Check if this is a negative test that should fail at parse time.
    pub fn is_negative_parse(&self) -> bool {
        self.negative_phase.as_deref() == Some("parse")
    }

    /// Check if this is a negative test that should fail at runtime.
    pub fn is_negative_runtime(&self) -> bool {
        self.negative_phase.as_deref() == Some("runtime")
    }

    /// Check if this is a negative test that should fail at module resolution.
    pub fn is_negative_resolution(&self) -> bool {
        self.negative_phase.as_deref() == Some("resolution")
    }

    /// Check if this test should be skipped (runtime/resolution negative tests).
    pub fn should_skip(&self) -> bool {
        self.is_negative_runtime() || self.is_negative_resolution()
    }

    /// Check if this test uses ES modules.
    pub fn is_module(&self) -> bool {
        self.flags.iter().any(|f| f == "module")
    }

    /// Check if this test requires non-strict (sloppy) mode.
    pub fn requires_sloppy_mode(&self) -> bool {
        self.flags.iter().any(|f| f == "noStrict")
    }

    /// Check if this test's source is used verbatim (`flags: [raw]`): no harness
    /// files, no `"use strict"` transform. Per test262/INTERPRETING.md a raw test
    /// runs **once, in non-strict mode only**. Most exercise mode-independent
    /// syntax (hashbang, HTML-close comments, directive prologues) tsv grades
    /// correctly anyway; the runner skips only the ones whose verdict genuinely
    /// needs sloppy semantics (see `runner::is_sloppy_only_raw`).
    pub fn is_raw(&self) -> bool {
        self.flags.iter().any(|f| f == "raw")
    }

    /// Check if this test requires strict mode only.
    pub fn requires_strict_mode(&self) -> bool {
        self.flags.iter().any(|f| f == "onlyStrict")
    }

    /// The first `features:` entry naming a proposal tsv does not implement
    /// (see `UNIMPLEMENTED_FEATURES`), or `None` if the test needs only
    /// implemented syntax. The runner skips a `Some` test rather than grading
    /// the unimplemented proposal as a failure.
    pub fn requires_unimplemented_feature(&self) -> Option<&'static str> {
        self.features
            .iter()
            .find_map(|f| UNIMPLEMENTED_FEATURES.iter().copied().find(|&u| u == f))
    }
}

/// Parse frontmatter from test file content.
///
/// Returns `None` if the frontmatter markers are not found.
/// Returns `Some(Frontmatter)` with whatever fields could be parsed,
/// even if some fields are missing or malformed.
pub fn parse(content: &str) -> Option<Frontmatter> {
    // Find the frontmatter markers
    let start_marker = "/*---";
    let end_marker = "---*/";

    let start = content.find(start_marker)?;
    let end = content.find(end_marker)?;

    if end <= start {
        return None;
    }

    let yaml = &content[start + start_marker.len()..end];

    let mut frontmatter = Frontmatter::default();
    let mut in_negative = false;
    let mut list_target: Option<ListField> = None;

    for line in yaml.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Block-sequence item ("  - value") for the list field opened above.
        if let Some(target) = list_target {
            if let Some(item) = trimmed.strip_prefix('-') {
                let item = item.trim().trim_matches('"').trim_matches('\'');
                if !item.is_empty() {
                    match target {
                        ListField::Features => frontmatter.features.push(item.to_string()),
                        ListField::Flags => frontmatter.flags.push(item.to_string()),
                    }
                }
                continue;
            }
            // A non-item line closes the block sequence; fall through to re-process it.
            list_target = None;
        }

        // features: [a, b, c]  (inline)  or  features:  (block list on following lines)
        if trimmed.starts_with("features:") {
            match parse_list_field(trimmed) {
                ListValue::Inline(items) => frontmatter.features = items,
                ListValue::Block => list_target = Some(ListField::Features),
            }
        }
        // flags: [a, b, c]  (inline)  or  flags:  (block list)
        else if trimmed.starts_with("flags:") {
            match parse_list_field(trimmed) {
                ListValue::Inline(items) => frontmatter.flags = items,
                ListValue::Block => list_target = Some(ListField::Flags),
            }
        }
        // negative: (start of block)
        else if trimmed.starts_with("negative:") {
            in_negative = true;
            // Handle inline negative: { phase: parse, type: SyntaxError } (rare but possible)
            if trimmed.contains("phase:") {
                frontmatter.negative_phase = extract_inline_field(trimmed, "phase");
            }
            if trimmed.contains("type:") {
                frontmatter.negative_type = extract_inline_field(trimmed, "type");
            }
        }
        // phase: parse (inside negative block)
        else if in_negative && trimmed.starts_with("phase:") {
            frontmatter.negative_phase = parse_value(trimmed);
        }
        // type: SyntaxError (inside negative block)
        else if in_negative && trimmed.starts_with("type:") {
            frontmatter.negative_type = parse_value(trimmed);
        }
        // Exit negative block on non-indented, non-empty line that's a new field
        else if in_negative
            && !line.starts_with(' ')
            && !line.starts_with('\t')
            && trimmed.contains(':')
        {
            in_negative = false;
        }
    }

    Some(frontmatter)
}

/// Which list field a YAML block sequence is being accumulated into.
#[derive(Clone, Copy)]
enum ListField {
    Features,
    Flags,
}

/// The shape of a `features:` / `flags:` line.
enum ListValue {
    /// Inline `[a, b, c]` (possibly empty).
    Inline(Vec<String>),
    /// No inline array — a YAML block sequence follows on subsequent `- item` lines.
    Block,
}

/// Classify a `features:` / `flags:` line as an inline array or the header of a
/// block sequence. test262 uses both forms (`flags: [onlyStrict]` inline,
/// `features:\n  - class` block).
fn parse_list_field(line: &str) -> ListValue {
    if line.contains('[') {
        ListValue::Inline(parse_array(line))
    } else {
        ListValue::Block
    }
}

/// Parse a YAML array from a line like "features: [a, b, c]"
fn parse_array(line: &str) -> Vec<String> {
    let Some(start) = line.find('[') else {
        return Vec::new();
    };
    let Some(end) = line.find(']') else {
        return Vec::new();
    };

    if end <= start {
        return Vec::new();
    }

    let content = &line[start + 1..end];

    content
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            // Remove quotes if present
            s.trim_matches('"').trim_matches('\'').to_string()
        })
        .collect()
}

/// Parse a value from a line like "phase: parse"
fn parse_value(line: &str) -> Option<String> {
    let colon_pos = line.find(':')?;
    let value = line[colon_pos + 1..].trim();
    if value.is_empty() {
        None
    } else {
        // Remove quotes if present
        Some(value.trim_matches('"').trim_matches('\'').to_string())
    }
}

/// Extract a field value from an inline object like "negative: { phase: parse, type: SyntaxError }"
fn extract_inline_field(line: &str, field: &str) -> Option<String> {
    let pattern = format!("{field}:");
    let field_start = line.find(&pattern)?;
    let after_colon = &line[field_start + pattern.len()..];

    // Find the value - either until comma, closing brace, or end of string
    let value_end = after_colon.find([',', '}']).unwrap_or(after_colon.len());

    let value = after_colon[..value_end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.trim_matches('"').trim_matches('\'').to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let content = r"// Copyright
/*---
esid: sec-example
description: Test description
---*/
var x = 1;
";

        let fm = parse(content).unwrap();
        assert!(fm.features.is_empty());
        assert!(fm.flags.is_empty());
        assert!(fm.negative_phase.is_none());
    }

    #[test]
    fn test_parse_features() {
        let content = r"/*---
features: [BigInt, class-fields-private]
---*/";

        let fm = parse(content).unwrap();
        assert_eq!(fm.features, vec!["BigInt", "class-fields-private"]);
    }

    #[test]
    fn test_parse_flags() {
        let content = r"/*---
flags: [async, module, onlyStrict]
---*/";

        let fm = parse(content).unwrap();
        assert_eq!(fm.flags, vec!["async", "module", "onlyStrict"]);
    }

    #[test]
    fn test_parse_features_block_form() {
        // test262 commonly writes `features` as a YAML block sequence, with an
        // inline `flags` line right after it.
        let content = r"/*---
features:
  - class
  - class-fields-private
flags: [onlyStrict]
---*/";

        let fm = parse(content).unwrap();
        assert_eq!(fm.features, vec!["class", "class-fields-private"]);
        // The inline `flags` line after the block sequence still parses.
        assert_eq!(fm.flags, vec!["onlyStrict"]);
    }

    #[test]
    fn test_parse_flags_block_form() {
        let content = r"/*---
flags:
  - async
  - module
---*/";

        let fm = parse(content).unwrap();
        assert_eq!(fm.flags, vec!["async", "module"]);
        assert!(fm.is_module());
    }

    #[test]
    fn test_parse_negative_parse() {
        let content = r"/*---
negative:
  phase: parse
  type: SyntaxError
---*/
$DONOTEVALUATE();
";

        let fm = parse(content).unwrap();
        assert!(fm.is_negative_parse());
        assert_eq!(fm.negative_type.as_deref(), Some("SyntaxError"));
    }

    #[test]
    fn test_parse_negative_runtime() {
        let content = r"/*---
negative:
  phase: runtime
  type: TypeError
---*/";

        let fm = parse(content).unwrap();
        assert!(fm.is_negative_runtime());
        assert!(fm.should_skip());
    }

    #[test]
    fn test_is_module() {
        let content = r"/*---
flags: [module]
---*/";

        let fm = parse(content).unwrap();
        assert!(fm.is_module());
    }

    #[test]
    fn test_no_frontmatter() {
        let content = "var x = 1;";
        assert!(parse(content).is_none());
    }

    #[test]
    fn test_requires_unimplemented_feature() {
        // The import-phase proposals are now parsed, so they are graded, not
        // filtered — `UNIMPLEMENTED_FEATURES` is empty.
        let source_phase = r"/*---
features: [source-phase-imports, dynamic-import]
flags: [generated, async]
---*/";
        assert!(
            parse(source_phase)
                .unwrap()
                .requires_unimplemented_feature()
                .is_none()
        );

        let import_defer = r"/*---
features: [import-defer, dynamic-import]
---*/";
        assert!(
            parse(import_defer)
                .unwrap()
                .requires_unimplemented_feature()
                .is_none()
        );

        // Plain dynamic-import is implemented — not filtered.
        let plain = r"/*---
features: [dynamic-import]
---*/";
        assert!(
            parse(plain)
                .unwrap()
                .requires_unimplemented_feature()
                .is_none()
        );

        // No features at all.
        let bare = r"/*---
esid: sec-example
---*/";
        assert!(
            parse(bare)
                .unwrap()
                .requires_unimplemented_feature()
                .is_none()
        );
    }
}
