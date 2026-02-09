use crate::cli::input_parser;
use crate::deno;
use crate::error;
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::{Input, ParserType};
use tsv_cli::json_utils::to_json_with_tabs;

/// canonical_parse command - parse using canonical external parsers
/// (Svelte's official parser, acorn+typescript, or Svelte's parseCss)
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
        ParserType::Svelte => {
            let ast = deno::parse_svelte(content).await?;
            Ok(format!("{}\n", to_json_with_tabs(&ast)?))
        }
        ParserType::TypeScript => {
            let ast = deno::parse_typescript(content).await?;
            Ok(format!("{}\n", to_json_with_tabs(&ast)?))
        }
        ParserType::Css => {
            let ast = deno::parse_css(content).await?;
            Ok(format!("{}\n", to_json_with_tabs(&ast)?))
        }
    }
}
