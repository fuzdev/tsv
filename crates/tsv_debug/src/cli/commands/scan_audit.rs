use argh::FromArgs;
use std::path::{Path, PathBuf};

/// Guard against new raw `str::find` / `str::rfind` delimiter scans over source.
///
/// The "Comment-Aware Delimiter Scans" effort consolidated source re-scanning
/// onto one trivia-aware cursor (`tsv_lang::source_scan`), because a raw
/// `self.source[..].find(delim)` can match the glyph **inside an enclosed comment
/// or string** — mis-anchoring the scan and dropping comments (silent data loss).
/// The class kept recurring because the easy path (`str::find`) is the wrong path.
/// This audit removes the easy wrong path: it flags every `str::find` / `str::rfind`
/// call (with a non-closure pattern) in the language crates, and fails on any that
/// isn't in the reviewed allow-list below.
///
/// A flagged site must either move onto the cursor
/// (`find_char` / `find_top_level_delim` / `match_bracket` / `skip_trivia`) or be
/// added to `ALLOW` with a category — a conscious, reviewed decision rather than a
/// silent reintroduction. Iterator/predicate `.find(|…|)` (a closure pattern, not a
/// delimiter literal) is excluded — it isn't a `str::find`-over-source scan. Hand
/// byte-loops are out of automated scope (undetectable by a line scan, and far
/// rarer to write accidentally than `.find`); the cursor is their sanctioned home.
///
/// Pure Rust — no Deno. Part of `deno task check` (via `deno task scan:audit`).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "scan_audit")]
pub struct ScanAuditCommand {
    /// list every detected `find`/`rfind` site (path:line: code) instead of auditing
    #[argh(switch)]
    list: bool,

    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,
}

/// Crate `src` roots scanned — the parser/printer/convert code that re-scans source.
/// Tooling/binding crates (tsv_debug, tsv_cli, tsv_ffi, tsv_wasm) don't re-scan
/// source for delimiters, so they're out of scope.
const CRATE_ROOTS: &[&str] = &[
    "crates/tsv_lang/src",
    "crates/tsv_ts/src",
    "crates/tsv_css/src",
    "crates/tsv_svelte/src",
];

/// One reviewed, sanctioned `find`/`rfind` site: `(crate-relative path, exact
/// trimmed source line, category)`. A candidate is allowed iff it matches an entry
/// on path **and** exact trimmed line — so reformatting a scanned line forces a
/// re-review (intended).
type Allow = (&'static str, &'static str, &'static str);

/// The reviewed allow-list. Generated from `scan_audit --list`, each entry
/// classified. Categories (the triage signal):
///
/// Benign (not the comment/string-vulnerable delimiter class):
/// - `comment-marker` — finds `/*` or `*/`: the target IS the comment delimiter, so
///   there's no "glyph hidden in a comment" risk.
/// - `newline` — line/column tracking over source; a `\n` isn't hidden inside trivia
///   in a way that breaks line-start math.
/// - `non-source` — over an output buffer or rendered doc text, not source.
/// - `number-literal` — content of an isolated numeric literal (no comments inside).
/// - `css-value` — `(`/`)` or function name of a `url()`/color/function value token
///   (per the lore's "url/color paren finds — not candidates").
/// - `at-rule-range` — connector keyword in a normalized CSS at-rule range prelude.
/// - `attr-name` — Svelte attribute-name `:` split (directive prefix).
/// - `template-marker` — `${` of a template-literal type.
/// - `jsdoc-tag` — scans a value/comment string for `@type`/`@satisfies` cast tags.
/// - `keyword-comment-aware` — keyword scan that already hand-rolls a comment skip
///   (the `from` finder; cleanup-pending cursor migration, not a live bug).
///
/// Tracked delimiter-over-source scans (real class, allow-listed but on the books in
/// the lore's "Comment-Aware Delimiter Scans" stage-4 inventory):
/// - `delimiter-deferred-bug` — a real comment-vulnerable scan tracked as a deferred
///   bug (switch `)`→`{`; default+empty-import). Allow-listed, NOT fixed here.
/// - `delimiter-latent` — byte-correct today but comment-blind; migrate-or-keep
///   tracked in the lore.
///
/// Closure-pattern `.find(|…|)` (iterator/predicate finds) are excluded by the
/// detector and never reach this list.
const ALLOW: &[Allow] = &[
    // ── tsv_css ──────────────────────────────────────────────────────────────
    (
        "tsv_css/src/ast/convert.rs",
        "if let Some(comment_idx) = before_colon.find(\"/*\") {",
        "comment-marker",
    ),
    (
        "tsv_css/src/ast/convert.rs",
        "if let Some(end_rel) = rest[2..].find(\"*/\") {",
        "comment-marker",
    ),
    (
        "tsv_css/src/ast/convert.rs",
        ".find(\"*/\")",
        "comment-marker",
    ),
    (
        "tsv_css/src/ast/convert.rs",
        "raw.find('(').map_or(span.end, |i| span.start + i as u32)",
        "css-value",
    ),
    (
        "tsv_css/src/parser/value/mod.rs",
        "if let Some(paren_pos) = s.find('(')",
        "css-value",
    ),
    (
        "tsv_css/src/printer/atrules.rs",
        ".find(&format!(\" {connector_keyword} \"))",
        "at-rule-range",
    ),
    (
        "tsv_css/src/printer/atrules.rs",
        ".or_else(|| range_lower.find(connector_keyword));",
        "at-rule-range",
    ),
    (
        "tsv_css/src/printer/declarations.rs",
        ".find(\"*/\")",
        "comment-marker",
    ),
    (
        "tsv_css/src/printer/value_normalization/colors.rs",
        "if let Some(open_paren) = raw.find('(') {",
        "css-value",
    ),
    (
        "tsv_css/src/printer/value_normalization/mod.rs",
        "if let Some(comment_start) = property_part.find(\"/*\") {",
        "comment-marker",
    ),
    (
        "tsv_css/src/printer/value_normalization/mod.rs",
        "if let Some(comment_end_rel) = property_part[comment_start..].find(\"*/\") {",
        "comment-marker",
    ),
    (
        "tsv_css/src/printer/value_normalization/numbers.rs",
        "let (mantissa, exponent) = match num.find(['e', 'E']) {",
        "number-literal",
    ),
    (
        "tsv_css/src/printer/value_normalization/splitting.rs",
        "let func_start = source.find(func_name)?;",
        "css-value",
    ),
    (
        "tsv_css/src/printer/value_normalization/splitting.rs",
        "let open_paren = after_name.find('(')?;",
        "css-value",
    ),
    (
        "tsv_css/src/url.rs",
        "let open = raw.find('(')?;",
        "css-value",
    ),
    (
        "tsv_css/src/url.rs",
        "let close = raw.rfind(')')?;",
        "css-value",
    ),
    // ── tsv_lang ─────────────────────────────────────────────────────────────
    (
        "tsv_lang/src/doc/arena_fits.rs",
        "if let Some(last_newline_pos) = s.rfind('\\n') {",
        "non-source",
    ),
    (
        "tsv_lang/src/doc/arena_render.rs",
        "let trim_start = s.rfind('\\n').map_or(0, |i| i + 1);",
        "non-source",
    ),
    (
        "tsv_lang/src/doc/arena_render.rs",
        "if let Some(last_nl) = s.rfind('\\n') {",
        "non-source",
    ),
    (
        "tsv_lang/src/error.rs",
        "let line_start = source[..position].rfind('\\n').map_or(0, |i| i + 1);",
        "newline",
    ),
    ("tsv_lang/src/error.rs", ".find('\\n')", "newline"),
    (
        "tsv_lang/src/output.rs",
        "let last_newline = self.buffer.rfind('\\n');",
        "non-source",
    ),
    // ── tsv_svelte ───────────────────────────────────────────────────────────
    (
        "tsv_svelte/src/parser/attribute.rs",
        "if let Some(colon_idx) = name_str.find(':') {",
        "attr-name",
    ),
    // ── tsv_ts ───────────────────────────────────────────────────────────────
    (
        "tsv_ts/src/ast/convert/statements.rs",
        ".find(\"export\")",
        "delimiter-latent",
    ),
    (
        "tsv_ts/src/parser/expression.rs",
        "while let Some(rel) = value[from..].find(tag) {",
        "jsdoc-tag",
    ),
    (
        "tsv_ts/src/parser/expression.rs",
        "let Some(open) = self.source[..i - 2].rfind(\"/*\") else {",
        "comment-marker",
    ),
    (
        "tsv_ts/src/printer/expressions/literals.rs",
        "let Some(e_idx) = s.find('e') else {",
        "number-literal",
    ),
    (
        "tsv_ts/src/printer/expressions/literals.rs",
        "let Some(dot) = s.find('.') else {",
        "number-literal",
    ),
    (
        "tsv_ts/src/printer/expressions/literals.rs",
        "if let Some(dot) = s.find('.') {",
        "number-literal",
    ),
    (
        "tsv_ts/src/printer/expressions/template_literal.rs",
        "if let Some(last_nl) = text.rfind('\\n') {",
        "newline",
    ),
    (
        "tsv_ts/src/printer/mod.rs",
        "let line_start = self.source[..pos].rfind('\\n').map_or(0, |i| i + 1);",
        "newline",
    ),
    (
        "tsv_ts/src/printer/mod.rs",
        ".find(keyword)",
        "delimiter-latent",
    ),
    (
        "tsv_ts/src/printer/mod.rs",
        "pos += search[pos..].find('\\n').unwrap_or(search.len() - pos);",
        "newline",
    ),
    (
        "tsv_ts/src/printer/mod.rs",
        ".find(\"*/\")",
        "comment-marker",
    ),
    (
        "tsv_ts/src/printer/statements/control_flow/switch.rs",
        ".find('{')",
        "delimiter-deferred-bug",
    ),
    (
        "tsv_ts/src/printer/statements/control_flow/try_jump.rs",
        ".rfind(\"finally\")",
        "delimiter-latent",
    ),
    (
        "tsv_ts/src/printer/statements/mod.rs",
        ".find(';')",
        "delimiter-latent",
    ),
    (
        "tsv_ts/src/printer/statements/modules/mod.rs",
        ".find('{')",
        "delimiter-deferred-bug",
    ),
    (
        "tsv_ts/src/printer/statements/modules/specifier_list.rs",
        "match text[search_offset..].find(\"from\") {",
        "keyword-comment-aware",
    ),
    (
        "tsv_ts/src/printer/types/literal_types.rs",
        ".rfind(\"${\")",
        "template-marker",
    ),
    (
        "tsv_ts/src/printer/types/type_literal.rs",
        "let semi_offset = source_slice.find(';');",
        "delimiter-latent",
    ),
];

/// A detected `find`/`rfind` call site.
struct Site {
    path: String,
    line_no: usize,
    code: String,
}

impl ScanAuditCommand {
    pub fn run(self) {
        let mut files: Vec<PathBuf> = Vec::new();
        for root in CRATE_ROOTS {
            let dir = Path::new(root);
            if !dir.exists() {
                eprintln!("Error: crate root not found: {root} (run from the repo root)");
                std::process::exit(1);
            }
            collect_rs_files(dir, &mut files);
        }
        files.sort();

        let mut sites: Vec<Site> = Vec::new();
        for file in &files {
            let Ok(text) = std::fs::read_to_string(file) else {
                continue;
            };
            let rel = crate_relative(file);
            scan_file(&rel, &text, &mut sites);
        }
        sites.sort_by(|a, b| (a.path.as_str(), a.line_no).cmp(&(b.path.as_str(), b.line_no)));

        if self.list {
            for s in &sites {
                println!("{}:{}: {}", s.path, s.line_no, s.code);
            }
            eprintln!("\n{} find/rfind site(s)", sites.len());
            return;
        }

        let violations: Vec<&Site> = sites.iter().filter(|s| !is_allowed(s)).collect();
        // Stale allow-list entries: a sanctioned line that no longer exists (a site
        // was migrated onto the cursor, or reformatted). The list must mirror the
        // live sites exactly, so a dead entry fails too — prompting its removal.
        let stale: Vec<&Allow> = ALLOW
            .iter()
            .filter(|(path, line, _)| !sites.iter().any(|s| s.path == *path && s.code == *line))
            .collect();

        if self.json {
            print_json(&sites, &violations, &stale);
        } else {
            print_human(&violations, &stale);
        }

        std::process::exit(i32::from(!violations.is_empty() || !stale.is_empty()));
    }
}

/// A site is allowed iff some `ALLOW` entry has the same path and exact trimmed code.
fn is_allowed(site: &Site) -> bool {
    ALLOW
        .iter()
        .any(|(path, line, _)| *path == site.path && *line == site.code)
}

/// Recursively collect `*.rs` files under `dir`.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Crate-relative path: `crates/tsv_ts/src/foo.rs` → `tsv_ts/src/foo.rs`.
fn crate_relative(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.strip_prefix("crates/").unwrap_or(&s).replace('\\', "/")
}

/// Find every `find`/`rfind` call site in `text`, skipping `#[cfg(test)]` modules,
/// `//`-comment occurrences, and closure-pattern (`.find(|…|)`) calls.
fn scan_file(rel: &str, text: &str, out: &mut Vec<Site>) {
    // Naive brace-depth tracker to skip inline `#[cfg(test)]` modules. Best-effort:
    // these crates' test modules are brace-balanced at the module boundary.
    let mut depth: i32 = 0;
    let mut pending_test = false;
    let mut test_exit: Option<i32> = None;

    for (i, raw) in text.lines().enumerate() {
        let in_test = test_exit.is_some();
        if !in_test {
            if raw.contains("#[cfg(test)]") {
                pending_test = true;
            }
            if pending_test && raw.contains('{') {
                test_exit = Some(depth);
                pending_test = false;
            }
        }

        if !in_test && let Some(code) = qualifying_find_line(raw) {
            out.push(Site {
                path: rel.to_string(),
                line_no: i + 1,
                code,
            });
        }

        depth += brace_delta(raw);
        if let Some(exit) = test_exit
            && depth <= exit
        {
            test_exit = None;
        }
    }
}

/// If `line` carries at least one real delimiter-scan candidate — a `.find(` /
/// `.rfind(` call that is NOT inside a `//` comment and NOT a closure pattern
/// (`.find(|…|)`, i.e. an iterator/predicate find) — return the trimmed line (one
/// entry per source line; the trimmed text is the allow-list key). Else `None`.
fn qualifying_find_line(line: &str) -> Option<String> {
    let comment = line.find("//");
    let bytes = line.as_bytes();
    // Open-paren index of every `.find(` / `.rfind(` occurrence on the line.
    let opens = line
        .match_indices(".find(")
        .map(|(i, m)| i + m.len() - 1)
        .chain(line.match_indices(".rfind(").map(|(i, m)| i + m.len() - 1));
    for open in opens {
        let in_comment = comment.is_some_and(|c| open >= c);
        let is_closure = bytes.get(open + 1) == Some(&b'|');
        if !in_comment && !is_closure {
            return Some(line.trim().to_string());
        }
    }
    None
}

/// Net `{` minus `}` on a line, ignoring those in `//` comments (best-effort; good
/// enough for test-module boundary tracking).
fn brace_delta(line: &str) -> i32 {
    let code = match line.find("//") {
        Some(c) => &line[..c],
        None => line,
    };
    let opens = code.matches('{').count() as i32;
    let closes = code.matches('}').count() as i32;
    opens - closes
}

fn print_human(violations: &[&Site], stale: &[&Allow]) {
    if violations.is_empty() && stale.is_empty() {
        println!("✓ no un-allow-listed raw find/rfind scans in the language crates");
        return;
    }
    if !violations.is_empty() {
        eprintln!(
            "✗ {} raw find/rfind site(s) not in the scan_audit allow-list:\n",
            violations.len()
        );
        for v in violations {
            eprintln!("  {}:{}: {}", v.path, v.line_no, v.code);
        }
        eprintln!(
            "\nA raw `self.source[..].find(delim)` can match the glyph inside an enclosed\n\
             comment or string, dropping content. Either move it onto the trivia-aware\n\
             cursor (`tsv_lang::source_scan`: find_char / find_top_level_delim /\n\
             match_bracket / skip_trivia), or — if it's genuinely safe — add it to ALLOW\n\
             in crates/tsv_debug/src/cli/commands/scan_audit.rs with a category.\n"
        );
    }
    if !stale.is_empty() {
        eprintln!(
            "✗ {} stale scan_audit allow-list entr(y/ies) — no matching source line\n\
             (a site was migrated or reformatted); remove it from ALLOW:\n",
            stale.len()
        );
        for (path, line, category) in stale {
            eprintln!("  [{category}] {path}: {line}");
        }
    }
}

fn print_json(sites: &[Site], violations: &[&Site], stale: &[&Allow]) {
    let to_json =
        |s: &Site| serde_json::json!({ "path": s.path, "line": s.line_no, "code": s.code });
    let report = serde_json::json!({
        "total": sites.len(),
        "allowed": sites.len() - violations.len(),
        "violation_count": violations.len(),
        "violations": violations.iter().map(|s| to_json(s)).collect::<Vec<_>>(),
        "stale_count": stale.len(),
        "stale": stale.iter().map(|(p, l, c)| {
            serde_json::json!({ "path": p, "line": l, "category": c })
        }).collect::<Vec<_>>(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_plain_find_and_rfind() {
        assert_eq!(
            qualifying_find_line("let x = s.find(';');").as_deref(),
            Some("let x = s.find(';');")
        );
        assert_eq!(
            qualifying_find_line("\t\tsource.rfind(\"from\")").as_deref(),
            Some("source.rfind(\"from\")")
        );
    }

    #[test]
    fn excludes_closure_patterns() {
        // Iterator / str-predicate finds take a closure, not a delimiter literal.
        assert_eq!(qualifying_find_line("xs.find(|&b| b == 0)"), None);
        assert_eq!(
            qualifying_find_line("name.find(|c: char| c.is_whitespace())"),
            None
        );
        assert_eq!(qualifying_find_line("(a..b).find(|&j| ok(j))"), None);
    }

    #[test]
    fn excludes_occurrences_inside_line_comments() {
        // A `.find(` after `//` is commentary, not code.
        assert_eq!(
            qualifying_find_line("// naive text.find(\"from\") matches"),
            None
        );
        // …but a real find with a trailing comment still counts.
        assert_eq!(
            qualifying_find_line("s.find('{') // locate brace").as_deref(),
            Some("s.find('{') // locate brace")
        );
    }

    #[test]
    fn rfind_does_not_double_count_as_find() {
        // `.find(` must not match inside `.rfind(` (the leading `.` differs).
        assert_eq!(
            qualifying_find_line("s.rfind('\\n')").as_deref(),
            Some("s.rfind('\\n')")
        );
    }

    #[test]
    fn ignores_lines_without_find() {
        assert_eq!(qualifying_find_line("let y = 1 + 2;"), None);
        assert_eq!(qualifying_find_line("// a comment"), None);
    }

    #[test]
    fn brace_delta_counts_code_braces_only() {
        assert_eq!(brace_delta("fn f() {"), 1);
        assert_eq!(brace_delta("}"), -1);
        assert_eq!(brace_delta("} // closes } in comment"), -1);
        assert_eq!(brace_delta("let x = 0;"), 0);
    }

    #[test]
    fn allow_list_has_no_duplicate_keys() {
        // (path, line) must be unique, else two distinct sites collapse silently.
        let mut seen = std::collections::BTreeSet::new();
        for (path, line, _) in ALLOW {
            assert!(
                seen.insert((*path, *line)),
                "duplicate allow key: {path}: {line}"
            );
        }
    }
}
