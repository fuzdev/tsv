use super::{Command, Executable};
use crate::cli::args::Args;
use crate::cli::input::{Input, ParserType};
use std::process;

/// Format command implementation
pub struct FormatCommand;

impl Command for FormatCommand {
    fn name(&self) -> &str {
        "format"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let (input, parser_type) = if let Some(content) = args.option("content") {
            // Format from --content string argument (requires --parser)
            let parser_str = args.required_option("parser")?;
            let parser_type = ParserType::from_str(&parser_str)?;
            (Input::from_content(content), parser_type)
        } else if args.flag("stdin") {
            // Read from stdin (requires --parser)
            let parser_str = args.required_option("parser")?;
            let parser_type = ParserType::from_str(&parser_str)?;
            let input = Input::from_stdin()?;
            (input, parser_type)
        } else if let Some(path) = args.positional() {
            // Read from file, detect type from extension
            let parser_type = ParserType::from_extension(&path);
            let input = Input::from_file(&path)?;
            (input, parser_type)
        } else {
            return Err("No input provided. Use a file path, --content, or --stdin".to_string());
        };

        Ok(Box::new(FormatExecutable { input, parser_type }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "format <file>                               Format file and output formatted code"
                .to_string(),
            "format --content <string> --parser <type>   Format string (preferred)".to_string(),
            "format --stdin --parser <type>              Format stdin (not preferred for agents)"
                .to_string(),
        ]
    }
}

/// Executable instance for format command
struct FormatExecutable {
    input: Input,
    parser_type: ParserType,
}

impl Executable for FormatExecutable {
    fn execute(&self) {
        let source = self.input.content();
        match self.parser_type {
            ParserType::Svelte => match tsv_svelte::parse(source) {
                Ok(ast) => {
                    let formatted = tsv_svelte::format(&ast, source);
                    println!("{}", formatted);
                }
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    process::exit(1);
                }
            },
            ParserType::Css => match tsv_css::parse(source, 0) {
                Ok(ast) => {
                    let formatted = tsv_css::format(&ast, source);
                    println!("{}", formatted);
                }
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    process::exit(1);
                }
            },
            ParserType::TypeScript => match tsv_ts::parse(source) {
                Ok(ast) => {
                    let formatted = tsv_ts::format(&ast);
                    println!("{}", formatted);
                }
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    process::exit(1);
                }
            },
        }
    }
}
