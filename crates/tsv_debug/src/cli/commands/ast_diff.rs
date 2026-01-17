use crate::cli::input_parser;
use crate::diff::{DiffOptions, diff_to_string};
use crate::error;
use crate::fixtures;
use crate::{deno, subprocess};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::{Input, ParserType};

/// ast_diff command - compare ASTs to verify semantic equivalence
pub struct AstDiffCommand;

impl Command for AstDiffCommand {
    fn name(&self) -> &str {
        "ast_diff"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        // Parse first input
        let (input1, parser_type) = input_parser::parse_input_and_parser_type(args)?;

        // Parse optional second input (for two-file comparison mode)
        let input2 = input_parser::parse_optional_second_input(args, parser_type)?;

        Ok(Box::new(AstDiffExecutable {
            input1,
            input2,
            parser_type,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "ast_diff <file>                                 Parse → format → parse → compare ASTs"
                .to_string(),
            "ast_diff <file1> <file2>                        Parse both files and compare ASTs".to_string(),
            "ast_diff --content <str> --parser <type>        Parse → format → parse (requires --parser)"
                .to_string(),
            "ast_diff --stdin --parser <type>                Parse → format → parse from stdin (requires --parser)"
                .to_string(),
        ]
    }
}

struct AstDiffExecutable {
    input1: Input,
    input2: Option<Input>,
    parser_type: ParserType,
}

impl Executable for AstDiffExecutable {
    fn execute(&self) {
        let rt = super::create_runtime();
        let result = if let Some(ref input2) = self.input2 {
            // Two input mode: compare both directly
            rt.block_on(compare_two_inputs(&self.input1, input2, self.parser_type))
        } else {
            // Single input mode: parse → format → parse → compare
            rt.block_on(compare_round_trip(&self.input1, self.parser_type))
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
        ParserType::Css => {
            // CSS subprocess returns JSON string, parse to Value
            let json_str = subprocess::run_tsv_parse(content, true)?;
            Ok(serde_json::from_str(&json_str)?)
        }
    }
}

/// Format content using our Rust printer
fn format_content(content: &str, parser_type: ParserType) -> error::Result<String> {
    let parser_name = match parser_type {
        ParserType::Svelte => "svelte",
        ParserType::TypeScript => "typescript",
        ParserType::Css => "css",
    };

    subprocess::run_tsv_format(content, parser_name)
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
