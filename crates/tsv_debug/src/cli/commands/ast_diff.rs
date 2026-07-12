use crate::cli::CliError;
use crate::deno;
use crate::diff::{DiffOptions, diff_to_string};
use crate::error;
use crate::render_normalize::normalize_pair;
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

    /// normalize both ASTs per Svelte 5 render-time whitespace rules before
    /// comparing (confirms render-equivalence, e.g. for block-style content)
    #[argh(switch)]
    render: bool,

    /// file path(s) — one for round-trip, two for direct compare
    #[argh(positional)]
    files: Vec<String>,
}

impl AstDiffCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        if self.files.len() > 2 {
            eprintln!("Error: ast_diff accepts at most two file positionals");
            return Err(CliError::Failed);
        }

        let has_content_or_stdin = self.content.is_some() || self.stdin;
        if has_content_or_stdin && !self.files.is_empty() {
            eprintln!("Error: cannot combine --content/--stdin with file positionals");
            return Err(CliError::Failed);
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
                    return Err(CliError::Failed);
                }
            }
        } else {
            let Some(first) = self.files.first().cloned() else {
                eprintln!("Error: No input provided. Use a file path, --content, or --stdin");
                return Err(CliError::Failed);
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
                    return Err(CliError::Failed);
                }
            }
        };

        // Optional second file for direct comparison mode
        let input2 = if self.files.len() == 2 {
            match Input::from_file(&self.files[1]) {
                Ok(i) => Some(i),
                Err(e) => {
                    eprintln!("Error: {e}");
                    return Err(CliError::Failed);
                }
            }
        } else {
            None
        };

        let rt = super::create_runtime();
        let result = if let Some(ref input2) = input2 {
            // Two input mode: compare both directly
            rt.block_on(compare_two_inputs(
                &input1,
                input2,
                parser_type,
                self.render,
            ))
        } else {
            // Single input mode: parse → format → parse → compare
            rt.block_on(compare_round_trip(&input1, parser_type, self.render))
        };

        match result {
            Ok(true) => {
                println!("✓ ASTs match (semantically equivalent)");
                Ok(())
            }
            Ok(false) => {
                println!("✗ ASTs differ (semantic change detected)");
                Err(CliError::Failed)
            }
            Err(err) => {
                eprintln!("Error: {err}");
                Err(CliError::Failed)
            }
        }
    }
}

/// Compare two inputs directly
async fn compare_two_inputs(
    input1: &Input,
    input2: &Input,
    parser_type: ParserType,
    render: bool,
) -> error::Result<bool> {
    let content1 = input1.content();
    let content2 = input2.content();

    let ast1 = parse_to_value(content1, parser_type).await?;
    let ast2 = parse_to_value(content2, parser_type).await?;

    compare_asts(ast1, ast2, render)
}

/// Compare round-trip: parse → format → parse → compare
async fn compare_round_trip(
    input: &Input,
    parser_type: ParserType,
    render: bool,
) -> error::Result<bool> {
    let content = input.content();

    // Parse original
    let ast1 = parse_to_value(content, parser_type).await?;

    // Format
    let formatted = format_content(content, parser_type)?;

    // Parse formatted
    let ast2 = parse_to_value(&formatted, parser_type).await?;

    compare_asts(ast1, ast2, render)
}

/// Parse content to AST Value
async fn parse_to_value(
    content: &str,
    parser_type: ParserType,
) -> error::Result<serde_json::Value> {
    Ok(deno::parse_by_type(content, parser_type).await?)
}

/// Format content using our Rust printer
fn format_content(content: &str, parser_type: ParserType) -> error::Result<String> {
    format_source(content, parser_type).map_err(error::DebugError::Command)
}

/// Compare two ASTs (ignoring spans/locations).
///
/// With `render`, both ASTs are first normalized per Svelte 5 render-time
/// whitespace rules (via [`normalize_pair`]) so render-equivalent forms — e.g.
/// `<small>text</small>` vs block-style `<small>⏎\ttext⏎</small>` — compare
/// equal even though the parser keeps the boundary whitespace verbatim.
fn compare_asts(
    ast1: serde_json::Value,
    ast2: serde_json::Value,
    render: bool,
) -> error::Result<bool> {
    let (ast1_clean, ast2_clean) = normalize_pair(ast1, ast2, render);

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
