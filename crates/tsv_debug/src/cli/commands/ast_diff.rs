use crate::fixtures;
use anyhow::Result;
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
        let use_stdin = args.flag("stdin");
        let file1 = args.positional();
        let file2 = args.positional();

        let (input1, input2, parser_type) = if let Some(content) = content1 {
            // Single content mode: parse → format → parse
            let parser = args
                .option("parser")
                .ok_or("Error: --parser required when using --content")?
                .parse()?;
            (Input::from_content(content), None, parser)
        } else if use_stdin {
            // Single stdin mode: parse → format → parse
            let parser = args
                .option("parser")
                .ok_or("Error: --parser required when using --stdin")?
                .parse()?;
            (Input::from_stdin()?, None, parser)
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
            return Err("No input provided. Use file path(s), --content, or --stdin".to_string());
        };

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
) -> Result<bool> {
    let content1 = input1.content();
    let content2 = input2.content();

    let ast1 = parse_to_json(content1, parser_type).await?;
    let ast2 = parse_to_json(content2, parser_type).await?;

    compare_asts(&ast1, &ast2)
}

/// Compare round-trip: parse → format → parse → compare
async fn compare_round_trip(input: &Input, parser_type: ParserType) -> Result<bool> {
    let content = input.content();

    // Parse original
    let ast1 = parse_to_json(content, parser_type).await?;

    // Format
    let formatted = format_content(content, parser_type)?;

    // Parse formatted
    let ast2 = parse_to_json(&formatted, parser_type).await?;

    compare_asts(&ast1, &ast2)
}

/// Parse content to JSON AST string
async fn parse_to_json(content: &str, parser_type: ParserType) -> Result<String> {
    match parser_type {
        ParserType::Svelte => fuz_client::parse_svelte(content).await,
        ParserType::TypeScript => fuz_client::parse_typescript(content).await,
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
                .output()?;

            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr))
            }
        }
    }
}

/// Format content using our Rust printer
fn format_content(content: &str, parser_type: ParserType) -> Result<String> {
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
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr))
    }
}

/// Compare two AST JSON strings (ignoring spans/locations)
fn compare_asts(json1: &str, json2: &str) -> Result<bool> {
    let ast1: serde_json::Value = serde_json::from_str(json1)?;
    let ast2: serde_json::Value = serde_json::from_str(json2)?;

    // Remove locations from both
    let ast1_clean = fixtures::remove_locations(ast1);
    let ast2_clean = fixtures::remove_locations(ast2);

    Ok(ast1_clean == ast2_clean)
}
