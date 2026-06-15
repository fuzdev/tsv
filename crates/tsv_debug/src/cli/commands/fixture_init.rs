use crate::deno;
use crate::diff::LINE_WIDTH_THRESHOLD;
use crate::fixtures::{self, InputType, find_input_file};
use argh::FromArgs;
use std::path::Path;
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_lang::printing::visual_width;
use tsv_lang::{PRINT_WIDTH, TAB_WIDTH};

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

    /// fixture directory path
    #[argh(positional)]
    dir: String,
}

impl FixtureInitCommand {
    pub fn run(self) {
        let rt = super::create_runtime();
        rt.block_on(self.run_async());
    }

    async fn run_async(self) {
        let dir = Path::new(&self.dir);

        // Determine input type from --parser flag, existing file, or default
        let input_type = resolve_input_type(self.parser.as_deref(), dir);

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
                std::process::exit(1);
            }
        };

        // Create directory
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("Error creating directory {dir:?}: {e}");
            std::process::exit(1);
        }

        // Format through prettier
        let formatted = match deno::run_prettier(&raw_content, input_type.prettier_parser()).await {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error: prettier formatting failed: {e}");
                std::process::exit(1);
            }
        };

        // Write input file
        let input_filename = format!("input{}", input_type.extension());
        let input_path = dir.join(&input_filename);

        if let Err(e) = fixtures::write_file(&input_path, &formatted) {
            eprintln!("Error writing {input_filename}: {e}");
            std::process::exit(1);
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
        let parse_result = deno::parse_by_type(&formatted, input_type.parser_type()).await;

        match parse_result {
            Ok(ast) => match to_json_with_tabs(&ast) {
                Ok(json) => {
                    let json_content = format!("{json}\n");
                    let expected_path = dir.join("expected.json");
                    match fixtures::write_file(&expected_path, &json_content) {
                        Ok(()) => println!("✓ expected.json"),
                        Err(e) => eprintln!("✗ Failed to write expected.json: {e}"),
                    }
                }
                Err(e) => {
                    eprintln!("⚠ Failed to serialize AST: {e}");
                }
            },
            Err(e) => {
                eprintln!("⚠ Canonical parse failed (expected for TDD): {e}");
            }
        }

        println!("\nFixture initialized: {}", self.dir);
    }
}

/// Resolve input type from --parser flag, existing file, or default (svelte)
fn resolve_input_type(parser: Option<&str>, dir: &Path) -> InputType {
    // --parser flag takes priority
    if let Some(parser) = parser {
        return match parser {
            "svelte" => InputType::Svelte,
            "typescript" | "ts" => InputType::TypeScript,
            "css" => InputType::Css,
            "svelte-ts" | "svelte.ts" => InputType::SvelteTs,
            _ => {
                eprintln!(
                    "Unknown parser type: '{parser}'. Valid: svelte, typescript, css, svelte-ts"
                );
                std::process::exit(1);
            }
        };
    }

    // Auto-detect from existing input file (closed set, so from_filepath
    // always matches; the unwrap_or is the no-file default)
    find_input_file(dir)
        .and_then(InputType::from_filepath)
        .unwrap_or(InputType::Svelte)
}

/// Resolve content from --content, --stdin, or existing input file
fn resolve_content(
    content_flag: Option<&str>,
    use_stdin: bool,
    force: bool,
    dir: &Path,
    input_type: InputType,
) -> Result<String, String> {
    // --content flag
    if let Some(content) = content_flag {
        if !force && find_input_file(dir).is_some() {
            return Err("Input file already exists. Use --force to overwrite.".to_string());
        }
        return Ok(content.to_string());
    }

    // --stdin flag (explicit, consistent with other tsv_debug commands)
    if use_stdin {
        if !force && find_input_file(dir).is_some() {
            return Err("Input file already exists. Use --force to overwrite.".to_string());
        }
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
