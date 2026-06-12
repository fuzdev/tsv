use crate::deno;
use crate::error;
use argh::FromArgs;
use tsv_cli::cli::input::{Input, InputArgs, ParserType};
use tsv_cli::json_utils::to_json_with_tabs;

/// Parse using canonical external parsers (Svelte, acorn+typescript, parseCss).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "canonical_parse")]
pub struct CanonicalParseCommand {
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

impl CanonicalParseCommand {
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
                std::process::exit(1);
            }
        };

        let rt = super::create_runtime();
        match rt.block_on(run(&input, parser_type)) {
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
