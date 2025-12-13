use crate::cli::input_parser;
use crate::error;
use crate::{deno, subprocess};
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
        let (input, parser_type) = input_parser::parse_input_and_parser_type(args)?;
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

async fn run(input: &Input, parser_type: ParserType) -> error::Result<String> {
    let content = input.content();

    match parser_type {
        ParserType::Svelte => Ok(deno::parse_svelte(content).await?),
        ParserType::TypeScript => Ok(deno::parse_typescript(content).await?),
        ParserType::Css => {
            // CSS uses our Rust parser (no external canonical parser available)
            parse_css_with_rust(content)
        }
    }
}

/// Parse CSS using our Rust parser (no external canonical parser available)
fn parse_css_with_rust(content: &str) -> error::Result<String> {
    subprocess::run_tsv_parse(content, false)
}
