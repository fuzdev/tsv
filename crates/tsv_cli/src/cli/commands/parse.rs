use super::{Command, Executable};
use crate::cli::args::Args;
use crate::cli::input::{Input, ParserType};
use crate::json_utils::to_json_with_tabs;
use std::process;

/// Parse command implementation
pub struct ParseCommand;

impl Command for ParseCommand {
    fn name(&self) -> &str {
        "parse"
    }

    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String> {
        let pretty = args.flag("pretty");
        let explicit_parser: Option<ParserType> =
            args.option("parser").map(|s| s.parse()).transpose()?;

        let (input, parser_type) = if let Some(content) = args.option("content") {
            // Parse from --content string argument (requires --parser)
            let parser_type =
                explicit_parser.ok_or("--content requires --parser <svelte|typescript|css>")?;
            (Input::from_content(content), parser_type)
        } else if args.flag("stdin") {
            // Read from stdin (requires --parser)
            let parser_type =
                explicit_parser.ok_or("--stdin requires --parser <svelte|typescript|css>")?;
            (Input::from_stdin()?, parser_type)
        } else if let Some(path) = args.positional() {
            // Read from file (auto-detects from extension, --parser overrides)
            let parser_type = explicit_parser.unwrap_or_else(|| ParserType::from_extension(&path));
            (Input::from_file(&path)?, parser_type)
        } else {
            return Err("No input provided. Use a file path, --content, or --stdin".to_string());
        };

        Ok(Box::new(ParseExecutable {
            input,
            pretty,
            parser_type,
        }))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "parse <file> [--pretty]                             Parse file, output AST as JSON"
                .to_string(),
            "parse --content <string> --parser <type> [--pretty]  Parse string (preferred)"
                .to_string(),
            "parse --stdin --parser <type> [--pretty]             Parse stdin (not preferred)"
                .to_string(),
        ]
    }
}

/// Executable instance for parse command
struct ParseExecutable {
    input: Input,
    pretty: bool,
    parser_type: ParserType,
}

impl Executable for ParseExecutable {
    fn execute(&self) {
        match parse_to_json(&self.input, self.pretty, self.parser_type) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("Parse error: {e}");
                process::exit(1);
            }
        }
    }
}

/// Parse source code and convert to JSON
fn parse_to_json(input: &Input, pretty: bool, parser_type: ParserType) -> Result<String, String> {
    let source = input.content();

    let json = match parser_type {
        ParserType::Svelte => {
            // Parse as Svelte
            let ast = tsv_svelte::parse(source).map_err(|e| e.to_string())?;
            let public_ast = tsv_svelte::convert_ast(&ast, source);

            // Serialize to JSON
            if pretty {
                to_json_with_tabs(&public_ast)
            } else {
                serde_json::to_string(&public_ast)
            }
        }
        ParserType::Css => {
            // Parse as CSS
            let nodes = tsv_css::parse(source, 0).map_err(|e| e.to_string())?;
            let json_value = tsv_css::convert_ast(&nodes, source);

            // Serialize to JSON
            if pretty {
                to_json_with_tabs(&json_value)
            } else {
                serde_json::to_string(&json_value)
            }
        }
        ParserType::TypeScript => {
            // Parse as TypeScript
            let ast = tsv_ts::parse(source).map_err(|e| e.to_string())?;
            let public_ast = tsv_ts::convert_ast(&ast, source);

            // Serialize to JSON
            if pretty {
                to_json_with_tabs(&public_ast)
            } else {
                serde_json::to_string(&public_ast)
            }
        }
    };

    json.map_err(|e| format!("JSON serialization failed: {e}"))
}
