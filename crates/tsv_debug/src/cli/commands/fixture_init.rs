use crate::cli::CliError;
use crate::deno;
use crate::diff::LINE_WIDTH_THRESHOLD;
use crate::fixtures::{self, CanonicalParseError, GOAL_FILENAME, InputType, find_input_file};
use argh::FromArgs;
use std::path::Path;
use tsv_lang::printing::visual_width;
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};
use tsv_ts::Goal;

/// Create or reinitialize a fixture (formats through prettier + generates expected.json).
///
/// Content sources (in priority order): `--content`, `--stdin`, existing input file.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "fixture_init")]
pub struct FixtureInitCommand {
    /// overwrite existing input file
    #[argh(switch)]
    force: bool,

    /// read content from stdin (for heredocs and pipes)
    #[argh(switch)]
    stdin: bool,

    /// parser type: svelte | typescript | ts | css | svelte-ts | svelte.ts
    #[argh(option)]
    parser: Option<String>,

    /// content string
    #[argh(option)]
    content: Option<String>,

    /// parse goal for a TypeScript fixture: script | module. `script` writes the
    /// `goal` marker and generates expected.json at acorn's `sourceType: 'script'`;
    /// `module` removes any marker. Either move happens only on a successful
    /// regeneration of expected.json — on failure the marker is left as it was.
    /// Omitting the flag keeps the directory's existing marker, and no marker means
    /// module. Rejected for svelte/css inputs, which have no goal.
    #[argh(option)]
    goal: Option<String>,

    /// fixture directory path
    #[argh(positional)]
    dir: String,
}

impl FixtureInitCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let rt = super::create_runtime();
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> Result<(), CliError> {
        let dir = Path::new(&self.dir);

        // Determine input type from --parser flag, existing file, or default
        let input_type = resolve_input_type(self.parser.as_deref(), dir)?;

        // The parse goal applies to the TS family only. An explicit --goal
        // (re)writes the `goal` marker below; without one the fixture keeps
        // whatever goal its directory already declares, so a bare reinit of a
        // script fixture regenerates expected.json at the goal it is graded at.
        let goal_flag = match self.goal {
            None => None,
            Some(ref goal_arg) => {
                if !matches!(input_type, InputType::TypeScript | InputType::SvelteTs) {
                    eprintln!(
                        "Error: --goal applies to .ts / .svelte.ts fixtures; svelte <script> is always a module and css has no goal"
                    );
                    return Err(CliError::Failed);
                }
                // This flag keeps the marker file's own word, `goal`, rather
                // than the shipped CLIs' `--source-type`, so it states the
                // expectation itself instead of borrowing their message.
                match Goal::from_source_type(goal_arg) {
                    Some(goal) => Some(goal),
                    None => {
                        eprintln!(
                            "Error: invalid --goal '{goal_arg}' (expected 'script' or 'module')"
                        );
                        return Err(CliError::Failed);
                    }
                }
            }
        };
        let goal = goal_flag.unwrap_or_else(|| fixtures::read_goal_marker(dir));

        // Get content from --content, --stdin, or existing file
        let raw_content = match resolve_content(
            self.content.as_deref(),
            self.stdin,
            self.force,
            dir,
            input_type,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Failed);
            }
        };

        // Create directory (refusing the wrong-cwd nested-fixture-root footgun)
        if let Err(e) = fixtures::create_fixture_dir(dir) {
            eprintln!("Error: {e}");
            return Err(CliError::Failed);
        }

        // Format through prettier
        let formatted = match deno::run_prettier(&raw_content, input_type.prettier_parser()).await {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: prettier formatting failed: {e}");
                return Err(CliError::Failed);
            }
        };

        // Write input file
        let input_filename = format!("input{}", input_type.extension());
        let input_path = dir.join(&input_filename);

        if let Err(e) = fixtures::write_file(&input_path, &formatted) {
            eprintln!("Error writing {input_filename}: {e}");
            return Err(CliError::Failed);
        }
        println!("✓ {input_filename} (prettier-formatted)");

        // Verify idempotency
        match deno::run_prettier(&formatted, input_type.prettier_parser()).await {
            Ok(reformatted) => {
                if reformatted != formatted {
                    eprintln!(
                        "⚠ Warning: input is not prettier-idempotent (formatting it again produces different output)"
                    );
                }
            }
            Err(e) => {
                eprintln!("⚠ Warning: idempotency check failed: {e}");
            }
        }

        // Show line width summary
        print_line_width_summary(&formatted, &self.dir);

        // Generate expected.json from canonical parser
        let canonical = fixtures::canonical_expected_json(&formatted, input_type, goal).await;

        let expected_written = match canonical {
            Ok(json_content) => {
                let expected_path = dir.join("expected.json");
                match fixtures::write_file(&expected_path, &json_content) {
                    Ok(()) => {
                        println!("✓ expected.json");
                        true
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to write expected.json: {e}");
                        false
                    }
                }
            }
            Err(CanonicalParseError::Unserializable(message)) => {
                eprintln!("⚠ {message}");
                false
            }
            Err(CanonicalParseError::Rejected(message)) => {
                eprintln!("⚠ Canonical parse failed (expected for TDD): {message}");
                false
            }
            // A fault is no verdict on the input, so it fails the command rather than
            // reading as the TDD rejection above
            Err(CanonicalParseError::Sidecar(e)) => {
                eprintln!(
                    "Error: {}",
                    fixtures::canonical_sidecar_failure(input_type, &e)
                );
                return Err(CliError::Failed);
            }
        };

        // The `goal` marker travels with the expected.json it describes:
        // `Script` writes it, `Module` (the default) is spelled by its absence,
        // so a reinit that flips the goal drops a stale marker. It moves only
        // once the parse at that goal has landed — a marker written beside an
        // expected.json generated at the other goal would be a fixture claiming
        // two goals at once.
        if let Some(goal) = goal_flag {
            if expected_written {
                if let Err(e) = write_goal_marker(dir, goal) {
                    eprintln!("Error: {e}");
                    return Err(CliError::Failed);
                }
            } else {
                eprintln!(
                    "⚠ {GOAL_FILENAME} left unchanged (expected.json was not regenerated, so the marker would disagree with it)"
                );
            }
        }

        println!("\nFixture initialized: {}", self.dir);
        Ok(())
    }
}

/// Write or clear the fixture's `goal` marker.
///
/// `Goal::Script` writes the marker (`script`, the spelling the fixture model
/// reads back); `Goal::Module` is the default goal and is spelled by the
/// marker's absence, so an existing marker is removed rather than rewritten.
fn write_goal_marker(dir: &Path, goal: Goal) -> Result<(), String> {
    let path = fixtures::goal_marker_path(dir);
    match goal {
        Goal::Script => {
            fixtures::write_file(&path, &format!("{}\n", goal.source_type()))?;
            println!("✓ {GOAL_FILENAME} ({})", goal.source_type());
            Ok(())
        }
        Goal::Module => {
            if fixtures::remove_if_present(&path)? {
                println!("✓ {GOAL_FILENAME} removed (module is the default goal)");
            }
            Ok(())
        }
    }
}

/// Resolve input type from --parser flag, existing file, or default (svelte).
///
/// # Errors
///
/// Returns [`CliError::Failed`] when `--parser` names an unknown type.
fn resolve_input_type(parser: Option<&str>, dir: &Path) -> Result<InputType, CliError> {
    // --parser flag takes priority
    if let Some(parser) = parser {
        return Ok(match parser {
            "svelte" => InputType::Svelte,
            "typescript" | "ts" => InputType::TypeScript,
            "css" => InputType::Css,
            "svelte-ts" | "svelte.ts" => InputType::SvelteTs,
            _ => {
                eprintln!(
                    "Unknown parser type: '{parser}'. Valid: svelte, typescript, css, svelte-ts"
                );
                return Err(CliError::Failed);
            }
        });
    }

    // Auto-detect from existing input file (closed set, so from_filepath
    // always matches; the unwrap_or is the no-file default)
    Ok(find_input_file(dir)
        .and_then(InputType::from_filepath)
        .unwrap_or(InputType::Svelte))
}

/// Resolve content from --content, --stdin, or existing input file
fn resolve_content(
    content_flag: Option<&str>,
    use_stdin: bool,
    force: bool,
    dir: &Path,
    input_type: InputType,
) -> Result<String, String> {
    // --content / --stdin write a new input, so neither may replace one without --force
    if (content_flag.is_some() || use_stdin) && !force && find_input_file(dir).is_some() {
        return Err("Input file already exists. Use --force to overwrite.".to_string());
    }

    // --content flag
    if let Some(content) = content_flag {
        return Ok(content.to_string());
    }

    // --stdin flag (explicit, consistent with other tsv_debug commands)
    if use_stdin {
        let mut buffer = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
            .map_err(|e| format!("Failed to read stdin: {e}"))?;
        if buffer.is_empty() {
            return Err("No content received from stdin.".to_string());
        }
        return Ok(buffer);
    }

    // Existing input file (reformat mode)
    let input_filename = format!("input{}", input_type.extension());
    let input_path = dir.join(&input_filename);
    if input_path.exists() {
        return fixtures::read_file(&input_path);
    }

    Err(
        "No content source. Provide --content, --stdin (heredoc), or ensure input file exists."
            .to_string(),
    )
}

/// Print a compact line width summary for the formatted input.
///
/// Shows lines at or near PRINT_WIDTH (90+), max width, and warns for `_long`
/// directories where nothing is near the boundary.
fn print_line_width_summary(content: &str, dir_path: &str) {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return;
    }

    let mut max_width = 0;
    let mut max_line_num = 0;
    let mut notable_lines: Vec<(usize, usize)> = Vec::new(); // (line_num, width)

    for (idx, line) in lines.iter().enumerate() {
        let width = visual_width(line, TAB_WIDTH);
        if width > max_width {
            max_width = width;
            max_line_num = idx + 1;
        }
        if width >= LINE_WIDTH_THRESHOLD {
            notable_lines.push((idx + 1, width));
        }
    }

    // Print notable lines (at/near/over PRINT_WIDTH)
    if notable_lines.is_empty() {
        println!("  max width: {max_width} (line {max_line_num})");
    } else {
        for &(line_num, width) in &notable_lines {
            let marker = if width > PRINT_WIDTH {
                "✗ EXCEEDS"
            } else if width == PRINT_WIDTH {
                "⚠ EXACTLY"
            } else {
                " "
            };
            println!("  line {line_num}: {width} chars {marker}");
        }
    }

    // Warn for _long directories where nothing is near PRINT_WIDTH
    let is_long_fixture = dir_path.contains("_long") || dir_path.ends_with("/long");
    if is_long_fixture && max_width < LINE_WIDTH_THRESHOLD {
        eprintln!(
            "⚠ Warning: directory name suggests a boundary test but max width is {max_width} (need ~{PRINT_WIDTH})"
        );
    }
}
