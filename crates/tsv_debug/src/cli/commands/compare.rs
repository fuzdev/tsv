use anyhow::Result;
use std::process::Command as ProcessCommand;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::{Input, ParserType};

/// Compare command - compares our printer output with prettier
pub struct CompareCommand;

impl Command for CompareCommand {
    fn name(&self) -> &str {
        "compare"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        // Parse input and detect parser type
        let (input, parser_type) = if let Some(content) = args.option("content") {
            // --content requires --parser
            let parser = args
                .option("parser")
                .ok_or("Error: --parser required when using --content")?
                .parse()?;
            (Input::from_content(content), parser)
        } else if args.flag("stdin") {
            // --stdin requires --parser
            let parser = args
                .option("parser")
                .ok_or("Error: --parser required when using --stdin")?
                .parse()?;
            (Input::from_stdin()?, parser)
        } else if let Some(path) = args.positional() {
            let parser = ParserType::from_extension(&path);
            (Input::from_file(&path)?, parser)
        } else {
            return Err("No input provided. Use a file path, --content, or --stdin".to_string());
        };

        Ok(Box::new(CompareExecutable { input, parser_type }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "compare <file>                                  Compare formatter output with prettier for file"
                .to_string(),
            "compare --content <string> --parser <type>      Compare formatter output (requires --parser svelte|typescript|css)"
                .to_string(),
            "compare --stdin --parser <type>                 Compare formatter output from stdin (requires --parser)"
                .to_string(),
        ]
    }
}

/// Executable instance for compare command
struct CompareExecutable {
    input: Input,
    parser_type: ParserType,
}

impl Executable for CompareExecutable {
    fn execute(&self) {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(run(&self.input, self.parser_type));
    }
}

async fn run(input: &Input, parser_type: ParserType) {
    let content = input.content();
    let parser_name = match parser_type {
        ParserType::Svelte => "svelte",
        ParserType::TypeScript => "typescript",
        ParserType::Css => "css",
    };

    println!("=== Input ===");
    println!("{}", content);
    println!();

    // Run our formatter
    println!("=== Our Formatter ===");
    match run_our_formatter(content, parser_name) {
        Ok(output) => println!("{}", output),
        Err(err) => eprintln!("Error running our formatter: {}", err),
    }
    println!();

    // Run prettier
    println!("=== Prettier ===");
    match run_prettier(content, parser_name).await {
        Ok(output) => println!("{}", output),
        Err(err) => eprintln!("Error running prettier: {}", err),
    }
}

fn run_our_formatter(content: &str, parser: &str) -> Result<String, String> {
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
        .args(["--parser", parser])
        .output()
        .map_err(|e| format!("Failed to execute cargo: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

async fn run_prettier(content: &str, parser: &str) -> Result<String> {
    let filepath = match parser {
        "svelte" => "temp.svelte",
        "css" => "temp.css",
        _ => "temp.ts",
    };

    fuz_client::run_prettier(content, filepath).await
}
