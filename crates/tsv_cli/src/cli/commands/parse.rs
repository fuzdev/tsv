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

    /// parse goal for TypeScript: script | module (default: module). `script`
    /// parses a standalone strict script — `await` is an ordinary identifier and
    /// `import`/`export`/`import.meta` are errors. Ignored for svelte/css.
    #[argh(option)]
    goal: Option<String>,

    /// file path (parser auto-detected from extension)
    #[argh(positional)]
    file: Option<String>,
}

impl ParseCommand {
    pub fn run(self) {
        let goal = match parse_goal_arg(self.goal.as_deref()) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("Error: {e}");
                process::exit(1);
            }
        };
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

        match parse_to_json(input.content(), self.pretty, parser_type, goal) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("Parse error: {e}");
                process::exit(1);
            }
        }
    }
}

/// Parse the `--goal` argument into a [`tsv_ts::Goal`]. Absent → `Module` (the
/// default); `module`/`script` map to the goals. Shared by `parse` and `format`.
pub fn parse_goal_arg(goal: Option<&str>) -> Result<tsv_ts::Goal, String> {
    match goal {
        None | Some("module") => Ok(tsv_ts::Goal::Module),
        Some("script") => Ok(tsv_ts::Goal::Script),
        Some(other) => Err(format!(
            "invalid --goal '{other}' (expected 'script' or 'module')"
        )),
    }
}

fn parse_to_json(
    source: &str,
    pretty: bool,
    parser_type: ParserType,
    goal: tsv_ts::Goal,
) -> Result<String, String> {
    // Compact output uses the convert_ast_json_string hot path (skips the
    // intermediate serde_json::Value when eligible); pretty-printing needs
    // the Value for tab-indented serialization.
    // The arena owns the internal AST; convert produces owned JSON, so nothing
    // borrowed escapes this function. Pre-sized to the source to avoid the
    // bump's chunk-doubling tail on the parse.
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    macro_rules! emit {
        ($lang:ident, $parse:expr) => {{
            let ast = $parse.map_err(|e| e.to_string())?;
            if pretty {
                to_json_with_tabs(&$lang::convert_ast_json(&ast, source))
                    .map_err(|e| format!("JSON serialization failed: {e}"))?
            } else {
                $lang::convert_ast_json_string(&ast, source)
            }
        }};
    }

    // The goal applies only to TypeScript; svelte is always a module and css has
    // no goal.
    Ok(match parser_type {
        ParserType::Svelte => emit!(tsv_svelte, tsv_svelte::parse(source, &arena)),
        ParserType::Css => emit!(tsv_css, tsv_css::parse(source, &arena)),
        ParserType::TypeScript => emit!(tsv_ts, tsv_ts::parse_with_goal(source, goal, &arena)),
    })
}
