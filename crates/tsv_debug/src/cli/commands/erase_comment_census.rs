use argh::FromArgs;
use std::path::{Path, PathBuf};

use super::profile::resolve_files;
use crate::cli::CliError;

/// Census: comments exposed to the type-eraser's refusal window.
///
/// For every `lang="ts"` `.svelte` component under the given roots, collects
/// the spans the compiler's type erasure would drop (TS-only statements,
/// `: T` annotations, type parameters/arguments, `as`/`satisfies`/`!` tails,
/// type-only imports/exports, `declare` items) and counts the comments whose
/// span intersects an erased span's refusal window — the erased span extended
/// to the **next surviving token** (so `let x: Foo /* c */ = v` counts). Such
/// a comment makes the eraser refuse the component rather than silently drop
/// or misplace it, so the exposure rate here sizes the haircut on the
/// stripper's corpus unlock.
///
/// Also flags, per file, the cheaply-detectable *other* refusal blockers
/// (directives/spread, special elements, module scripts, `<option>`/populated
/// `<select>`, instance exports, `{@debug}`, `<svelte:options>`) — an
/// approximation of "the stripper is this file's only blocker", NOT the
/// compiler's full refusal set (runes, derived reads, and evaluator refusals
/// are not detected).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "erase_comment_census")]
pub struct EraseCommentCensusCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// list each exposed file with its exposed-comment kinds
    #[argh(switch)]
    verbose: bool,

    /// file paths, directories, or glob patterns
    #[argh(positional)]
    paths: Vec<String>,
}

impl EraseCommentCensusCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        if self.paths.is_empty() {
            eprintln!("Error: No files provided. Use file paths, directories, or glob patterns.");
            return Err(CliError::Failed);
        }
        let mut files = match resolve_files(&self.paths) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };
        files.retain(|p| p.extension().and_then(|e| e.to_str()) == Some("svelte"));
        if files.is_empty() {
            eprintln!("Error: No .svelte files found");
            return Err(CliError::Failed);
        }

        let mut scanned = 0usize;
        let mut parse_failed = 0usize;
        let mut results = Vec::new();

        for path in &files {
            scanned += 1;
            match census_file(path) {
                Ok(Some(result)) => results.push(result),
                Ok(None) => {} // not a lang="ts" component
                Err(_) => parse_failed += 1,
            }
        }

        let totals = Totals::from_results(&results);
        if self.json {
            print_json(&results, scanned, parse_failed, &totals);
        } else {
            print_table(&results, scanned, parse_failed, &totals, self.verbose);
        }
        Ok(())
    }
}

/// The erased-region kinds tracked for the breakdown, ordered for display.
const KINDS: [&str; 7] = [
    "interface",
    "type_alias",
    "import_type",
    "annotation",
    "type_params",
    "cast",
    "other_ts",
];

/// One `lang="ts"` component's census result.
struct FileResult {
    path: PathBuf,
    /// Cheaply-detected non-TS refusal blockers present (empty = the stripper
    /// is plausibly this file's only blocker).
    other_blockers: Vec<&'static str>,
    /// Exposed comments by erased-region kind (indices follow `KINDS`).
    exposed_by_kind: [usize; 7],
}

impl FileResult {
    fn exposed(&self) -> usize {
        self.exposed_by_kind.iter().sum()
    }
}

struct Totals {
    ts_files: usize,
    ts_exposed: usize,
    unlock_files: usize,
    unlock_exposed: usize,
    comments_by_kind: [usize; 7],
}

impl Totals {
    fn from_results(results: &[FileResult]) -> Self {
        let mut t = Self {
            ts_files: results.len(),
            ts_exposed: 0,
            unlock_files: 0,
            unlock_exposed: 0,
            comments_by_kind: [0; 7],
        };
        for r in results {
            let exposed = r.exposed() > 0;
            if exposed {
                t.ts_exposed += 1;
            }
            if r.other_blockers.is_empty() {
                t.unlock_files += 1;
                if exposed {
                    t.unlock_exposed += 1;
                }
            }
            for (total, n) in t.comments_by_kind.iter_mut().zip(r.exposed_by_kind) {
                *total += n;
            }
        }
        t
    }
}

/// An erased span with its classification (index into `KINDS`).
struct ErasedSpan {
    start: usize,
    end: usize,
    kind: usize,
}

/// Census one file. `Ok(None)` = parses but is not a `lang="ts"` component.
fn census_file(path: &Path) -> Result<Option<FileResult>, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(&source, &arena).map_err(|e| format!("parse error: {e}"))?;
    let wire = tsv_svelte::convert_ast_json(&root, &source);

    if !is_ts_document(&wire) {
        return Ok(None);
    }

    let mut spans = Vec::new();
    let mut blockers = Vec::new();
    collect(&wire, &mut spans, &mut blockers);
    if wire.get("options").is_some_and(|o| !o.is_null()) {
        push_blocker(&mut blockers, "svelte:options");
    }
    if wire.get("module").is_some_and(|m| !m.is_null()) {
        push_blocker(&mut blockers, "module script");
    }

    let mut exposed_by_kind = [0usize; 7];
    for comment in &root.comments {
        let (c_start, c_end) = (comment.span.start as usize, comment.span.end as usize);
        // First matching span classifies the comment; each comment counts once.
        if let Some(span) = spans.iter().find(|s| {
            let window_end = next_token_pos(&source, s.end);
            c_start < window_end && c_end > s.start
        }) {
            exposed_by_kind[span.kind] += 1;
        }
    }

    Ok(Some(FileResult {
        path: path.to_path_buf(),
        other_blockers: blockers,
        exposed_by_kind,
    }))
}

/// The oracle's document-wide flag: any `<script>` with `lang` exactly `"ts"`.
fn is_ts_document(wire: &serde_json::Value) -> bool {
    ["instance", "module"].iter().any(|key| {
        wire.get(*key)
            .and_then(|script| script.get("attributes"))
            .and_then(|attrs| attrs.as_array())
            .is_some_and(|attrs| attrs.iter().any(is_lang_ts_attribute))
    })
}

fn is_lang_ts_attribute(attr: &serde_json::Value) -> bool {
    attr.get("name").and_then(|n| n.as_str()) == Some("lang")
        && attr
            .get("value")
            .and_then(|v| v.as_array())
            .and_then(|v| v.first())
            .and_then(|t| t.get("data"))
            .and_then(|d| d.as_str())
            == Some("ts")
}

fn push_blocker(blockers: &mut Vec<&'static str>, blocker: &'static str) {
    if !blockers.contains(&blocker) {
        blockers.push(blocker);
    }
}

fn span_of(node: &serde_json::Value) -> Option<(usize, usize)> {
    let start = node.get("start")?.as_u64()?;
    let end = node.get("end")?.as_u64()?;
    Some((start as usize, end as usize))
}

/// Kind index in `KINDS` for a whole-node erasure.
fn kind_of(node_type: &str) -> usize {
    match node_type {
        "TSInterfaceDeclaration" => 0,
        "TSTypeAliasDeclaration" => 1,
        "TSTypeAnnotation" => 3,
        "TSTypeParameterDeclaration" | "TSTypeParameterInstantiation" => 4,
        _ => 6,
    }
}

/// Recursive generic walk over the wire JSON: collect erased-region spans and
/// cheaply-detectable non-TS blockers.
fn collect(
    node: &serde_json::Value,
    spans: &mut Vec<ErasedSpan>,
    blockers: &mut Vec<&'static str>,
) {
    match node {
        serde_json::Value::Array(items) => {
            for item in items {
                collect(item, spans, blockers);
            }
        }
        serde_json::Value::Object(map) => {
            let node_type = map.get("type").and_then(|t| t.as_str()).unwrap_or("");

            // Whole-node erasures and TS tails.
            match node_type {
                // Expression wrappers: only the type tail past the inner
                // expression is erased; keep walking the expression side.
                "TSAsExpression"
                | "TSSatisfiesExpression"
                | "TSNonNullExpression"
                | "TSInstantiationExpression" => {
                    if let (Some((_, end)), Some(expr)) = (span_of(node), map.get("expression"))
                        && let Some((_, expr_end)) = span_of(expr)
                    {
                        spans.push(ErasedSpan {
                            start: expr_end,
                            end,
                            kind: 5,
                        });
                        collect(expr, spans, blockers);
                    }
                    return;
                }
                "TSTypeAssertion" => {
                    if let (Some((start, _)), Some(expr)) = (span_of(node), map.get("expression"))
                        && let Some((expr_start, _)) = span_of(expr)
                    {
                        spans.push(ErasedSpan {
                            start,
                            end: expr_start,
                            kind: 5,
                        });
                        collect(expr, spans, blockers);
                    }
                    return;
                }
                // Fully-erased (or refused) subtrees: the whole span is the
                // window; nothing inside survives, so don't recurse.
                t if t.starts_with("TS") => {
                    if let Some((start, end)) = span_of(node) {
                        spans.push(ErasedSpan {
                            start,
                            end,
                            kind: kind_of(t),
                        });
                    }
                    return;
                }
                // Non-TS blockers (the greppable approximation).
                "SpreadAttribute"
                | "BindDirective"
                | "OnDirective"
                | "UseDirective"
                | "TransitionDirective"
                | "AnimateDirective"
                | "ClassDirective"
                | "StyleDirective"
                | "LetDirective"
                | "AttachTag" => {
                    push_blocker(blockers, "directive/spread");
                }
                "SvelteElement" | "SvelteComponent" | "SvelteSelf" | "SvelteHead"
                | "SvelteWindow" | "SvelteDocument" | "SvelteBody" | "SvelteFragment"
                | "SlotElement" => {
                    push_blocker(blockers, "special element");
                }
                "DebugTag" => push_blocker(blockers, "{@debug}"),
                "ExportNamedDeclaration" | "ExportDefaultDeclaration" | "ExportAllDeclaration"
                    if map.get("exportKind").and_then(|k| k.as_str()) != Some("type") =>
                {
                    push_blocker(blockers, "instance export");
                }
                "RegularElement" => {
                    let name = map.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let has_children = map
                        .get("fragment")
                        .and_then(|f| f.get("nodes"))
                        .and_then(|n| n.as_array())
                        .is_some_and(|n| !n.is_empty());
                    if name == "option" || (matches!(name, "select" | "optgroup") && has_children) {
                        push_blocker(blockers, "option/select");
                    }
                }
                _ => {}
            }

            // Type-only imports/exports: whole statement or single specifier.
            let import_export_kind = map
                .get("importKind")
                .or_else(|| map.get("exportKind"))
                .and_then(|k| k.as_str());
            if import_export_kind == Some("type")
                && let Some((start, end)) = span_of(node)
            {
                spans.push(ErasedSpan {
                    start,
                    end,
                    kind: 2,
                });
                return;
            }
            // `declare` items are dropped whole.
            if map.get("declare").and_then(serde_json::Value::as_bool) == Some(true)
                && let Some((start, end)) = span_of(node)
            {
                spans.push(ErasedSpan {
                    start,
                    end,
                    kind: 6,
                });
                return;
            }

            for value in map.values() {
                collect(value, spans, blockers);
            }
        }
        _ => {}
    }
}

/// Position of the next surviving token at or after `from`: skips ASCII
/// whitespace and both comment forms, so a comment glued to an erased span's
/// tail sits inside the refusal window.
fn next_token_pos(source: &str, from: usize) -> usize {
    let bytes = source.as_bytes();
    let mut pos = from.min(bytes.len());
    loop {
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos + 1 < bytes.len() && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if pos + 1 < bytes.len() && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < bytes.len() && !(bytes[pos] == b'*' && bytes[pos + 1] == b'/') {
                pos += 1;
            }
            pos = (pos + 2).min(bytes.len());
            continue;
        }
        return pos;
    }
}

#[allow(clippy::cast_precision_loss)]
fn pct(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    part as f64 / whole as f64 * 100.0
}

fn print_table(
    results: &[FileResult],
    scanned: usize,
    parse_failed: usize,
    totals: &Totals,
    verbose: bool,
) {
    if verbose {
        for r in results.iter().filter(|r| r.exposed() > 0) {
            let kinds: Vec<String> = KINDS
                .iter()
                .zip(r.exposed_by_kind)
                .filter(|(_, n)| *n > 0)
                .map(|(k, n)| format!("{k}:{n}"))
                .collect();
            let blockers = if r.other_blockers.is_empty() {
                String::new()
            } else {
                format!("  [other blockers: {}]", r.other_blockers.join(", "))
            };
            eprintln!("{}  {}{}", r.path.display(), kinds.join(" "), blockers);
        }
        if totals.ts_exposed > 0 {
            eprintln!();
        }
    }

    eprintln!("scanned: {scanned} .svelte files ({parse_failed} parse failures)");
    eprintln!(
        "lang=\"ts\" components: {} — {} exposed ({:.1}%)",
        totals.ts_files,
        totals.ts_exposed,
        pct(totals.ts_exposed, totals.ts_files)
    );
    eprintln!(
        "unlock candidates (no other detected blocker): {} — {} exposed ({:.1}%)",
        totals.unlock_files,
        totals.unlock_exposed,
        pct(totals.unlock_exposed, totals.unlock_files)
    );
    let by_kind: Vec<String> = KINDS
        .iter()
        .zip(totals.comments_by_kind)
        .filter(|(_, n)| *n > 0)
        .map(|(k, n)| format!("{k}: {n}"))
        .collect();
    eprintln!(
        "exposed comments by erased-region kind: {}",
        by_kind.join(", ")
    );
}

fn print_json(results: &[FileResult], scanned: usize, parse_failed: usize, totals: &Totals) {
    let files: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| r.exposed() > 0)
        .map(|r| {
            let kinds: serde_json::Map<String, serde_json::Value> = KINDS
                .iter()
                .zip(r.exposed_by_kind)
                .filter(|(_, n)| *n > 0)
                .map(|(k, n)| ((*k).to_string(), serde_json::json!(n)))
                .collect();
            serde_json::json!({
                "path": r.path.to_string_lossy(),
                "other_blockers": r.other_blockers,
                "exposed": kinds,
            })
        })
        .collect();

    let by_kind: serde_json::Map<String, serde_json::Value> = KINDS
        .iter()
        .zip(totals.comments_by_kind)
        .map(|(k, n)| ((*k).to_string(), serde_json::json!(n)))
        .collect();

    let output = serde_json::json!({
        "scanned": scanned,
        "parse_failed": parse_failed,
        "ts_files": totals.ts_files,
        "ts_exposed": totals.ts_exposed,
        "unlock_files": totals.unlock_files,
        "unlock_exposed": totals.unlock_exposed,
        "comments_by_kind": by_kind,
        "exposed_files": files,
    });

    // SAFETY: serde_json Value types always serialize successfully
    #[allow(clippy::unwrap_used)]
    let json_str = serde_json::to_string_pretty(&output).unwrap();
    println!("{json_str}");
}
