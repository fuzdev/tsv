use crate::deno;
use crate::fixtures::{self, InputType};
use std::path::Path;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_lang::printing::visual_width;

/// fixture_init command - create or reinitialize a fixture
///
/// Creates the fixture directory, formats content through prettier to produce
/// the canonical input file, and generates expected.json from the canonical parser.
///
/// Content sources (in priority order):
/// 1. `--content` flag
/// 2. `--stdin` flag (for heredocs and pipes)
/// 3. Existing input file in the directory (reformat mode)
pub struct FixtureInitCommand;

impl Command for FixtureInitCommand {
    fn name(&self) -> &str {
        "fixture_init"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let force = args.flag("force");
        let use_stdin = args.flag("stdin");
        let parser = args.option("parser");
        let content = args.option("content");

        let dir = args
            .positional()
            .ok_or("Error: fixture directory path required")?;

        Ok(Box::new(FixtureInitExecutable {
            dir,
            content,
            parser,
            force,
            use_stdin,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "fixture_init <dir>                          Reformat existing input file + regenerate expected.json"
                .to_string(),
            "fixture_init <dir> --content '<code>'       Create fixture from content string"
                .to_string(),
            "fixture_init <dir> --stdin                  Create fixture from stdin (heredoc)"
                .to_string(),
            "fixture_init <dir> --parser typescript       Specify parser type (default: svelte)"
                .to_string(),
            "fixture_init <dir> --force                  Overwrite existing input file".to_string(),
        ]
    }
}

struct FixtureInitExecutable {
    dir: String,
    content: Option<String>,
    parser: Option<String>,
    force: bool,
    use_stdin: bool,
}

impl Executable for FixtureInitExecutable {
    fn execute(&self) {
        let rt = super::create_runtime();
        rt.block_on(self.run());
    }
}

impl FixtureInitExecutable {
    async fn run(&self) {
        let dir = Path::new(&self.dir);

        // Determine input type from --parser flag, existing file, or default
        let input_type = resolve_input_type(self.parser.as_deref(), dir);

        // Get content from --content, --stdin, or existing file
        let raw_content = match resolve_content(
            self.content.as_deref(),
            self.use_stdin,
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
        let parse_result = match input_type {
            InputType::Svelte => deno::parse_svelte(&formatted).await,
            InputType::SvelteTs | InputType::TypeScript => deno::parse_typescript(&formatted).await,
            InputType::Css => deno::parse_css(&formatted).await,
        };

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

    // Auto-detect from existing input file
    if let Some(input_file) = find_input_file(dir) {
        return input_file_to_type(input_file);
    }

    // Default to svelte
    InputType::Svelte
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

/// Find the input file in a directory, if any
fn find_input_file(dir: &Path) -> Option<&'static str> {
    if dir.join("input.svelte").exists() {
        Some("input.svelte")
    } else if dir.join("input.svelte.ts").exists() {
        Some("input.svelte.ts")
    } else if dir.join("input.ts").exists() {
        Some("input.ts")
    } else if dir.join("input.css").exists() {
        Some("input.css")
    } else {
        None
    }
}

/// Convert input filename to InputType
fn input_file_to_type(input_file: &str) -> InputType {
    if input_file.ends_with(".svelte.ts") {
        InputType::SvelteTs
    } else if input_file.ends_with(".ts") {
        InputType::TypeScript
    } else if input_file.ends_with(".css") {
        InputType::Css
    } else {
        InputType::Svelte
    }
}

/// Print a compact line width summary for the formatted input.
///
/// Shows lines at or near print_width (90+), max width, and warns for `_long`
/// directories where nothing is near the boundary.
fn print_line_width_summary(content: &str, dir_path: &str) {
    let tab_width = 2;
    let print_width = 100;
    let threshold = 90; // Show lines at 90+ chars

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return;
    }

    let mut max_width = 0;
    let mut max_line_num = 0;
    let mut notable_lines: Vec<(usize, usize)> = Vec::new(); // (line_num, width)

    for (idx, line) in lines.iter().enumerate() {
        let width = visual_width(line, tab_width);
        if width > max_width {
            max_width = width;
            max_line_num = idx + 1;
        }
        if width >= threshold {
            notable_lines.push((idx + 1, width));
        }
    }

    // Print notable lines (at/near/over print_width)
    if notable_lines.is_empty() {
        println!("  max width: {max_width} (line {max_line_num})");
    } else {
        for &(line_num, width) in &notable_lines {
            let marker = if width > print_width {
                "✗ EXCEEDS"
            } else if width == print_width {
                "⚠ EXACTLY"
            } else {
                " "
            };
            println!("  line {line_num}: {width} chars {marker}");
        }
    }

    // Warn for _long directories where nothing is near print_width
    let is_long_fixture = dir_path.contains("_long") || dir_path.ends_with("/long");
    if is_long_fixture && max_width < threshold {
        eprintln!(
            "⚠ Warning: directory name suggests a boundary test but max width is {max_width} (need ~{print_width})"
        );
    }
}
