use super::{Command, Executable};
use crate::cli::args::Args;
use crate::cli::input::Input;
use std::process;
use tsv::parse_to_json_with_options;

/// Parse command implementation
pub struct ParseCommand;

impl Command for ParseCommand {
    fn name(&self) -> &str {
        "parse"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let pretty = args.flag("pretty");

        let input = if let Some(content) = args.option("content") {
            // Parse from --content string argument
            Input::from_content(content)
        } else if args.flag("stdin") {
            // Read from stdin (discouraged for agent usage)
            Input::from_stdin()?
        } else if let Some(path) = args.positional() {
            // Read from file
            Input::from_file(&path)?
        } else {
            return Err("No input provided. Use a file path, --content, or --stdin".to_string());
        };

        Ok(Box::new(ParseExecutable { input, pretty }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "parse <file> [--pretty]              Parse file and output AST as JSON".to_string(),
            "parse --content <string> [--pretty]  Parse string and output AST as JSON".to_string(),
            "parse --stdin [--pretty]             Parse stdin (not preferred for agents)"
                .to_string(),
        ]
    }
}

/// Executable instance for parse command
struct ParseExecutable {
    input: Input,
    pretty: bool,
}

impl Executable for ParseExecutable {
    fn execute(&self) {
        match parse_to_json_with_options(self.input.content(), self.pretty) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Parse error: {}", e);
                process::exit(1);
            }
        }
    }
}
