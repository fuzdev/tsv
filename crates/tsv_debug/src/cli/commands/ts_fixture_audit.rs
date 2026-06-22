use crate::deno::{PrettierParser, parse_svelte, run_prettier};
use crate::diff::{ColorChoice, DiffOptions, print_diff_with_options};
use crate::fixtures::{self, InputType};
use argh::FromArgs;
use std::path::Path;

/// Audit `input.ts` fixtures: verify which genuinely need `.ts` vs. could be
/// `.svelte` (`<script lang="ts">`).
///
/// For each fixture this embeds EVERY `.ts` file (input + variants) in a Svelte
/// `<script lang="ts">` block and checks — with BOTH our formatter and prettier —
/// whether the embedded form produces the same TypeScript. A fixture is **necessary**
/// as `.ts` if any of its files is a byte-0 / file-level feature, fails to parse in a
/// Svelte context, or formats differently embedded (e.g. JSDoc cast paren stripping —
/// which often lives in a variant, not `input.ts`). Otherwise it is **convertible**.
///
/// Replaces eyeballing directory names with an empirical check. Caveat: **convertible**
/// means only that formatting is identical in both contexts — it does NOT judge intent
/// (a fixture may be `.ts` deliberately to exercise the standalone `tsv_ts` / acorn
/// parser path, whose `expected.json` pins a different AST than Svelte's). Treat
/// convertible as "safe to convert if you also want the embedded-path coverage," not a
/// mandate.
///
/// Because the formatting check can't see intent, fixtures that are `.ts` ON PURPOSE
/// are listed in `INTENTIONAL_TS` and reported as **intentional** rather than
/// convertible — this keeps the convertible list to fixtures that are genuinely free to
/// move. Add a fixture there (with a reason) when its `.ts`-ness is load-bearing.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "ts_fixture_audit")]
pub struct TsFixtureAuditCommand {
    /// show the TS-vs-Svelte diff for fixtures that format differently
    #[argh(switch)]
    verbose: bool,

    /// fixture filter patterns (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

impl TsFixtureAuditCommand {
    pub fn run(self) {
        let rt = super::create_runtime();
        rt.block_on(run(self.verbose, &self.filters));
    }
}

/// Fixtures kept as `.ts` ON PURPOSE even though their formatting is context-invariant
/// (the audit would otherwise call them convertible). Each exercises the standalone
/// `tsv_ts` / prettier-typescript parser path that its `expected.json` pins, or backs a
/// doc claim that depends on the standalone-TS context — converting to `.svelte` would
/// dissolve coverage. Matched by relative-path suffix.
const INTENTIONAL_TS: &[(&str, &str)] = &[
    (
        "typescript/syntax/comments/jsdoc_type_cast_ts_prettier_divergence",
        "standalone-TS proof of the JSDoc-cast paren divergence: tsv preserves, prettier's oxc-ts strips. The JS-context match is jsdoc_type_cast_svelte (see conformance_prettier.md §JSDoc / paren semantics)",
    ),
    (
        "typescript/syntax/comments/jsdoc_type_cast_ts",
        "standalone-TS negative match for JSDoc casts (never-add parens, non-@type comments still strip, no double-wrap) — pins the tsv_ts + prettier-typescript path",
    ),
    (
        "typescript/types/function_type/open_paren_comment_prettier_divergence",
        "format-convertible, but the Svelte parser duplicates a function-type `(`-leading comment in Root.comments (lists the same span twice), so the embedded expected.json can't match without replicating that Svelte bug — kept .ts for clean acorn parity",
    ),
    (
        "typescript/syntax/unicode_offsets",
        "pins byte→UTF-16 offset translation on the standalone tsv_ts JSON path (convert_ast_json_string's multibyte branch) — embedding in .svelte would route through tsv_svelte's convert instead",
    ),
    (
        "typescript/syntax/unicode_line_terminators",
        "pins U+2028/U+2029 line counting on the standalone tsv_ts loc path (LocationTracker::new_ecmascript, acorn's LineTerminator set) — formatting is context-invariant but the .svelte path tracks LF-only locations (Svelte's locate-character), so embedding would pin different locs",
    ),
    (
        "typescript/syntax/comments/format_ignore_prettier_divergence",
        "format-convertible (the directive works the same embedded), but kept .ts on purpose to pin the standalone tsv_ts + prettier-typescript-parser path for the format-ignore directive — the Svelte-embedded coverage lives in svelte/syntax/format_ignore/",
    ),
];

/// Look up a fixture in `INTENTIONAL_TS` by relative-path suffix.
fn intentional_ts(dir: &Path) -> Option<&'static str> {
    INTENTIONAL_TS
        .iter()
        .find(|(suffix, _)| dir.ends_with(suffix))
        .map(|(_, reason)| *reason)
}

/// Why a fixture must stay `.ts` (or that it need not).
enum Verdict {
    /// Genuinely needs `.ts`, with a human-readable reason.
    Necessary(String),
    /// Kept as `.ts` deliberately (in `INTENTIONAL_TS`) despite being format-convertible.
    Intentional(&'static str),
    /// Formats differently when embedded — needs `.ts`. Names the file that
    /// diverged (input.ts or a variant) and carries both forms for an optional diff.
    FormatsDifferently {
        file: String,
        tool: &'static str,
        standalone: String,
        embedded: String,
    },
    /// Could be `.svelte` with `lang="ts"`.
    Convertible,
}

// Deliberately serial (no spawn-per-fixture / sidecar pool like the other bulk
// fixtures commands): only ~19 input.ts fixtures exist, a full run is ~0.4s.
async fn run(verbose: bool, filters: &[String]) {
    let fixtures_dir = Path::new("tests/fixtures");
    if !fixtures_dir.exists() {
        eprintln!("Error: fixtures directory not found: tests/fixtures");
        std::process::exit(1);
    }

    let all = match fixtures::walk_fixtures(fixtures_dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error walking fixtures: {e}");
            std::process::exit(1);
        }
    };

    let ts_fixtures: Vec<_> = all
        .into_iter()
        .filter(|f| matches!(f.input_type(), InputType::TypeScript))
        .filter(|f| f.matches_filters(filters))
        .collect();

    if ts_fixtures.is_empty() {
        eprintln!("No input.ts fixtures matched.");
        std::process::exit(1);
    }

    let mut convertible = Vec::new();
    let mut necessary_count = 0;
    let mut intentional_count = 0;

    for fixture in &ts_fixtures {
        let content = match std::fs::read_to_string(fixture.input_path()) {
            Ok(c) => c,
            Err(e) => {
                println!("  ?  {} — read error: {e}", fixture.relative_path);
                continue;
            }
        };

        match classify(&content, &fixture.path).await {
            Verdict::Convertible => {
                println!("  ✓ CONVERTIBLE  {}", fixture.relative_path);
                convertible.push(fixture.relative_path.clone());
            }
            Verdict::Necessary(reason) => {
                necessary_count += 1;
                println!("  ·  necessary    {}  ({reason})", fixture.relative_path);
            }
            Verdict::Intentional(reason) => {
                intentional_count += 1;
                println!("  ·  intentional  {}  ({reason})", fixture.relative_path);
            }
            Verdict::FormatsDifferently {
                file,
                tool,
                standalone,
                embedded,
            } => {
                necessary_count += 1;
                println!(
                    "  ·  necessary    {}  (formats differently: {tool}, {file})",
                    fixture.relative_path
                );
                if verbose {
                    print_diff_with_options(
                        &format!(
                            "{} / {file} — standalone .ts vs embedded .svelte ({tool})",
                            fixture.relative_path
                        ),
                        &standalone,
                        &embedded,
                        &DiffOptions::compare().with_color_choice(ColorChoice::Auto),
                    );
                }
            }
        }
    }

    println!(
        "\n{} input.ts fixtures: {} necessary, {} intentional, {} convertible to .svelte",
        ts_fixtures.len(),
        necessary_count,
        intentional_count,
        convertible.len()
    );
    if !convertible.is_empty() {
        println!(
            "\nConvertible (format identically embedded — could be .svelte with lang=\"ts\"):"
        );
        for p in &convertible {
            println!("  - {p}");
        }
        if !verbose {
            println!(
                "\nRe-run with --verbose to see the diff on the 'formats differently' fixtures."
            );
        }
    }
}

/// Classify a fixture by checking EVERY `.ts` file in its directory (input.ts and
/// all variants — `output_prettier.ts`, `prettier_variant_*`, `unformatted_ours_*`,
/// `prettier_intermediate_*`, …). A fixture is `.ts`-necessary if any of its files
/// carry a byte-0 feature, fail to parse embedded, or format differently between the
/// standalone `.ts` and embedded `<script lang="ts">` contexts.
///
/// Checking variants — not just `input.ts` — matters because a divergence fixture's
/// distinguishing case often lives in a variant (the BOM fixture's `input.ts` is
/// de-BOMed; a normalization variant could collapse differently when embedded).
async fn classify(content: &str, dir: &Path) -> Verdict {
    if let Some(reason) = intentional_ts(dir) {
        return Verdict::Intentional(reason);
    }
    if let Some(reason) = dir_byte0_feature(dir) {
        return Verdict::Necessary(reason);
    }
    if content.trim().is_empty() {
        return Verdict::Necessary("file-level: empty/whitespace-only".into());
    }

    for path in ts_files_sorted(dir) {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let Ok(file_content) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Byte-0 carriers are handled by dir_byte0_feature; don't try to embed them.
        if file_content.starts_with('\u{FEFF}')
            || file_content.starts_with("#!")
            || file_content.trim().is_empty()
        {
            continue;
        }

        match context_diff(&file_content).await {
            Ok(None) => {}
            Ok(Some((tool, standalone, embedded))) => {
                return Verdict::FormatsDifferently {
                    file: name,
                    tool,
                    standalone,
                    embedded,
                };
            }
            Err(reason) => return Verdict::Necessary(format!("{name}: {reason}")),
        }
    }

    Verdict::Convertible
}

/// Does `content` format the SAME standalone (`.ts`) and embedded
/// (`<script lang="ts">`), for both tsv and prettier?
///
/// - `Ok(None)` — context-equivalent (the context doesn't change its formatting)
/// - `Ok(Some((tool, standalone, embedded)))` — formats differently
/// - `Err(reason)` — can't be embedded (Svelte parse / format failure)
///
/// Compares standalone-vs-embedded of the SAME content (not against `input.ts`), so
/// it's valid for non-idempotent variants (`prettier_intermediate_*`, `unformatted_*`)
/// too — it asks "does the surrounding context matter?", independent of convergence.
async fn context_diff(content: &str) -> Result<Option<(&'static str, String, String)>, String> {
    let embedded = wrap_in_ts_script(content);

    if let Err(e) = parse_svelte(&embedded).await {
        return Err(format!("Svelte parse fails: {}", short(&e.to_string())));
    }

    let ts_out = fixtures::format_with_our_formatter(content, "input.ts")
        .map_err(|e| format!("tsv format error: {}", short(&e)))?;
    let sv_out = fixtures::format_with_our_formatter(&embedded, "embed.svelte")
        .map_err(|e| format!("tsv format error: {}", short(&e)))?;
    let sv_body = dedent_script_body(&sv_out);
    if normalize(&sv_body) != normalize(&ts_out) {
        return Ok(Some(("tsv", ts_out, sv_body)));
    }

    let p_ts = run_prettier(content, PrettierParser::Parser("typescript"))
        .await
        .map_err(|e| format!("prettier error: {}", short(&e.to_string())))?;
    let p_sv = run_prettier(&embedded, PrettierParser::Parser("svelte"))
        .await
        .map_err(|e| format!("prettier error: {}", short(&e.to_string())))?;
    let p_body = dedent_script_body(&p_sv);
    if normalize(&p_body) != normalize(&p_ts) {
        return Ok(Some(("prettier", p_ts, p_body)));
    }

    Ok(None)
}

/// All `.ts` files in a fixture dir (excluding `.svelte.ts`), sorted — `input.ts`
/// sorts first, so it's reported as the diverging file when it's the cause.
fn ts_files_sorted(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("ts")
                && !p.to_string_lossy().ends_with(".svelte.ts")
        })
        .collect();
    files.sort();
    files
}

/// Scan every `.ts` file in the fixture dir for a byte-0 feature (BOM or
/// hashbang) that would be lost if the fixture moved into a `<script>` block.
/// Returns a reason naming the file when found.
fn dir_byte0_feature(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ts") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return Some(format!("byte-0 BOM (in {name})"));
        }
        if bytes.starts_with(b"#!") {
            return Some(format!("byte-0 hashbang (in {name})"));
        }
    }
    None
}

/// Wrap standalone TS in a `<script lang="ts">` block, indenting code one tab.
/// Blank lines stay blank; literal lines (e.g. template-string content at column 0)
/// are still prefixed, but `dedent_script_body` strips at most one tab so they round-trip.
fn wrap_in_ts_script(content: &str) -> String {
    let mut s = String::from("<script lang=\"ts\">\n");
    for line in content.lines() {
        if line.is_empty() {
            s.push('\n');
        } else {
            s.push('\t');
            s.push_str(line);
            s.push('\n');
        }
    }
    s.push_str("</script>\n");
    s
}

/// Extract the `<script>` body from formatted Svelte output and strip one tab of
/// indentation per line (the inverse of `wrap_in_ts_script`).
fn dedent_script_body(svelte: &str) -> String {
    let mut out = Vec::new();
    let mut in_script = false;
    for line in svelte.lines() {
        let trimmed = line.trim_start();
        if !in_script {
            if trimmed.starts_with("<script") {
                in_script = true;
            }
            continue;
        }
        if trimmed.starts_with("</script") {
            break;
        }
        out.push(line.strip_prefix('\t').unwrap_or(line));
    }
    out.join("\n")
}

/// Normalize for comparison: ignore trailing whitespace and trailing blank lines.
fn normalize(s: &str) -> String {
    s.trim_end().to_string()
}

/// Truncate a long error message for one-line reporting.
fn short(s: &str) -> String {
    let first = s.lines().next().unwrap_or(s);
    if first.len() > 80 {
        format!("{}…", &first[..80])
    } else {
        first.to_string()
    }
}
