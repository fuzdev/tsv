use crate::deno;
use crate::diff::{DiffOptions, diff_to_string};
use crate::error;
use crate::fixtures;
use argh::FromArgs;
use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::{Input, InputArgs, ParserType};

/// Compare ASTs to verify semantic equivalence (round-trip or two-file).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "ast_diff")]
pub struct AstDiffCommand {
    /// content to parse (requires --parser, single-input round-trip mode)
    #[argh(option)]
    content: Option<String>,

    /// read from stdin (requires --parser, single-input round-trip mode)
    #[argh(switch)]
    stdin: bool,

    /// parser type: svelte | typescript | css
    #[argh(option)]
    parser: Option<ParserType>,

    /// file path(s) — one for round-trip, two for direct compare
    #[argh(positional)]
    files: Vec<String>,
}

impl AstDiffCommand {
    pub fn run(self) {
        if self.files.len() > 2 {
            eprintln!("Error: ast_diff accepts at most two file positionals");
            std::process::exit(1);
        }

        let has_content_or_stdin = self.content.is_some() || self.stdin;
        if has_content_or_stdin && !self.files.is_empty() {
            eprintln!("Error: cannot combine --content/--stdin with file positionals");
            std::process::exit(1);
        }

        // Resolve the primary input
        let (input1, parser_type) = if has_content_or_stdin {
            let input_args = InputArgs {
                content: self.content,
                stdin: self.stdin,
                parser: self.parser,
                file: None,
            };
            match input_args.resolve() {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            let Some(first) = self.files.first().cloned() else {
                eprintln!("Error: No input provided. Use a file path, --content, or --stdin");
                std::process::exit(1);
            };
            let input_args = InputArgs {
                content: None,
                stdin: false,
                parser: self.parser,
                file: Some(first),
            };
            match input_args.resolve() {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        };

        // Optional second file for direct comparison mode
        let input2 = if self.files.len() == 2 {
            match Input::from_file(&self.files[1]) {
                Ok(i) => Some(i),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            None
        };

        let rt = super::create_runtime();
        let result = if let Some(ref input2) = input2 {
            // Two input mode: compare both directly
            rt.block_on(compare_two_inputs(&input1, input2, parser_type))
        } else {
            // Single input mode: parse → format → parse → compare
            rt.block_on(compare_round_trip(&input1, parser_type))
        };

        match result {
            Ok(true) => {
                println!("✓ ASTs match (semantically equivalent)");
            }
            Ok(false) => {
                println!("✗ ASTs differ (semantic change detected)");
                std::process::exit(1);
            }
            Err(err) => {
                eprintln!("Error: {err}");
                std::process::exit(1);
            }
        }
    }
}

/// Compare two inputs directly
async fn compare_two_inputs(
    input1: &Input,
    input2: &Input,
    parser_type: ParserType,
) -> error::Result<bool> {
    let content1 = input1.content();
    let content2 = input2.content();

    let ast1 = parse_to_value(content1, parser_type).await?;
    let ast2 = parse_to_value(content2, parser_type).await?;

    compare_asts(ast1, ast2)
}

/// Compare round-trip: parse → format → parse → compare
async fn compare_round_trip(input: &Input, parser_type: ParserType) -> error::Result<bool> {
    let content = input.content();

    // Parse original
    let ast1 = parse_to_value(content, parser_type).await?;

    // Format
    let formatted = format_content(content, parser_type)?;

    // Parse formatted
    let ast2 = parse_to_value(&formatted, parser_type).await?;

    compare_asts(ast1, ast2)
}

/// Parse content to AST Value
async fn parse_to_value(
    content: &str,
    parser_type: ParserType,
) -> error::Result<serde_json::Value> {
    match parser_type {
        ParserType::Svelte => Ok(deno::parse_svelte(content).await?),
        ParserType::TypeScript => Ok(deno::parse_typescript(content).await?),
        ParserType::Css => Ok(deno::parse_css(content).await?),
    }
}

/// Format content using our Rust printer
fn format_content(content: &str, parser_type: ParserType) -> error::Result<String> {
    format_source(content, parser_type).map_err(error::DebugError::Command)
}

/// Compare two ASTs (ignoring spans/locations)
fn compare_asts(ast1: serde_json::Value, ast2: serde_json::Value) -> error::Result<bool> {
    // Remove locations from both
    let ast1_clean = fixtures::remove_locations(ast1);
    let ast2_clean = fixtures::remove_locations(ast2);

    if ast1_clean == ast2_clean {
        return Ok(true);
    }

    // Show diff when they don't match
    let pretty1 = serde_json::to_string_pretty(&ast1_clean)?;
    let pretty2 = serde_json::to_string_pretty(&ast2_clean)?;

    println!("\n=== AST Diff ===");
    let options = DiffOptions::ast_diff();
    print!("{}", diff_to_string(&pretty1, &pretty2, &options));

    Ok(false)
}
