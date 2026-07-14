//! Layout-neutrality audit — the ownership-blind-gate probe.
//!
//! ## Why this exists
//!
//! `Comment::owned_by_node` takes a comment out of the positional model: the node
//! its token begins prints it. **Ownership must be invisible to layout** — an
//! owned comment occupies exactly the page space a same-width ordinary comment
//! does. A layout gate that instead *skips* owned comments (asks the *to-emit*
//! question where it should ask *on-page*) goes blind: it lays an owned comment
//! out differently, and the comment silently changes the layout it should have
//! forced (a call that should expand hugs, a value that should hang stays inline).
//! Every bug in the `bug107`/`bug108` arc was one of these.
//!
//! This mechanizes the length-matched-control technique that found them by hand:
//! at each glued block-comment position, format the file with the comment made
//! **owned** (annotation-shaped, `/* @__PP__ */`) and made **ordinary** (plain
//! filler, `/* xxxxx */`) at the **same width** — so the only thing that varies is
//! ownership. If the two layouts differ, a gate read ownership → it is blind.
//!
//! ## Pre-change tool, not a standing gate
//!
//! The probe needs an owned/ordinary *contrast* to detect anything. Before the
//! general "own every glued block comment" rule, an annotation is owned and plain
//! filler is not — the contrast exists, and the probe enumerates exactly the gates
//! that rule would newly trip. After it, plain glued comments are owned too, so the
//! contrast vanishes and the probe passes vacuously — it is a development /
//! characterization tool (and a guard to run *before* any future ownership-rule
//! change), report-and-triage, not a `deno task check` gate.
//!
//! TypeScript-family files only; glued block comments concentrate in JSDoc-typed JS.

use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::Path;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::cli::CliError;

use super::profile::resolve_files;

/// Audit whether a comment's *ownership* ever changes tsv's layout (a gate reading
/// ownership where it should be blind to it).
///
/// Defaults to `tests/fixtures`; point it at external corpora
/// (`../svelte/packages/svelte/src`, `../prettier/tests/format/{js,typescript}`)
/// for the real yield. TypeScript-family files only.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "neutrality_audit")]
pub struct NeutralityAuditCommand {
    /// exit 1 if any layout-non-neutral position is found (dev-loop convenience;
    /// this is not a `deno task check` gate — see the module docs)
    #[argh(switch)]
    gate: bool,

    /// print the owned-vs-ordinary output diff for each finding
    #[argh(switch)]
    verbose: bool,

    /// cap the number of files audited (0 = unlimited)
    #[argh(option, default = "0")]
    limit: usize,

    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// file paths / directories (default: tests/fixtures)
    #[argh(positional)]
    paths: Vec<String>,
}

/// A position whose layout depends on the comment's ownership.
struct Finding {
    display: String,
    /// The original comment content (delimiters excluded) at this position.
    content: String,
    /// 1-based line of the comment in the source (for the report).
    line: usize,
    owned_layout: String,
    ordinary_layout: String,
}

impl NeutralityAuditCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let paths = if self.paths.is_empty() {
            vec!["tests/fixtures".to_string()]
        } else {
            self.paths.clone()
        };
        let mut files = match resolve_files(&paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };
        files.retain(|p| is_ts_family(p) && !is_invalid_input(p));
        if self.limit > 0 {
            files.truncate(self.limit);
        }

        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut findings: Vec<Finding> = Vec::new();
        for path in &files {
            match audit_file(path) {
                Ok(fs) => {
                    if fs.is_empty() {
                        *counts.entry("clean").or_default() += 1;
                    } else {
                        *counts.entry("non_neutral").or_default() += 1;
                        findings.extend(fs);
                    }
                }
                Err(()) => *counts.entry("skipped").or_default() += 1,
            }
        }
        findings.sort_by(|a, b| a.display.cmp(&b.display).then(a.line.cmp(&b.line)));

        self.report(files.len(), &counts, &findings)
    }

    fn report(
        &self,
        scanned: usize,
        counts: &BTreeMap<&'static str, usize>,
        findings: &[Finding],
    ) -> Result<(), CliError> {
        if self.json {
            let findings_json: Vec<_> = findings
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "path": f.display,
                        "line": f.line,
                        "content": f.content,
                        "owned": f.owned_layout,
                        "ordinary": f.ordinary_layout,
                    })
                })
                .collect();
            let out = serde_json::json!({
                "scanned": scanned,
                "counts": counts,
                "findings": findings_json,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
            return self.finish(findings);
        }

        println!("layout-neutrality audit — {scanned} files\n");
        for (label, n) in counts {
            println!("  {n:>6}  {label}");
        }
        println!();
        if findings.is_empty() {
            println!("✓ no ownership-dependent layout (every gate is blind to ownership)");
            return self.finish(findings);
        }
        println!(
            "✗ {} ownership-dependent position(s) — a layout gate reads ownership:\n",
            findings.len()
        );
        for f in findings {
            println!("  {}:{}  /*{}*/", f.display, f.line, f.content);
            if self.verbose {
                println!("    --- owned ---\n{}", indent(&f.owned_layout));
                println!("    --- ordinary ---\n{}", indent(&f.ordinary_layout));
            }
        }
        self.finish(findings)
    }

    fn finish(&self, findings: &[Finding]) -> Result<(), CliError> {
        if self.gate && !findings.is_empty() {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

/// Probe every glued block-comment position in one file.
fn audit_file(path: &Path) -> Result<Vec<Finding>, ()> {
    let source = std::fs::read_to_string(path).map_err(|_| ())?;
    // Format the file as-authored first: if tsv can't, it's out of scope.
    if format_source(&source, ParserType::TypeScript).is_err() {
        return Err(());
    }
    let sites = glued_block_sites(&source);
    let mut findings = Vec::new();
    for site in sites {
        if let Some(f) = probe_site(path, &source, &site) {
            findings.push(f);
        }
    }
    Ok(findings)
}

/// A glued block comment's content byte-range in the source.
struct Site {
    content_start: usize,
    content_end: usize,
    line: usize,
}

/// Every same-line-glued block comment whose content is wide enough to host an
/// annotation (≥6 bytes), keyed by its content byte-range.
fn glued_block_sites(source: &str) -> Vec<Site> {
    let arena = bumpalo::Bump::new();
    let Ok(program) = tsv_ts::parse(source, &arena) else {
        return Vec::new();
    };
    let bytes = source.as_bytes();
    let mut sites = Vec::new();
    for c in &program.comments {
        if !c.is_block {
            continue;
        }
        let end = c.span.end as usize;
        let anchor = skip_ws(bytes, end);
        // Glued to a token on the same line (not another comment / EOF).
        if anchor >= bytes.len() || bytes[anchor] == b'/' || source[end..anchor].contains('\n') {
            continue;
        }
        let (cs, ce) = (c.content_span.start as usize, c.content_span.end as usize);
        if ce - cs < ANNOTATION_MIN {
            continue;
        }
        sites.push(Site {
            content_start: cs,
            content_end: ce,
            line: source[..cs].bytes().filter(|&b| b == b'\n').count() + 1,
        });
    }
    sites
}

/// Format `source` with the site's comment made owned (annotation-shaped) and made
/// ordinary (plain filler) at the same width; a layout difference is the finding.
fn probe_site(path: &Path, source: &str, site: &Site) -> Option<Finding> {
    let width = site.content_end - site.content_start;
    let owned_src = splice(source, site, &annotation_of_width(width));
    let ordinary_src = splice(source, site, &"x".repeat(width));

    let owned = format_source(&owned_src, ParserType::TypeScript).ok()?;
    let ordinary = format_source(&ordinary_src, ParserType::TypeScript).ok()?;

    // Layout-neutral iff the two outputs differ ONLY in the comment's own content:
    // equal length, and the differing bytes form one contiguous run of exactly
    // `width` (the content the two variants swap). Any other diff — a shifted line
    // break, a changed indent — means a gate laid the owned comment out differently.
    if owned.len() == ordinary.len() {
        let ob = owned.as_bytes();
        let xb = ordinary.as_bytes();
        let first = (0..ob.len()).find(|&i| ob[i] != xb[i]);
        let Some(first) = first else {
            return None; // identical (both comments dropped) — not a layout issue
        };
        let last = (0..ob.len())
            .rev()
            .find(|&i| ob[i] != xb[i])
            .unwrap_or(first);
        if last - first + 1 == width {
            return None; // neutral
        }
    }

    Some(Finding {
        display: path.to_string_lossy().into_owned(),
        content: source[site.content_start..site.content_end].to_string(),
        line: site.line,
        owned_layout: owned,
        ordinary_layout: ordinary,
    })
}

/// Replace the site's comment content with `replacement` (same width).
fn splice(source: &str, site: &Site, replacement: &str) -> String {
    let mut s = String::with_capacity(source.len());
    s.push_str(&source[..site.content_start]);
    s.push_str(replacement);
    s.push_str(&source[site.content_end..]);
    s
}

/// Minimum content width that can host a bundler annotation (`@__A__`).
const ANNOTATION_MIN: usize = 6;

/// A bundler-annotation-shaped comment content of exactly `width` bytes (owned by
/// the parser): `@__` + payload + `__`. Requires `width >= ANNOTATION_MIN`.
fn annotation_of_width(width: usize) -> String {
    debug_assert!(width >= ANNOTATION_MIN);
    format!("@__{}__", "P".repeat(width - 5))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn indent(s: &str) -> String {
    use std::fmt::Write as _;
    s.lines().fold(String::new(), |mut acc, l| {
        let _ = writeln!(acc, "      {l}");
        acc
    })
}

fn is_ts_family(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "js" | "mts" | "cts" | "mjs" | "cjs")
    )
}

fn is_invalid_input(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("input_invalid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_of_width_is_owned_shape() {
        // Exactly `width` bytes and recognized as an annotation.
        for w in [ANNOTATION_MIN, 9, 11, 20] {
            let a = annotation_of_width(w);
            assert_eq!(a.len(), w, "width {w}");
            assert!(tsv_ts::is_jsdoc_type_cast_comment(&a) || a.starts_with("@__"));
        }
    }

    #[test]
    fn neutral_when_ownership_is_ignored() {
        // A leading comment on a plain identifier binding: owned or not, the layout
        // is the same, so this position is neutral (no finding).
        let src = "const a = /* @__PURE__ */ x;\n";
        let sites = glued_block_sites(src);
        assert_eq!(sites.len(), 1);
        let arena = bumpalo::Bump::new();
        let _ = tsv_ts::parse(src, &arena); // sanity: parses
        assert!(
            probe_site(Path::new("t.ts"), src, &sites[0]).is_none(),
            "a plain leading-comment position must be layout-neutral"
        );
    }
}
