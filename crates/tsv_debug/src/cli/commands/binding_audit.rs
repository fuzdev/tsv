//! Comment↔token binding audit — the re-binding gate.
//!
//! ## Why this exists
//!
//! Two comment kinds bind **forward, to the token after them**: a **JSDoc type
//! cast** (`/** @type {T} */ (x)` — the parens plus the comment *are* the cast)
//! and a **bundler annotation** (`/* @__PURE__ */ f()` — marks the call after it
//! side-effect-free). If formatting moves a paren across such a comment — the
//! cast's parens migrating to wrap a wider expression, or a paren synthesized
//! between an annotation and its call — the comment silently **re-binds** to a
//! different node, changing meaning (a cast annotating the wrong subtree, an
//! annotation gone inert).
//!
//! This class is **invisible to every other gate**: neither a cast nor an
//! annotation nor a grouping paren is a node in the public AST, so both the
//! correct and the re-bound form serialize to byte-identical wire JSON —
//! `ast_diff` calls them equivalent, `roundtrip_audit`'s structural skeleton
//! can't see the difference, and `corpus:compare:format`'s SAFETY check is
//! char-frequency (the characters only *move*).
//!
//! ## The signal
//!
//! Reparse with **`preserve_parens`** (so grouping parens become
//! `ParenthesizedExpression` wire nodes) and, for each glued comment, compare the
//! subtree its token binds — in the input against the tsv-formatted output. Two
//! facts make the comparison sound:
//!
//! - A **cast** stays invisible even under `preserve_parens` (its `JsdocCast`
//!   node emits its bare inner), so the audit anchors *inside* the cast's `(`, on
//!   the first real token, and compares the wrapped subtree.
//! - Under `preserve_parens` the ONLY structural delta formatting can introduce
//!   is a clarity-paren add/remove (formatting is otherwise structure-preserving —
//!   `roundtrip_audit` gates that). So the bound subtree's skeleton is compared
//!   with `ParenthesizedExpression` nodes **stripped** (making it equal to the
//!   paren-free normal-parse skeleton, which formatting preserves), and the
//!   binding-paren signal is carried separately by `anchor_is_paren` — a paren
//!   appearing at the anchor is the re-binding, a clarity paren deep inside is not.
//!
//! ## Buckets
//!
//! A finding is **hard** when the comment is `owned_by_node` in the input (a cast
//! or annotation the parser bound — a re-binding here is a real bug) and **soft**
//! otherwise (a plain glued block comment, whose relocation is a comment-position
//! divergence, not a semantic loss). `--gate` fails on hard findings only.
//!
//! TypeScript-family files only (`.ts`/`.js`/`.mts`/`.cts`/…); casts and
//! annotations live overwhelmingly in JSDoc-typed JS. Svelte `<script>` / `{expr}`
//! embedding is out of scope here.

use argh::FromArgs;
use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;
use tsv_lang::source_scan::{TriviaProfile, skip_trivia};

use crate::audit::properties::Utf16ToByte;
use crate::cli::CliError;
use crate::render_normalize::structural_skeleton;

use super::profile::{is_input_invalid_fixture, resolve_files};

/// Audit whether tsv formatting re-binds any forward-binding comment (JSDoc cast
/// or bundler annotation) to a different subtree.
///
/// Defaults to `tests/fixtures` when no paths are given — a cheap tripwire there
/// (the fixture idempotency invariants make formatted output stable), with the
/// real yield on external corpora (`../svelte/packages/svelte/src`,
/// `../prettier/tests/format/{js,typescript}`, real repos), where JSDoc casts and
/// annotations are dense. TypeScript-family files only.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "binding_audit")]
pub struct BindingAuditCommand {
    /// gate mode: fail (exit 1) on HARD findings only — an owned cast/annotation
    /// comment that re-binds. Soft findings (plain glued comments) are counted
    /// but non-fatal. This is the `deno task check` regression-guard mode.
    #[argh(switch)]
    gate: bool,

    /// print the in→out bound-subtree skeleton for each finding
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

/// A block comment's forward binding, or `None` when it binds nothing (an
/// own-line comment, or one glued to another comment rather than a token).
///
/// The re-binding test is [`is_rebind`], **not** structural equality: the
/// `anchor_is_paren` flag is asymmetric (a paren appearing at the anchor is a
/// re-binding; one disappearing is the safe restore direction), so a derived
/// `PartialEq` would be too coarse for `Glued`.
#[derive(Clone)]
enum Binding {
    /// A JSDoc cast — the paren-stripped skeleton of the subtree it wraps.
    Cast(Value),
    /// A same-line-glued block comment (annotation or plain) — the paren-stripped
    /// skeleton of the bound node plus whether the bound token is a `(` (a
    /// synthesized paren the comment now leads is a re-binding).
    Glued {
        skeleton: Value,
        anchor_is_paren: bool,
    },
}

/// Whether the comment's binding changed in a way that counts as a re-binding.
///
/// A skeleton change (the paren-stripped bound subtree differs) is a re-binding
/// in either direction. The `anchor_is_paren` flag is **asymmetric**: a paren
/// *appearing* at the anchor (`false → true`) is the bug — a synthesized paren
/// now leads the comment and re-binds it — while a paren *disappearing*
/// (`true → false`, a redundant grouping paren stripped) restores the comment's
/// binding to the underlying token and is safe (never a bug; for an annotation
/// it *restores* the correct binding). A kind change (cast↔glued, bound↔unbound)
/// is a re-binding. Using derived `!=` here fired on the safe paren-strip
/// direction — the coverage gap the `ignores_stripped_grouping_paren` test pins.
fn is_rebind(a: Option<&Binding>, b: Option<&Binding>) -> bool {
    match (a, b) {
        (None, None) => false,
        (Some(Binding::Cast(sa)), Some(Binding::Cast(sb))) => sa != sb,
        (
            Some(Binding::Glued {
                skeleton: sa,
                anchor_is_paren: pa,
            }),
            Some(Binding::Glued {
                skeleton: sb,
                anchor_is_paren: pb,
            }),
        ) => sa != sb || (!*pa && *pb),
        _ => true,
    }
}

/// One block comment's audit record: its content (the cross-format match key),
/// whether the parser owns it (hard vs soft), and its forward binding.
struct CommentBinding {
    content: String,
    owned: bool,
    binding: Option<Binding>,
}

/// Per-file outcome.
enum FileOutcome {
    /// Input didn't parse (a parse gap; out of scope).
    ParseError,
    /// tsv couldn't format the input.
    FormatError,
    /// Formatted output didn't reparse (a round-trip failure — `roundtrip_audit`'s
    /// domain; here it just means the bindings can't be compared).
    ReparseError,
    /// The block-comment sequence changed under formatting (a drop / add / merge /
    /// reorder — `comments:audit`'s domain), so bindings can't be aligned.
    CommentSetChanged,
    /// Compared cleanly (with any per-comment re-binding findings).
    Compared(Vec<Finding>),
}

/// One re-binding finding on a single comment.
struct Finding {
    display: String,
    content: String,
    /// `true` for the hard bucket (an owned cast/annotation).
    hard: bool,
    /// The comment is a JSDoc cast (vs an annotation / plain glued comment).
    cast: bool,
    in_sig: String,
    out_sig: String,
}

impl BindingAuditCommand {
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
        // TypeScript-family only; `.svelte`/`.css` (and intentionally-invalid
        // fixture inputs) aren't binding-audit subjects.
        files.retain(|p| is_ts_family(p) && !is_input_invalid_fixture(p));
        // A scan with nothing in it must not read as a pass: `--gate` reports "no
        // re-binding findings" and exits 0 on an empty set, so a typo'd path — or a
        // tree with no TS-family files at all — would look identical to a clean run.
        // Fail loud instead, matching `render_audit`'s "No .svelte files found".
        if files.is_empty() {
            eprintln!("Error: no TypeScript-family files found (searched {paths:?})");
            return Err(CliError::Failed);
        }
        if self.limit > 0 {
            files.truncate(self.limit);
        }

        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut findings: Vec<Finding> = Vec::new();
        for path in &files {
            match audit_file(path) {
                FileOutcome::ParseError => *counts.entry("parse_error").or_default() += 1,
                FileOutcome::FormatError => *counts.entry("format_error").or_default() += 1,
                FileOutcome::ReparseError => *counts.entry("reparse_error").or_default() += 1,
                FileOutcome::CommentSetChanged => {
                    *counts.entry("comment_set_changed").or_default() += 1;
                }
                FileOutcome::Compared(fs) => {
                    if fs.is_empty() {
                        *counts.entry("clean").or_default() += 1;
                    }
                    for f in fs {
                        *counts
                            .entry(if f.hard { "hard_rebind" } else { "soft_rebind" })
                            .or_default() += 1;
                        findings.push(f);
                    }
                }
            }
        }
        // Hard findings first.
        findings.sort_by(|a, b| b.hard.cmp(&a.hard).then(a.display.cmp(&b.display)));

        self.report(files.len(), &counts, &findings)
    }

    fn report(
        &self,
        scanned: usize,
        counts: &BTreeMap<&'static str, usize>,
        findings: &[Finding],
    ) -> Result<(), CliError> {
        // `--gate` fails on hard findings only; a bare run fails on any finding.
        let has_hard = findings.iter().any(|f| f.hard);
        let is_fail = if self.gate {
            has_hard
        } else {
            !findings.is_empty()
        };

        if self.json {
            let findings_json: Vec<Value> = findings
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "path": f.display,
                        "content": f.content,
                        "bucket": if f.hard { "hard_rebind" } else { "soft_rebind" },
                        "cast": f.cast,
                        "in": f.in_sig,
                        "out": f.out_sig,
                    })
                })
                .collect();
            let out = serde_json::json!({
                "scanned": scanned,
                "gate": self.gate,
                "counts": counts,
                "findings": findings_json,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
            return if is_fail {
                Err(CliError::Failed)
            } else {
                Ok(())
            };
        }

        println!("comment↔token binding audit — {scanned} files\n");
        for (label, n) in counts {
            println!("  {n:>6}  {label}");
        }
        println!();
        if self.gate {
            println!("(gate mode: only hard_rebind fails; soft counts are informational)\n");
        }

        // In gate mode only hard findings are reported (soft are informational and
        // already shown in the counts).
        let reported: Vec<&Finding> = if self.gate {
            findings.iter().filter(|f| f.hard).collect()
        } else {
            findings.iter().collect()
        };
        if reported.is_empty() {
            println!("✓ no re-binding findings (every glued comment binds the same subtree)");
            return if is_fail {
                Err(CliError::Failed)
            } else {
                Ok(())
            };
        }
        println!("✗ {} finding(s):\n", reported.len());
        for f in reported {
            // A hard non-cast finding is any parser-owned glued comment (a bundler
            // annotation OR a plain glued comment — post "own every glued block
            // comment" they bind identically); a soft one is an unowned glued comment.
            let kind = if f.cast {
                "cast"
            } else if f.hard {
                "owned"
            } else {
                "glued"
            };
            println!(
                "  [{}] {kind}  {:?}  {}",
                if f.hard { "hard_rebind" } else { "soft_rebind" },
                f.content,
                f.display,
            );
            if self.verbose {
                println!("      in : {}", f.in_sig);
                println!("      out: {}", f.out_sig);
            }
        }
        if is_fail {
            Err(CliError::Failed)
        } else {
            Ok(())
        }
    }
}

/// Audit one file: format it, reparse both, align block comments, compare bindings.
fn audit_file(path: &Path) -> FileOutcome {
    let Ok(source) = std::fs::read_to_string(path) else {
        return FileOutcome::ParseError;
    };
    let Some(input) = extract_bindings(&source) else {
        return FileOutcome::ParseError;
    };
    let Ok(formatted) = format_source(&source, ParserType::TypeScript) else {
        return FileOutcome::FormatError;
    };
    let Some(output) = extract_bindings(&formatted) else {
        return FileOutcome::ReparseError;
    };

    // A pure re-binding preserves the block-comment content sequence (it only
    // moves a comment relative to tokens); a changed sequence is a drop / add /
    // merge / reorder, which this audit can't align and doesn't own.
    if input.len() != output.len()
        || input
            .iter()
            .zip(&output)
            .any(|(a, b)| a.content != b.content)
    {
        return FileOutcome::CommentSetChanged;
    }

    let display = path.to_string_lossy().into_owned();
    let mut findings = Vec::new();
    for (a, b) in input.iter().zip(&output) {
        if is_rebind(a.binding.as_ref(), b.binding.as_ref()) {
            findings.push(Finding {
                display: display.clone(),
                content: a.content.clone(),
                hard: a.owned,
                cast: matches!(a.binding, Some(Binding::Cast(_)))
                    || matches!(b.binding, Some(Binding::Cast(_))),
                in_sig: describe(a.binding.as_ref()),
                out_sig: describe(b.binding.as_ref()),
            });
        }
    }
    FileOutcome::Compared(findings)
}

/// The forward binding of every block comment in `source` (parsed with
/// `preserve_parens`), in source order. `None` if the source doesn't parse.
fn extract_bindings(source: &str) -> Option<Vec<CommentBinding>> {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse_preserve_parens(source, &arena).ok()?;
    let wire = tsv_ts::convert_ast_json(&program, source);
    let map = Utf16ToByte::new(source);
    let bytes = source.as_bytes();

    let mut out = Vec::new();
    for c in program.comments {
        if !c.is_block {
            continue;
        }
        let content = c.content(source).to_string();
        let binding = comment_binding(bytes, source, &wire, &map, c, &content);
        out.push(CommentBinding {
            content,
            owned: c.owned_by_node,
            binding,
        });
    }
    Some(out)
}

/// Resolve one block comment's forward binding.
fn comment_binding(
    bytes: &[u8],
    source: &str,
    wire: &Value,
    map: &Utf16ToByte,
    comment: &tsv_lang::Comment,
    content: &str,
) -> Option<Binding> {
    let end = comment.span.end as usize;
    let anchor = skip_ws(bytes, end);
    // Glued to another comment or EOF — bound to no token.
    if anchor >= bytes.len() || bytes[anchor] == b'/' {
        return None;
    }
    let skeleton_at =
        |pos: usize| outermost_at(wire, map, pos).map(|n| structural_skeleton(&strip_parens(n)));

    if tsv_ts::is_jsdoc_type_cast_comment(content) {
        // A cast's parens are expected: it must be followed by `(`, and it binds
        // the subtree *inside* — so anchor past the `(` and its trivia.
        if bytes[anchor] != b'(' {
            return None;
        }
        let inner = skip_trivia_run(bytes, anchor + 1);
        return Some(Binding::Cast(skeleton_at(inner)?));
    }

    // A non-cast block comment binds only when glued to its token on the same line
    // (a comment on its own line leads the line, not the token).
    if source[end..anchor].contains('\n') {
        return None;
    }
    Some(Binding::Glued {
        skeleton: skeleton_at(anchor)?,
        anchor_is_paren: bytes[anchor] == b'(',
    })
}

/// The outermost wire node whose byte-start equals `target_byte` (pre-order DFS —
/// the first hit is the shallowest, i.e. widest, node beginning there).
///
/// Wire `start`/`end` are **UTF-16** code-unit offsets (acorn/JS semantics), while
/// `target_byte` and every source scan here are byte-space. A node's `start` is
/// translated to bytes through `map` and compared to `target_byte` — start-only, the
/// faithful analog of the previous UTF-16 `start`-equality match (no `end`, so no node
/// is narrowed out by a missing `end` translation). The two spaces coincide on ASCII
/// and diverge past any multibyte char — see
/// `glued_binding_resolves_through_a_multibyte_offset`.
fn outermost_at<'a>(v: &'a Value, map: &Utf16ToByte, target_byte: usize) -> Option<&'a Value> {
    if let Value::Object(m) = v
        && m.contains_key("type")
        && let Some(start) = m.get("start").and_then(Value::as_u64)
        && map.byte(start as usize) == Some(target_byte)
    {
        return Some(v);
    }
    match v {
        Value::Object(m) => m.values().find_map(|c| outermost_at(c, map, target_byte)),
        Value::Array(a) => a.iter().find_map(|c| outermost_at(c, map, target_byte)),
        _ => None,
    }
}

/// Recursively unwrap `ParenthesizedExpression` nodes. Under `preserve_parens`
/// the only structural delta formatting can introduce is a clarity-paren
/// add/remove, so a paren-stripped skeleton is formatting-invariant; the
/// binding-paren signal is carried separately by `anchor_is_paren`.
fn strip_parens(v: &Value) -> Value {
    if let Value::Object(m) = v
        && m.get("type").and_then(Value::as_str) == Some("ParenthesizedExpression")
        && let Some(inner) = m.get("expression")
    {
        return strip_parens(inner);
    }
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, val)| (k.clone(), strip_parens(val)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(strip_parens).collect()),
        other => other.clone(),
    }
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Skip a run of whitespace + comments (not strings — a string starts an
/// expression). Used to reach the inner expression past a cast's `(` (an
/// intervening comment must not be mistaken for the bound node).
fn skip_trivia_run(bytes: &[u8], mut i: usize) -> usize {
    let end = bytes.len();
    loop {
        while i < end && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= end {
            return i;
        }
        match skip_trivia(bytes, i, end, TriviaProfile::COMMENTS) {
            Some(j) => i = j,
            None => return i,
        }
    }
}

/// A short human-readable form of a binding for the finding report.
fn describe(b: Option<&Binding>) -> String {
    match b {
        None => "<unbound>".to_string(),
        Some(Binding::Cast(sk)) => format!("cast({})", top_type(sk)),
        Some(Binding::Glued {
            skeleton,
            anchor_is_paren,
        }) => format!(
            "glued({}{})",
            top_type(skeleton),
            if *anchor_is_paren { ", on-paren" } else { "" }
        ),
    }
}

/// The `type` of a skeleton's top node (for the readable finding summary).
fn top_type(sk: &Value) -> &str {
    sk.get("type").and_then(Value::as_str).unwrap_or("?")
}

fn is_ts_family(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "js" | "mts" | "cts" | "mjs" | "cjs")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Format `src`, then list the comments whose binding changed, as
    /// `(content, hard)` pairs. Panics if `src` doesn't format/round-trip.
    fn format_rebinds(src: &str) -> Vec<(String, bool)> {
        let input = extract_bindings(src).expect("input parses");
        let formatted = format_source(src, ParserType::TypeScript).expect("formats");
        let output = extract_bindings(&formatted).expect("output reparses");
        assert_eq!(
            input.len(),
            output.len(),
            "comment set changed: {formatted:?}"
        );
        input
            .iter()
            .zip(&output)
            .filter(|(a, b)| is_rebind(a.binding.as_ref(), b.binding.as_ref()))
            .map(|(a, _)| (a.content.clone(), a.owned))
            .collect()
    }

    /// Compare two authorings directly (bypassing tsv's own formatter), the way
    /// the audit compares input vs formatted output.
    fn rebinds(a: &str, b: &str) -> Vec<String> {
        let ga = extract_bindings(a).expect("a parses");
        let gb = extract_bindings(b).expect("b parses");
        assert_eq!(ga.len(), gb.len(), "comment set differs");
        ga.iter()
            .zip(&gb)
            .filter(|(x, y)| is_rebind(x.binding.as_ref(), y.binding.as_ref()))
            .map(|(x, _)| x.content.clone())
            .collect()
    }

    #[test]
    fn detects_cast_paren_migration() {
        // The cast's parens migrate from `root` to the whole `??` expression, so
        // it annotates a different node — the class no whole-tree diff can see.
        let on_root = "var t = c ? a : /** @type {D} */ (root).head ?? b;\n";
        let on_whole = "var t = c ? a : /** @type {D} */ (root.head ?? b);\n";
        assert!(!rebinds(on_root, on_whole).is_empty());
    }

    #[test]
    fn detects_annotation_synthesized_paren() {
        // A paren synthesized between the annotation and its call leaves the
        // annotation marking a paren (inert) instead of the call.
        let glued = "!/* @__PURE__ */ f();\n";
        let synth_paren = "/* @__PURE__ */ (f());\n";
        assert!(!rebinds(glued, synth_paren).is_empty());
    }

    #[test]
    fn ignores_pure_reformat() {
        // A mere line break inside an annotated call is not a re-binding.
        let flat = "const x = /* @__PURE__ */ f(aaaaaa, bbbbbb);\n";
        let broken = "const x = /* @__PURE__ */ f(\n\taaaaaa,\n\tbbbbbb\n);\n";
        assert!(rebinds(flat, broken).is_empty());
    }

    #[test]
    fn ignores_stripped_grouping_paren() {
        // A redundant grouping paren stripped from around the bound token — the
        // safe direction (anchor_is_paren true→false, identical skeleton) — is
        // NOT a re-binding; it restores the comment's binding to the token. This
        // is the paren-strip direction that a derived `!=` wrongly flagged.
        let with_paren = "const b = /* grouping */ (expr);\n";
        let without = "const b = /* grouping */ expr;\n";
        assert!(rebinds(with_paren, without).is_empty());
    }

    #[test]
    fn detects_appearing_grouping_paren() {
        // The mirror of the case above: a paren *appearing* at the anchor
        // (false→true) re-binds the comment to the paren even when the stripped
        // skeleton is unchanged — the bug direction, so it must be a finding.
        let without = "const b = /* grouping */ expr;\n";
        let with_paren = "const b = /* grouping */ (expr);\n";
        assert!(!rebinds(without, with_paren).is_empty());
    }

    #[test]
    fn ignores_nested_clarity_paren() {
        // A clarity paren added deep inside the cast's wrapped subtree is not a
        // re-binding of the cast — paren-stripping erases it.
        let a = "const t = /** @type {T} */ (f(a && b || c));\n";
        let b = "const t = /** @type {T} */ (f((a && b) || c));\n";
        assert!(rebinds(a, b).is_empty());
    }

    #[test]
    fn cast_anchor_skips_intervening_comment() {
        // A comment between the cast's `(` and its inner expression must not be
        // mistaken for the bound node — the cast binds the inner call.
        let src = "const t = /** @type {T} */ (\n\t/* note */\n\tg()\n);\n";
        let bindings = extract_bindings(src).expect("parses");
        let cast = bindings
            .iter()
            .find(|b| b.content.contains("@type"))
            .expect("cast present");
        match &cast.binding {
            Some(Binding::Cast(sk)) => assert_eq!(top_type(sk), "CallExpression"),
            other => panic!(
                "cast should bind the inner call, got {}",
                describe(other.as_ref())
            ),
        }
    }

    /// NON-ASCII coordinate guard — the whole risk of the byte-space rewrite.
    ///
    /// The audit compares wire node `start`s (UTF-16 code units, acorn semantics)
    /// against byte offsets from source scanning. The two coincide on ASCII and
    /// **diverge** the moment a file holds a multibyte char, so every corpus/gate run
    /// (all ASCII, in practice) grades byte-space and UTF-16-space arithmetic
    /// identically and is blind to a coordinate bug here — only a hand-built non-ASCII
    /// case can catch it.
    ///
    /// The `é` (2 bytes / 1 UTF-16 unit) before the glued comment's bound token pushes
    /// `foo`'s byte offset (19) one past its wire UTF-16 start (18). The correct
    /// byte-space resolution (translate the node's UTF-16 `start` through `Utf16ToByte`,
    /// then compare to the byte anchor) binds the `foo` `Identifier`. A byte-vs-UTF-16
    /// confusion would instead hunt for a node at UTF-16 offset 19 — byte 20, mid-`foo`,
    /// where nothing begins — and resolve to `None`. So `Some(Glued Identifier)` vs
    /// `None` is a value only correct arithmetic yields, making this a real guard rather
    /// than a vacuous pass.
    #[test]
    fn glued_binding_resolves_through_a_multibyte_offset() {
        let src = "const é = /* c */ foo;\n";
        let bindings = extract_bindings(src).expect("parses");
        let comment = bindings
            .iter()
            .find(|b| b.content.contains('c'))
            .expect("block comment present");
        match &comment.binding {
            Some(Binding::Glued {
                skeleton,
                anchor_is_paren,
            }) => {
                assert_eq!(
                    top_type(skeleton),
                    "Identifier",
                    "the glued comment must bind the `foo` identifier"
                );
                assert!(!*anchor_is_paren, "the bound token is `foo`, not a `(`");
            }
            other => panic!(
                "glued comment should bind the Identifier, got {}",
                describe(other.as_ref())
            ),
        }
    }

    #[test]
    fn owned_cast_and_annotation_are_hard() {
        // Under the current formatter these owned comments must not re-bind (the
        // green guard), and their bucket is hard when they would.
        for src in [
            "var t = c ? a : /** @type {D} */ (root).head ?? b;\n",
            "const x = /* @__PURE__ */ f();\n",
            "const y = !(/* @__PURE__ */ g());\n",
        ] {
            let hard: Vec<_> = format_rebinds(src)
                .into_iter()
                .filter(|(_, h)| *h)
                .collect();
            assert!(
                hard.is_empty(),
                "owned re-binding under format: {src:?} -> {hard:?}"
            );
        }
    }
}
