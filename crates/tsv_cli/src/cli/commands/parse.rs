use super::{Command, Executable};
use crate::cli::args::Args;
use crate::cli::input::Input;
use std::process;

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
        match parse_to_json(&self.input, self.pretty) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Parse error: {}", e);
                process::exit(1);
            }
        }
    }
}

/// Parse source code and convert to JSON
///
/// Automatically detects whether input is Svelte, TypeScript, or CSS
fn parse_to_json(input: &Input, pretty: bool) -> Result<String, String> {
    use crate::cli::input::ParserType;

    let source = input.content();

    // Try file extension first (most reliable), then fall back to content heuristics
    let parser_type = input.parser_type().unwrap_or_else(|| {
        // Content-based detection for stdin/content input
        let trimmed = source.trim_start();
        if trimmed.starts_with('<') {
            ParserType::Svelte
        } else if trimmed.ends_with('}') || (trimmed.contains('{') && trimmed.contains(':')) {
            ParserType::Css
        } else {
            ParserType::TypeScript
        }
    });

    let json = match parser_type {
        ParserType::Svelte => {
            // Parse as Svelte
            let ast = tsv_svelte::parse(source).map_err(|e| e.to_string())?;
            let public_ast = tsv_svelte::convert_ast(&ast, source);

            // Serialize to JSON
            if pretty {
                serde_json::to_string_pretty(&public_ast)
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
                serde_json::to_string_pretty(&json_value)
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
                serde_json::to_string_pretty(&public_ast)
            } else {
                serde_json::to_string(&public_ast)
            }
        }
    };

    json.map_err(|e| format!("JSON serialization failed: {}", e))
}
