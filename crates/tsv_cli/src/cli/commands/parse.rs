use crate::cli::input::{InputArgs, ParserType};
use crate::json_utils::to_json_with_tabs;
use argh::FromArgs;
use std::process;

/// Parse source code into AST JSON.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "parse")]
pub struct ParseCommand {
    /// pretty-print JSON output
    #[argh(switch)]
    pretty: bool,

    /// content to parse (requires --parser)
    #[argh(option)]
    content: Option<String>,

    /// read from stdin (requires --parser)
    #[argh(switch)]
    stdin: bool,

    /// parser type: svelte | typescript | css
    #[argh(option)]
    parser: Option<ParserType>,

    /// file path (parser auto-detected from extension)
    #[argh(positional)]
    file: Option<String>,
}

impl ParseCommand {
    pub fn run(self) {
        let input_args = InputArgs {
            content: self.content,
            stdin: self.stdin,
            parser: self.parser,
            file: self.file,
        };
        let (input, parser_type) = match input_args.resolve() {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("Error: {e}");
                process::exit(1);
            }
        };

        match parse_to_json(input.content(), self.pretty, parser_type) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("Parse error: {e}");
                process::exit(1);
            }
        }
    }
}

fn parse_to_json(source: &str, pretty: bool, parser_type: ParserType) -> Result<String, String> {
    let json_value = match parser_type {
        ParserType::Svelte => {
            let ast = tsv_svelte::parse(source).map_err(|e| e.to_string())?;
            tsv_svelte::convert_ast_json(&ast, source)
        }
        ParserType::Css => {
            let ast = tsv_css::parse(source).map_err(|e| e.to_string())?;
            tsv_css::convert_ast_json(&ast, source)
        }
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(source).map_err(|e| e.to_string())?;
            tsv_ts::convert_ast_json(&ast, source)
        }
    };

    let json = if pretty {
        to_json_with_tabs(&json_value)
    } else {
        serde_json::to_string(&json_value)
    };
    json.map_err(|e| format!("JSON serialization failed: {e}"))
}
