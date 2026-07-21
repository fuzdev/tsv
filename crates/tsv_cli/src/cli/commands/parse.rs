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

    /// omit per-node `loc` (line/column). Emits `start`/`end` offsets only — the
    /// opt-in span-only wire (mirrors acorn's `locations: false`). `loc` is
    /// derivable from the offsets plus source, so nothing is lost for a consumer
    /// that has the source. No-op for css (`parseCss` emits no `loc`).
    #[argh(switch)]
    no_locations: bool,

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

        match parse_to_json(
            input.content(),
            self.pretty,
            parser_type,
            goal,
            !self.no_locations,
        ) {
            Ok(json) => {
                // The wire bytes are UTF-8 by construction; writing them
                // directly (plus the newline `println!` would add) skips the
                // O(output) validation a `String` round trip would pay on
                // ~15×-source-sized JSON.
                use std::io::Write;
                let stdout = std::io::stdout();
                let mut out = stdout.lock();
                if out
                    .write_all(&json)
                    .and_then(|()| out.write_all(b"\n"))
                    .is_err()
                {
                    process::exit(1);
                }
            }
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
        None => Ok(tsv_ts::Goal::Module),
        Some(s) => tsv_ts::Goal::from_source_type(s)
            .ok_or_else(|| format!("invalid --goal '{s}' (expected 'script' or 'module')")),
    }
}

fn parse_to_json(
    source: &str,
    pretty: bool,
    parser_type: ParserType,
    goal: tsv_ts::Goal,
    locations: bool,
) -> Result<Vec<u8>, String> {
    // Compact output uses the convert_ast_json_bytes hot path (skips the
    // intermediate serde_json::Value and the output UTF-8 validation a String
    // would require); pretty-printing needs the Value for tab-indented
    // serialization.
    // The arena owns the internal AST; convert produces owned JSON, so nothing
    // borrowed escapes this function. Pre-sized to the source to avoid the
    // bump's chunk-doubling tail on the parse.
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));

    // Shared tail: `--pretty` reparses the compact wire bytes (the sole emission
    // path) into a `Value` for tab-indented serialization; compact returns the
    // bytes verbatim. A no-locations pretty print rides the same bytes rather
    // than a separate `Value` writer.
    let finish = |bytes: Vec<u8>| -> Result<Vec<u8>, String> {
        if pretty {
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|e| format!("JSON parse failed: {e}"))?;
            Ok(to_json_with_tabs(&value)
                .map_err(|e| format!("JSON serialization failed: {e}"))?
                .into_bytes())
        } else {
            Ok(bytes)
        }
    };

    // The goal applies only to TypeScript; svelte is always a module and css has
    // no goal.
    let bytes = match parser_type {
        ParserType::Svelte => {
            let ast = tsv_svelte::parse(source, &arena).map_err(|e| e.to_string())?;
            if locations {
                tsv_svelte::convert_ast_json_bytes(&ast, source)
            } else {
                tsv_svelte::convert_ast_json_bytes_no_locations(&ast, source)
            }
        }
        ParserType::Css => {
            let ast = tsv_css::parse(source, &arena).map_err(|e| e.to_string())?;
            if locations {
                tsv_css::convert_ast_json_bytes(&ast, source)
            } else {
                tsv_css::convert_ast_json_bytes_no_locations(&ast, source)
            }
        }
        ParserType::TypeScript => {
            let ast = tsv_ts::parse_with_goal(source, goal, &arena).map_err(|e| e.to_string())?;
            if locations {
                tsv_ts::convert_ast_json_bytes(&ast, source)
            } else {
                tsv_ts::convert_ast_json_bytes_no_locations(&ast, source)
            }
        }
    };
    finish(bytes)
}
