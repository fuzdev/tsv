use crate::deno;
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
        let (input, parser_type) = if let Some(content) = args.option("content") {
            let parser = args
                .option("parser")
                .map(|p| ParserType::from_str(&p))
                .transpose()?
                .unwrap_or(ParserType::Svelte);
            (Input::from_content(content), parser)
        } else if let Some(path) = args.positional() {
            let parser = ParserType::from_extension(&path);
            (Input::from_file(&path)?, parser)
        } else {
            return Err("No input provided. Use a file path or --content".to_string());
        };

        Ok(Box::new(FormatPrettierExecutable { input, parser_type }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "format_prettier <file>                          Format file using prettier"
                .to_string(),
            "format_prettier --content <str> --parser <type>  Format content using prettier"
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
        let content = self.input.content();
        let filepath = match self.parser_type {
            ParserType::Svelte => "temp.svelte",
            ParserType::TypeScript => "temp.ts",
            ParserType::Css => "temp.css",
        };

        match deno::run_prettier(content, filepath) {
            Ok(formatted) => print!("{}", formatted),
            Err(err) => {
                eprintln!("Error formatting with prettier: {}", err);
                std::process::exit(1);
            }
        }
    }
}
