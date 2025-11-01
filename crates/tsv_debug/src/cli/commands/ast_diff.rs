use crate::{deno, fixtures};
use std::process::Command as ProcessCommand;
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
        // Two modes:
        // 1. Single input: parse → format → parse → compare
        // 2. Two inputs: parse both → compare
        let content1 = args.option("content");
        let file1 = args.positional();
        let file2 = args.positional();

        let (input1, input2, parser_type) = if let Some(content) = content1 {
            // Single content mode: parse → format → parse
            let parser = args
                .option("parser")
                .map(|p| p.parse())
                .transpose()?
                .unwrap_or(ParserType::Svelte);
            (Input::from_content(content), None, parser)
        } else if let Some(path1) = file1 {
            let parser = ParserType::from_extension(&path1);
            if let Some(path2) = file2 {
                // Two file mode: compare both
                (
                    Input::from_file(&path1)?,
                    Some(Input::from_file(&path2)?),
                    parser,
                )
            } else {
                // Single file mode: parse → format → parse
                (Input::from_file(&path1)?, None, parser)
            }
        } else {
            return Err("No input provided. Use file path(s) or --content".to_string());
        };

        Ok(Box::new(AstDiffExecutable {
            input1,
            input2,
            parser_type,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "ast_diff <file>                       Parse → format → parse → compare ASTs"
                .to_string(),
            "ast_diff <file1> <file2>              Parse both files and compare ASTs".to_string(),
            "ast_diff --content <str> --parser <p> Parse → format → parse → compare ASTs"
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
        let result = if let Some(ref input2) = self.input2 {
            // Two input mode: compare both directly
            compare_two_inputs(&self.input1, input2, self.parser_type)
        } else {
            // Single input mode: parse → format → parse → compare
            compare_round_trip(&self.input1, self.parser_type)
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
                eprintln!("Error: {}", err);
                std::process::exit(1);
            }
        }
    }
}

/// Compare two inputs directly
fn compare_two_inputs(
    input1: &Input,
    input2: &Input,
    parser_type: ParserType,
) -> Result<bool, String> {
    let content1 = input1.content();
    let content2 = input2.content();

    let ast1 = parse_to_json(content1, parser_type)?;
    let ast2 = parse_to_json(content2, parser_type)?;

    compare_asts(&ast1, &ast2)
}

/// Compare round-trip: parse → format → parse → compare
fn compare_round_trip(input: &Input, parser_type: ParserType) -> Result<bool, String> {
    let content = input.content();

    // Parse original
    let ast1 = parse_to_json(content, parser_type)?;

    // Format
    let formatted = format_content(content, parser_type)?;

    // Parse formatted
    let ast2 = parse_to_json(&formatted, parser_type)?;

    compare_asts(&ast1, &ast2)
}

/// Parse content to JSON AST string
fn parse_to_json(content: &str, parser_type: ParserType) -> Result<String, String> {
    match parser_type {
        ParserType::Svelte => deno::parse_svelte(content),
        ParserType::TypeScript => deno::parse_typescript(content),
        ParserType::Css => {
            // Use our Rust parser for CSS
            let output = ProcessCommand::new("cargo")
                .args([
                    "run",
                    "-p",
                    "tsv_cli",
                    "--quiet",
                    "--",
                    "parse",
                    "--content",
                ])
                .arg(content)
                .args(["--pretty"])
                .output()
                .map_err(|e| format!("Failed to execute cargo: {}", e))?;

            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
    }
}

/// Format content using our Rust formatter
fn format_content(content: &str, parser_type: ParserType) -> Result<String, String> {
    let parser_name = match parser_type {
        ParserType::Svelte => "svelte",
        ParserType::TypeScript => "typescript",
        ParserType::Css => "css",
    };

    let output = ProcessCommand::new("cargo")
        .args([
            "run",
            "-p",
            "tsv_cli",
            "--quiet",
            "--",
            "format",
            "--content",
        ])
        .arg(content)
        .args(["--parser", parser_name])
        .output()
        .map_err(|e| format!("Failed to execute cargo: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Compare two AST JSON strings (ignoring spans/locations)
fn compare_asts(json1: &str, json2: &str) -> Result<bool, String> {
    let ast1: serde_json::Value = serde_json::from_str(json1)
        .map_err(|e| format!("Failed to parse first AST JSON: {}", e))?;
    let ast2: serde_json::Value = serde_json::from_str(json2)
        .map_err(|e| format!("Failed to parse second AST JSON: {}", e))?;

    // Remove locations from both
    let ast1_clean = fixtures::remove_locations(ast1);
    let ast2_clean = fixtures::remove_locations(ast2);

    Ok(ast1_clean == ast2_clean)
}
