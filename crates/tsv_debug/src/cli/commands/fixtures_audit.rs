use crate::deno::run_prettier;
use crate::fixtures::{
    self, Fixture, InputType, discover_prettier_intermediate_variants,
    discover_prettier_quirk_variants, discover_prettier_stable_variants,
    discover_unformatted_ours_variants, discover_unformatted_variants,
    has_prettier_divergence_suffix, read_file,
};
use futures_util::stream::{self, StreamExt};
use std::collections::HashMap;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// fixtures_audit command - diagnostic tool for investigating fixture normalization graphs
pub struct FixturesAuditCommand;

impl Command for FixturesAuditCommand {
    fn name(&self) -> &str {
        "fixtures_audit"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let verbose = args.flag("verbose") || args.flag("v");
        let all = args.flag("all");
        let json = args.flag("json");

        // Collect remaining args as filters
        let mut filters = Vec::new();
        while let Some(filter) = args.positional() {
            filters.push(filter);
        }

        Ok(Box::new(FixturesAuditExecutable {
            verbose,
            all,
            json,
            filters,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixtures_audit                              Audit _prettier_divergence fixtures"
                .to_string(),
            "fixtures_audit --all                        Audit all fixtures".to_string(),
            "fixtures_audit --verbose                    Show full graph for every fixture"
                .to_string(),
            "fixtures_audit --json                       JSON output".to_string(),
            "fixtures_audit <filter>...                  Audit matching fixtures".to_string(),
        ]
    }
}

struct FixturesAuditExecutable {
    verbose: bool,
    all: bool,
    json: bool,
    filters: Vec<String>,
}

impl Executable for FixturesAuditExecutable {
    fn execute(&self) {
        let rt = crate::cli::commands::create_runtime();
        rt.block_on(self.run());
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
    /// Output matches a prettier_quirk_* file
    MatchesPrettierQuirk(String),
    /// Output matches a prettier_stable_* file
    MatchesPrettierStable(String),
    /// Output matches a prettier_intermediate_* file
    MatchesPrettierIntermediate(String),
    /// Novel output not matching any known file
    Novel,
}

/// Suggestion for a novel result
#[derive(Debug, Clone, serde::Serialize)]
enum Suggestion {
    /// Suggest creating a prettier_quirk_* file
    PrettierQuirk(String),
    /// Suggest creating a prettier_stable_* file
    PrettierStable(String),
    /// Suggest creating a prettier_intermediate_* file
    PrettierIntermediate(String),
    /// Needs investigation
    Investigate(String),
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

impl FixturesAuditExecutable {
    async fn run(&self) {
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
                let dir_name = f.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                has_prettier_divergence_suffix(dir_name)
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

        // Audit fixtures concurrently
        let concurrency = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(4);

        let results: Vec<_> = stream::iter(fixture_list)
            .map(|fixture| async move { audit_fixture(&fixture).await })
            .buffer_unordered(concurrency)
            .collect()
            .await;

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
                    novel_count += 1;
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

fn format_result_label(result: &FormatResult) -> String {
    match result {
        FormatResult::IdempotentSelf => "self".to_string(),
        FormatResult::MatchesInput => "input".to_string(),
        FormatResult::MatchesOutputPrettier => "output_prettier".to_string(),
        FormatResult::MatchesPrettierQuirk(name) => name.clone(),
        FormatResult::MatchesPrettierStable(name) => name.clone(),
        FormatResult::MatchesPrettierIntermediate(name) => name.clone(),
        FormatResult::Novel => "[novel]".to_string(),
    }
}

fn format_suggestion(suggestion: &Suggestion) -> String {
    match suggestion {
        Suggestion::PrettierQuirk(suffix) => {
            format!("suggest prettier_quirk_{suffix} (prettier stable, ours normalizes to input)")
        }
        Suggestion::PrettierStable(suffix) => {
            format!("suggest prettier_stable_{suffix} (both formatters keep stable)")
        }
        Suggestion::PrettierIntermediate(suffix) => {
            format!(
                "suggest prettier_intermediate_{suffix} (prettier unstable, converges to input)"
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

    let prettier_quirk_variants = discover_prettier_quirk_variants(fixture_dir, input_ext);
    for name in &prettier_quirk_variants {
        if let Ok(content) = read_file(&fixture_dir.join(name)) {
            known_files.insert(name.clone(), content);
        }
    }

    let prettier_stable_variants = discover_prettier_stable_variants(fixture_dir, input_ext);
    for name in &prettier_stable_variants {
        if let Ok(content) = read_file(&fixture_dir.join(name)) {
            known_files.insert(name.clone(), content);
        }
    }

    let prettier_intermediate_variants =
        discover_prettier_intermediate_variants(fixture_dir, input_ext);
    for name in &prettier_intermediate_variants {
        if let Ok(content) = read_file(&fixture_dir.join(name)) {
            known_files.insert(name.clone(), content);
        }
    }

    // Collect all files to audit
    let mut files_to_audit: Vec<String> = vec![fixture.input_file.clone()];

    if output_prettier_content.is_some() {
        files_to_audit.push(fixture.output_prettier_filename().to_string());
    }

    let unformatted_variants = discover_unformatted_variants(fixture_dir, input_ext);
    files_to_audit.extend(unformatted_variants);

    let unformatted_ours_variants = discover_unformatted_ours_variants(fixture_dir, input_ext);
    files_to_audit.extend(unformatted_ours_variants);

    files_to_audit.extend(prettier_quirk_variants);
    files_to_audit.extend(prettier_stable_variants);
    files_to_audit.extend(prettier_intermediate_variants);

    let mut file_audits = Vec::new();
    let mut has_novel = false;
    let mut suggestions = Vec::new();

    // Only run prettier for Svelte/SvelteTs fixtures
    let use_prettier = input_type == InputType::Svelte || input_type == InputType::SvelteTs;

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
        )
        .await;

        if novel_suggestion.is_some() {
            has_novel = true;
            if let Some(ref s) = novel_suggestion {
                suggestions.push(s.clone());
            }
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
            } else if name.starts_with("prettier_quirk_") {
                return FormatResult::MatchesPrettierQuirk(name.clone());
            } else if name.starts_with("prettier_stable_") {
                return FormatResult::MatchesPrettierStable(name.clone());
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
        // Non-variant source files (input.*, output_prettier.*, prettier_quirk_*, etc.)
        // can't generate meaningful variant names — flag for investigation
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
                    // Our formatter normalizes it to input -> prettier_quirk_*
                    Some(Suggestion::PrettierQuirk(suffix.to_string()))
                } else {
                    // Check if our formatter keeps it stable
                    match fixtures::format_with_our_formatter(&ours_of_novel, &fixture.input_file) {
                        Ok(second) if second == ours_of_novel => {
                            // Our formatter is idempotent on this -> prettier_stable_*
                            Some(Suggestion::PrettierStable(suffix.to_string()))
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
            let ours_normalizes = matches!(ours_result, Some(FormatResult::MatchesInput));
            if ours_normalizes {
                Some(Suggestion::Investigate(
                    "prettier unstable, does not converge to input".to_string(),
                ))
            } else {
                Some(Suggestion::Investigate(
                    "prettier unstable, non-converging".to_string(),
                ))
            }
        }
    }
}
