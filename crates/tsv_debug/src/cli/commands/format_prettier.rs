use crate::cli::input_parser;
use crate::deno::{run_prettier, PrettierParser};
use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};
use tsv_cli::cli::input::{Input, ParserType};

/// format_prettier command - format code using prettier
pub struct FormatPrettierCommand;

impl Command for FormatPrettierCommand {
    fn name(&self) -> &str {
        "format_prettier"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let (input, parser_type) = input_parser::parse_input_and_parser_type(args)?;
        Ok(Box::new(FormatPrettierExecutable { input, parser_type }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "format_prettier <file>                                Format file using prettier"
                .to_string(),
            "format_prettier --content <str> --parser <type>        Format content using prettier (requires --parser)"
                .to_string(),
            "format_prettier --stdin --parser <type>                Format from stdin using prettier (requires --parser)"
                .to_string(),
        ]
    }
}

struct FormatPrettierExecutable {
    input: Input,
    parser_type: ParserType,
}

impl Executable for FormatPrettierExecutable {
    fn execute(&self) {
        let rt = super::create_runtime();
        rt.block_on(run(&self.input, self.parser_type));
    }
}

async fn run(input: &Input, parser_type: ParserType) {
    let content = input.content();
    let parser = match parser_type {
        ParserType::Svelte => PrettierParser::Parser("svelte"),
        ParserType::TypeScript => PrettierParser::Parser("typescript"),
        ParserType::Css => PrettierParser::Parser("css"),
    };

    match run_prettier(content, parser).await {
        Ok(formatted) => print!("{formatted}"),
        Err(err) => {
            eprintln!("Error formatting with prettier: {err}");
            std::process::exit(1);
        }
    }
}
