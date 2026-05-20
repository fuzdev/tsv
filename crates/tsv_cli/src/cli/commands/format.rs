use crate::cli::input::{InputArgs, ParserType};
use argh::FromArgs;
use std::process;

/// Format source code (matches Prettier output).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "format")]
pub struct FormatCommand {
    /// content to format (requires --parser)
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

impl FormatCommand {
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

        let source = input.content();
        let result: Result<String, String> = match parser_type {
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
