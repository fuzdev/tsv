use crate::cli::CliError;
use crate::deno;
use crate::json::to_json_with_tabs;
use argh::FromArgs;
use tsv_cli::cli::input::{Input, InputArgs, ParserType};

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
    pub(crate) fn run(self) -> Result<(), CliError> {
        let (input, parser_type) = super::resolve_input_or_fail(
            InputArgs {
                content: self.content,
                stdin: self.stdin,
                parser: self.parser,
                file: self.file,
            },
            CliError::Failed,
        )?;

        let rt = super::create_runtime();
        match rt.block_on(run(&input, parser_type)) {
            Ok(json) => {
                print!("{json}");
                Ok(())
            }
            Err(err) => {
                eprintln!("Error parsing: {err}");
                Err(CliError::Failed)
            }
        }
    }
}

async fn run(input: &Input, parser_type: ParserType) -> Result<String, String> {
    let content = input.content();

    let ast = deno::parse_by_type(content, parser_type)
        .await
        .map_err(|e| super::describe_deno_error(&e))?;
    let json = to_json_with_tabs(&ast).map_err(|e| e.to_string())?;
    Ok(format!("{json}\n"))
}
