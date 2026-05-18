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
            let parser_type = parser_str.parse()?;
            (Input::from_content(content), parser_type)
        } else if args.flag("stdin") {
            // Read from stdin (requires --parser)
            let parser_str = args.required_option("parser")?;
            let parser_type = parser_str.parse()?;
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
            "format <file>                              Format file, output formatted code"
                .to_string(),
            "format --content <string> --parser <type>  Format string (preferred)".to_string(),
            "format --stdin --parser <type>             Format stdin (not preferred)".to_string(),
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
        let result: Result<String, String> = match self.parser_type {
            ParserType::Svelte => tsv_svelte::parse(source)
                .map(|ast| tsv_svelte::format(&ast, source))
                .map_err(|e| e.to_string()),
            ParserType::Css => tsv_css::parse(source)
                .map(|ast| tsv_css::format(&ast, source))
                .map_err(|e| e.to_string()),
            ParserType::TypeScript => tsv_ts::parse(source)
                .map(|ast| tsv_ts::format(&ast, source))
                .map_err(|e| e.to_string()),
        };

        match result {
            Ok(formatted) => print!("{formatted}"),
            Err(e) => {
                eprintln!("Parse error: {e}");
                process::exit(1);
            }
        }
    }
}
