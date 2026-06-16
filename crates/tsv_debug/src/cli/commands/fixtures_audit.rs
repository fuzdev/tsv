use crate::deno::run_prettier;
use crate::fixtures::{self, AuditSignature, Fixture, FixtureFiles, read_file};
use argh::FromArgs;
use futures_util::stream::{self, StreamExt};
use std::collections::HashMap;

/// Investigate fixture normalization graphs (diagnostic; --all for every fixture).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fixtures_audit")]
pub struct FixturesAuditCommand {
    /// show full graph for every fixture
    #[argh(switch, short = 'v')]
    verbose: bool,

    /// audit all fixtures (default: only _prettier_divergence)
    #[argh(switch)]
    all: bool,

    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// fixture filter patterns (multiple = OR)
    #[argh(positional)]
    filters: Vec<String>,
}

impl FixturesAuditCommand {
    pub fn run(self) {
        let rt = crate::cli::commands::create_runtime();
        rt.block_on(self.run_async());
    }

    async fn run_async(self) {
        let fixtures_dir = std::path::Path::new("tests/fixtures");

        if !fixtures_dir.exists() {
            eprintln!("Error: fixtures directory not found: tests/fixtures");
            std::process::exit(1);
        }

        let all_fixtures = match fixtures::walk_fixtures(fixtures_dir) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error walking fixtures: {e}");
                std::process::exit(1);
            }
        };

        // Apply filters, then scope to divergence fixtures by default
        let fixture_list: Vec<_> = all_fixtures
            .into_iter()
            .filter(|f| f.matches_filters(&self.filters))
            .filter(|f| {
                if self.all || !self.filters.is_empty() {
                    return true;
                }
                // Default: only _prettier_divergence fixtures
                f.is_prettier_divergence()
            })
            .collect();

        if fixture_list.is_empty() {
            if self.filters.is_empty() {
                eprintln!("No _prettier_divergence fixtures found (use --all for all fixtures)");
            } else {
                eprintln!("No fixtures found matching: {}", self.filters.join(" "));
            }
            std::process::exit(1);
        }

        // Audit fixtures in parallel — tokio::spawn per fixture so the
        // CPU-bound Rust work runs on all runtime workers (buffer_unordered
        // alone only interleaves at await points on the stream-driving task),
        // with a small sidecar pool for the JS side
        let concurrency = crate::deno::init_bulk_pool();

        let joined: Vec<_> = stream::iter(fixture_list)
            .map(|fixture| tokio::spawn(async move { audit_fixture(&fixture).await }))
            .buffer_unordered(concurrency)
            .collect()
            .await;
        let mut results = Vec::with_capacity(joined.len());
        for handle in joined {
            match handle {
                Ok(r) => results.push(r),
                Err(e) => {
                    eprintln!("fixture audit task panicked: {e}");
                    std::process::exit(2);
                }
            }
        }

        if self.json {
            self.print_json(&results);
        } else {
            self.print_human(&results);
        }
    }

    fn print_json(&self, results: &[FixtureAudit]) {
        let output: Vec<_> = results
            .iter()
            .filter(|r| self.verbose || r.has_novel)
            .map(|r| {
                serde_json::json!({
                    "fixture": r.fixture_path,
                    "has_novel": r.has_novel,
                    "suggestions": r.suggestions,
                })
            })
            .collect();

        // serde_json serialization of these plain Value/output types is infallible
        #[allow(clippy::expect_used)]
        let json = serde_json::to_string_pretty(&output).expect("Failed to serialize JSON");
        println!("{json}");
    }

    fn print_human(&self, results: &[FixtureAudit]) {
        let mut novel_count = 0;
        let mut shown_count = 0;

        for audit in results {
            if !self.verbose && !audit.has_novel {
                continue;
            }

            shown_count += 1;
            println!("{}/", audit.fixture_path);

            for file_audit in &audit.file_audits {
                // In non-verbose mode, skip files that have no novel results
                if !self.verbose && file_audit.novel_suggestion.is_none() {
                    continue;
                }

                println!("  {}", file_audit.filename);

                if let Some(ref ours) = file_audit.ours_result {
                    println!("    ours  -> {}", format_result_label(ours));
                }

                if let Some(ref prettier) = file_audit.prettier_result {
                    println!("    prttr -> {}", format_result_label(prettier));
                }

                if let Some(ref suggestion) = file_audit.novel_suggestion {
                    let msg = format_suggestion(suggestion);
                    println!("    >> {msg}");
                    if !matches!(suggestion, Suggestion::DocumentedMultiPass(_)) {
                        novel_count += 1;
                    }
                }
            }

            println!();
        }

        // Summary
        let total = results.len();
        let with_novel = results.iter().filter(|r| r.has_novel).count();

        if novel_count > 0 {
            println!(
                "Audited {total} fixtures: {with_novel} with novel results ({novel_count} novel outputs)"
            );
        } else if shown_count > 0 {
            println!("Audited {total} fixtures: no novel results");
        } else {
            println!(
                "Audited {total} fixtures: no novel results (use --verbose to see full graphs)"
            );
        }
    }
}

/// Classification of a formatting result
#[derive(Debug, Clone)]
enum FormatResult {
    /// Output matches the file itself (idempotent)
    IdempotentSelf,
    /// Output matches input.*
    MatchesInput,
    /// Output matches output_prettier.*
    MatchesOutputPrettier,
    /// Output matches a prettier_variant_* file
    MatchesPrettierVariant(String),
    /// Output matches a variant_* file
    MatchesVariant(String),
    /// Output matches a prettier_intermediate_* file
    MatchesPrettierIntermediate(String),
    /// Novel output not matching any known file
    Novel,
}

/// Suggestion for a novel result
#[derive(Debug, Clone, serde::Serialize)]
enum Suggestion {
    /// Suggest creating a prettier_variant_* file
    PrettierVariant(String),
    /// Suggest creating a variant_* file
    Variant(String),
    /// Suggest creating a prettier_intermediate_* file
    PrettierIntermediate(String),
    /// Suggest creating a prettier_intermediate_to_variant_* file
    PrettierIntermediateToVariant(String),
    /// Prettier-chain from output_prettier is documented in audit_signature.txt (chain depth K)
    DocumentedMultiPass(usize),
    /// Needs investigation
    Investigate(String),
}

/// Result of checking audit_signature.txt against the live prettier chain
enum AuditSignatureCheck {
    /// Signature file exists and matches the live chain. Carries chain depth.
    MatchesRecorded(usize),
    /// Signature file exists but the live chain differs from what's recorded —
    /// genuine prettier drift since signature was captured.
    Drift,
    /// Check could not be performed: I/O failure, malformed signature, or prettier
    /// failed during the live walk. Distinct from `Drift` because the remediation is
    /// "investigate" rather than "regenerate" — regenerating would hit the same error.
    Error(String),
    /// No signature file present
    NoSignature,
}

/// Check whether `output_prettier.*`'s prettier chain matches a recorded `audit_signature.txt`.
///
/// Reads the signature file (if present), walks prettier from `output_prettier.*` to its fixed
/// point live, and byte-compares. This mirrors F4 validation but runs in audit context.
async fn check_audit_signature(fixture: &Fixture) -> AuditSignatureCheck {
    let signature_path = fixture.audit_signature_path();
    if !signature_path.exists() {
        return AuditSignatureCheck::NoSignature;
    }
    let raw = match read_file(&signature_path) {
        Ok(s) => s,
        Err(e) => return AuditSignatureCheck::Error(format!("read audit_signature.txt: {e}")),
    };
    let recorded = match AuditSignature::parse(&raw) {
        Ok(s) => s,
        Err(e) => return AuditSignatureCheck::Error(format!("parse audit_signature.txt: {e}")),
    };
    let output_prettier_path = fixture.output_prettier_path();
    let output_prettier_content = match read_file(&output_prettier_path) {
        Ok(s) => s,
        Err(e) => return AuditSignatureCheck::Error(format!("read output_prettier: {e}")),
    };
    let parser = fixture.input_type().prettier_parser();
    match AuditSignature::walk(&output_prettier_content, parser).await {
        Ok(Some(live)) if live.passes == recorded.passes => {
            AuditSignatureCheck::MatchesRecorded(live.passes.len())
        }
        Ok(_) => AuditSignatureCheck::Drift,
        Err(e) => AuditSignatureCheck::Error(format!("prettier chain walk: {e}")),
    }
}

/// Result of auditing one file within a fixture
#[derive(Debug)]
struct FileAudit {
    filename: String,
    ours_result: Option<FormatResult>,
    prettier_result: Option<FormatResult>,
    novel_suggestion: Option<Suggestion>,
}

/// Result of auditing an entire fixture
#[derive(Debug, serde::Serialize)]
struct FixtureAudit {
    fixture_path: String,
    #[serde(skip)]
    file_audits: Vec<FileAudit>,
    has_novel: bool,
    suggestions: Vec<Suggestion>,
}

fn format_result_label(result: &FormatResult) -> String {
    match result {
        FormatResult::IdempotentSelf => "self".to_string(),
        FormatResult::MatchesInput => "input".to_string(),
        FormatResult::MatchesOutputPrettier => "output_prettier".to_string(),
        FormatResult::MatchesPrettierVariant(name) => name.clone(),
        FormatResult::MatchesVariant(name) => name.clone(),
        FormatResult::MatchesPrettierIntermediate(name) => name.clone(),
        FormatResult::Novel => "[novel]".to_string(),
    }
}

fn format_suggestion(suggestion: &Suggestion) -> String {
    match suggestion {
        Suggestion::PrettierVariant(suffix) => {
            format!("suggest prettier_variant_{suffix} (prettier stable, ours normalizes to input)")
        }
        Suggestion::Variant(suffix) => {
            format!("suggest variant_{suffix} (both formatters keep stable)")
        }
        Suggestion::PrettierIntermediate(suffix) => {
            format!(
                "suggest prettier_intermediate_{suffix} (prettier unstable, converges to input)"
            )
        }
        Suggestion::PrettierIntermediateToVariant(suffix) => {
            format!(
                "suggest prettier_intermediate_to_variant_{suffix} (prettier unstable, converges to a variant_* / prettier_variant_*)"
            )
        }
        Suggestion::DocumentedMultiPass(depth) => {
            format!(
                "documented: prettier non-idempotent on output_prettier (chain depth={depth}, pinned by audit_signature.txt)"
            )
        }
        Suggestion::Investigate(reason) => format!("investigate: {reason}"),
    }
}

/// Audit a single fixture, building the normalization graph
async fn audit_fixture(fixture: &Fixture) -> FixtureAudit {
    let fixture_dir = &fixture.path;
    let input_type = fixture.input_type();
    let input_ext = input_type.extension();
    let prettier_parser = input_type.prettier_parser();
    let files = FixtureFiles::scan(fixture);

    // Read all known file contents
    let input_content = read_file(&fixture.input_path()).unwrap_or_default();
    let output_prettier_content = {
        let path = fixture.output_prettier_path();
        if path.exists() {
            read_file(&path).ok()
        } else {
            None
        }
    };

    // Build content map for classification
    let mut known_files: HashMap<String, String> = HashMap::new();
    known_files.insert(fixture.input_file.clone(), input_content.clone());

    if let Some(ref opc) = output_prettier_content {
        known_files.insert(fixture.output_prettier_filename().to_string(), opc.clone());
    }

    for name in files
        .prettier_variant
        .iter()
        .chain(&files.variant)
        .chain(&files.prettier_intermediate)
        .chain(&files.prettier_intermediate_to_variant)
    {
        if let Ok(content) = read_file(&fixture_dir.join(name)) {
            known_files.insert(name.clone(), content);
        }
    }

    // Collect all files to audit
    let mut files_to_audit: Vec<String> = vec![fixture.input_file.clone()];

    if output_prettier_content.is_some() {
        files_to_audit.push(fixture.output_prettier_filename().to_string());
    }

    files_to_audit.extend(files.unformatted.iter().cloned());
    files_to_audit.extend(files.unformatted_ours.iter().cloned());
    files_to_audit.extend(files.prettier_variant.iter().cloned());
    files_to_audit.extend(files.variant.iter().cloned());
    files_to_audit.extend(files.prettier_intermediate.iter().cloned());
    files_to_audit.extend(files.prettier_intermediate_to_variant.iter().cloned());

    let mut file_audits = Vec::new();
    let mut has_novel = false;
    let mut suggestions = Vec::new();

    // Skip prettier for the no-oracle markers — prettier_nonconvergent.txt (no
    // fixed point) and prettier_rejects.txt (prettier throws). In both cases its
    // output of any file is unclassifiable noise. All input types otherwise audit
    // (prettier_parser() routes .ts/.css correctly).
    let use_prettier = !files.prettier_nonconvergent && !files.prettier_rejects;

    for filename in &files_to_audit {
        let filepath = fixture_dir.join(filename);
        let Ok(content) = read_file(&filepath) else {
            continue;
        };

        // Run our formatter
        let ours_result = match fixtures::format_with_our_formatter(&content, &fixture.input_file) {
            Ok(formatted) => Some(classify_output(&formatted, &content, &known_files)),
            Err(_) => None,
        };

        // Run prettier
        let prettier_result = if use_prettier {
            match run_prettier(&content, prettier_parser).await {
                Ok(formatted) => Some(classify_output(&formatted, &content, &known_files)),
                Err(_) => None,
            }
        } else {
            None
        };

        // Classify novel results and generate suggestions
        let novel_suggestion = classify_novel(
            prettier_result.as_ref(),
            ours_result.as_ref(),
            filename,
            input_ext,
            &input_content,
            fixture,
            use_prettier,
            &known_files,
        )
        .await;

        if let Some(ref s) = novel_suggestion {
            // DocumentedMultiPass is informational — covered by audit_signature.txt and
            // checked byte-for-byte by F4. Don't mark the fixture as novel; it would
            // surface noise on every audit run even though nothing is wrong.
            if !matches!(s, Suggestion::DocumentedMultiPass(_)) {
                has_novel = true;
            }
            suggestions.push(s.clone());
        }

        file_audits.push(FileAudit {
            filename: filename.clone(),
            ours_result,
            prettier_result,
            novel_suggestion,
        });
    }

    FixtureAudit {
        fixture_path: fixture.relative_path.clone(),
        file_audits,
        has_novel,
        suggestions,
    }
}

/// Classify a formatting output against known file contents
fn classify_output(
    output: &str,
    source_content: &str,
    known_files: &HashMap<String, String>,
) -> FormatResult {
    // Check self-idempotent first
    if output == source_content {
        return FormatResult::IdempotentSelf;
    }

    // Check against all known files
    for (name, content) in known_files {
        if output == content {
            if name.starts_with("input.") {
                return FormatResult::MatchesInput;
            } else if name.starts_with("output_prettier.") {
                return FormatResult::MatchesOutputPrettier;
            } else if name.starts_with("prettier_variant_") {
                return FormatResult::MatchesPrettierVariant(name.clone());
            } else if name.starts_with("variant_") {
                return FormatResult::MatchesVariant(name.clone());
            } else if name.starts_with("prettier_intermediate_") {
                return FormatResult::MatchesPrettierIntermediate(name.clone());
            }
        }
    }

    FormatResult::Novel
}

/// Classify a novel Prettier result and generate a suggestion
#[allow(clippy::too_many_arguments)]
async fn classify_novel(
    prettier_result: Option<&FormatResult>,
    ours_result: Option<&FormatResult>,
    filename: &str,
    input_ext: &str,
    input_content: &str,
    fixture: &Fixture,
    use_prettier: bool,
    known_files: &HashMap<String, String>,
) -> Option<Suggestion> {
    // Only interested in novel prettier results
    let FormatResult::Novel = prettier_result? else {
        return None;
    };

    // Get the suffix from the filename — only auto-suggest for unformatted_* source files
    let suffix = if let Some(rest) = filename.strip_prefix("unformatted_ours_") {
        rest.strip_suffix(input_ext).unwrap_or(rest)
    } else if let Some(rest) = filename.strip_prefix("unformatted_") {
        rest.strip_suffix(input_ext).unwrap_or(rest)
    } else {
        // Non-variant source files (input.*, output_prettier.*, prettier_variant_*, etc.)
        // can't generate meaningful variant names. Before flagging, check whether this is
        // a documented prettier non-idempotent case captured in audit_signature.txt.
        if filename.starts_with("output_prettier.") && use_prettier {
            match check_audit_signature(fixture).await {
                AuditSignatureCheck::MatchesRecorded(depth) => {
                    return Some(Suggestion::DocumentedMultiPass(depth));
                }
                AuditSignatureCheck::Drift => {
                    return Some(Suggestion::Investigate(
                        "audit_signature.txt drift — prettier chain from output_prettier no longer matches recorded chain. Run: deno task fixtures:update:formatted".to_string()
                    ));
                }
                AuditSignatureCheck::Error(reason) => {
                    return Some(Suggestion::Investigate(format!(
                        "audit_signature.txt check failed: {reason}"
                    )));
                }
                AuditSignatureCheck::NoSignature => {
                    // Fall through to original "investigate manually" message
                }
            }
        }
        return Some(Suggestion::Investigate(format!(
            "prettier({filename}) produces novel output — investigate manually"
        )));
    };

    if !use_prettier {
        return Some(Suggestion::Investigate(
            "novel output from non-Svelte fixture".to_string(),
        ));
    }

    // Read the source file to get the novel prettier output
    let filepath = fixture.path.join(filename);
    let Ok(content) = read_file(&filepath) else {
        return Some(Suggestion::Investigate("cannot read file".to_string()));
    };

    let prettier_parser = fixture.input_type().prettier_parser();
    let Ok(novel_output) = run_prettier(&content, prettier_parser).await else {
        return Some(Suggestion::Investigate(
            "prettier failed on source".to_string(),
        ));
    };

    // Check if the novel output is prettier-stable (idempotent)
    let Ok(second_pass) = run_prettier(&novel_output, prettier_parser).await else {
        return Some(Suggestion::Investigate(
            "prettier failed on novel output".to_string(),
        ));
    };

    let prettier_stable = second_pass == novel_output;

    if prettier_stable {
        // Check what our formatter does with this novel output
        match fixtures::format_with_our_formatter(&novel_output, &fixture.input_file) {
            Ok(ours_of_novel) => {
                if ours_of_novel == *input_content {
                    // Our formatter normalizes it to input -> prettier_variant_*
                    Some(Suggestion::PrettierVariant(suffix.to_string()))
                } else {
                    // Check if our formatter keeps it stable
                    match fixtures::format_with_our_formatter(&ours_of_novel, &fixture.input_file) {
                        Ok(second) if second == ours_of_novel => {
                            // Our formatter is idempotent on this -> variant_*
                            Some(Suggestion::Variant(suffix.to_string()))
                        }
                        _ => Some(Suggestion::Investigate(
                            "prettier stable but our formatter not idempotent on novel output"
                                .to_string(),
                        )),
                    }
                }
            }
            Err(_) => Some(Suggestion::Investigate(
                "our formatter fails on novel output".to_string(),
            )),
        }
    } else {
        // Prettier unstable — check if it converges to input
        if second_pass == *input_content {
            Some(Suggestion::PrettierIntermediate(suffix.to_string()))
        } else {
            // Does the second pass converge to a documented variant_* / prettier_variant_*?
            let converges_to_variant = known_files.iter().any(|(name, content)| {
                (name.starts_with("variant_") || name.starts_with("prettier_variant_"))
                    && name.ends_with(input_ext)
                    && *content == second_pass
            });
            if converges_to_variant {
                Some(Suggestion::PrettierIntermediateToVariant(
                    suffix.to_string(),
                ))
            } else {
                let ours_normalizes = matches!(ours_result, Some(FormatResult::MatchesInput));
                if ours_normalizes {
                    Some(Suggestion::Investigate(
                        "prettier unstable, does not converge to input or any documented variant"
                            .to_string(),
                    ))
                } else {
                    Some(Suggestion::Investigate(
                        "prettier unstable, non-converging".to_string(),
                    ))
                }
            }
        }
    }
}
