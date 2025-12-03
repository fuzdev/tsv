use crate::error::DebugError;
use std::process::Command as ProcessCommand;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::{Input, ParserType};

/// canonical_parse command - parse using canonical external parsers
/// (Svelte's official parser, acorn+typescript, or our CSS parser)
pub struct CanonicalParseCommand;

impl Command for CanonicalParseCommand {
    fn name(&self) -> &str {
        "canonical_parse"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
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

        Ok(Box::new(CanonicalParseExecutable { input, parser_type }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "canonical_parse <file>                           Parse file using canonical parser"
                .to_string(),
            "canonical_parse --content <str> --parser <type>  Parse content using canonical parser (requires --parser)"
                .to_string(),
            "canonical_parse --stdin --parser <type>          Parse from stdin using canonical parser (requires --parser)"
                .to_string(),
        ]
    }
}

struct CanonicalParseExecutable {
    input: Input,
    parser_type: ParserType,
}

impl Executable for CanonicalParseExecutable {
    fn execute(&self) {
        let rt = super::create_runtime();
        let result = rt.block_on(run(&self.input, self.parser_type));

        match result {
            Ok(json) => print!("{json}"),
            Err(err) => {
                eprintln!("Error parsing: {err}");
                std::process::exit(1);
            }
        }
    }
}

async fn run(input: &Input, parser_type: ParserType) -> crate::error::Result<String> {
    let content = input.content();

    match parser_type {
        ParserType::Svelte => Ok(fuz_client::parse_svelte(content).await?),
        ParserType::TypeScript => Ok(fuz_client::parse_typescript(content).await?),
        ParserType::Css => {
            // CSS uses our Rust parser (no external canonical parser available)
            parse_css_with_rust(content)
        }
    }
}

/// Parse CSS using our Rust parser (no external canonical parser available)
fn parse_css_with_rust(content: &str) -> crate::error::Result<String> {
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
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(DebugError::Command(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
}
