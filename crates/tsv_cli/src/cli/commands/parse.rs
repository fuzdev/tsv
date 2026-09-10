use crate::cli::input::{InputArgs, ParserType};
use crate::cli::out::write_stdout;
use crate::err_line;
use crate::json_utils::indent_json_with_tabs;
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
    /// parses a standalone script — `await` is an ordinary identifier and
    /// `import`/`export`/`import.meta` are errors; the script is sloppy unless a
    /// `"use strict"` directive prologue says otherwise. TypeScript only — an error
    /// with svelte/css.
    #[argh(option)]
    source_type: Option<String>,

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
        // An unnamed source type is `Module` here: the wire carries
        // `Program.sourceType`, a claim about which grammar produced the AST, so
        // one settled goal has to produce it (`format`, which exposes neither the
        // AST nor the source type, reads the same absence as its fallback).
        let goal = match parse_source_type_arg(self.source_type.as_deref()) {
            Ok(g) => g.unwrap_or(tsv_ts::Goal::Module),
            Err(e) => {
                err_line!("Error: {e}");
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
                err_line!("Error: {e}");
                process::exit(1);
            }
        };
        if let Err(e) = check_source_type_language(self.source_type.as_deref(), parser_type) {
            err_line!("Error: {e}");
            process::exit(1);
        }

        match parse_to_json(
            input.content(),
            self.pretty,
            parser_type,
            goal,
            !self.no_locations,
        ) {
            Ok(json) => {
                // The wire bytes are UTF-8 by construction; writing them directly skips
                // the O(output) validation a `String` round trip would pay on
                // ~15×-source-sized JSON. The newline is a second write rather than a
                // `push` onto the same buffer: the writer sizes its `Vec` from an
                // estimate (`estimated_json_capacity`), so a wire that lands exactly on
                // its capacity would pay a realloc and a full copy of the output for
                // one byte — and two writes cost the same syscalls either way.
                //
                // A consumer that closed the pipe stops the write without failing the
                // run — the rule `write_stdout` holds for both commands. This used to
                // `exit(1)` there, reporting a closed reader as the parse error it
                // wasn't.
                write_stdout(&json);
                write_stdout(b"\n");
            }
            Err(e) => {
                err_line!("Parse error: {e}");
                process::exit(1);
            }
        }
    }
}

/// Parse the `--source-type` argument into a [`tsv_ts::Goal`]. Absent stays
/// **absent** — `module`/`script` map to the goals, and what an unnamed source
/// type means is the caller's to decide: `parse` reads it as `Module` (the wire's
/// `Program.sourceType` is a claim, so one grammar produces it), `format` as the
/// module-then-script fallback. Shared by both.
pub(crate) fn parse_source_type_arg(
    source_type: Option<&str>,
) -> Result<Option<tsv_ts::Goal>, String> {
    match source_type {
        None => Ok(None),
        Some(s) => tsv_ts::Goal::from_source_type(s)
            .map(Some)
            .ok_or_else(|| format!("invalid --source-type '{s}' (expected 'script' or 'module')")),
    }
}

/// Refuse a `--source-type` on a language that has no goal axis. Svelte hard-wires
/// `Module` and CSS has no goal, so a caller naming one there asked for something
/// that cannot be honored and must be told — the stance every binding takes
/// (`tsv_wasm`'s `read_options`, `tsv_ffi`'s `ffi_source_type`, `tsv_napi`), so the
/// CLI is not the one surface where the flag is silently dropped — the JS mirror
/// (`crates/tsv_wasm/npm/cli.js`, the bin of both npm CLIs) carries the same
/// refusal word for word, so the two shipped `tsv` bins cannot drift. Shared by
/// `parse` and `format`; the flag's *value* is validated ahead of this by
/// `parse_source_type_arg`, so this only asks whether it was named at all.
pub(crate) fn check_source_type_language(
    source_type: Option<&str>,
    parser_type: ParserType,
) -> Result<(), String> {
    if source_type.is_some() && parser_type != ParserType::TypeScript {
        return Err(format!(
            "--source-type is only supported for typescript (the {} parser has no source type)",
            parser_type.name()
        ));
    }
    Ok(())
}

fn parse_to_json(
    source: &str,
    pretty: bool,
    parser_type: ParserType,
    goal: tsv_ts::Goal,
    locations: bool,
) -> Result<Vec<u8>, String> {
    // Both outputs ride the convert_ast_json_bytes hot path (no intermediate
    // tree, and no output UTF-8 validation a String would require): compact
    // returns the bytes verbatim, and `--pretty` re-indents them in one linear
    // pass (`indent_json_with_tabs`) rather than reading them back into a
    // `serde_json::Value` — a read that recursed per JSON level and, at
    // serde_json's default recursion limit, refused past ~60 nested arrays
    // what the compact form of the same input emitted fine. So the pretty
    // route has no depth ceiling of its own; it stops where the parser stops.
    // The arena owns the internal AST; convert produces owned JSON, so nothing
    // borrowed escapes this function. Pre-sized to the source to avoid the
    // bump's chunk-doubling tail on the parse.
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));

    // Shared tail; a no-locations pretty print rides the same bytes.
    let finish = |bytes: Vec<u8>| -> Result<Vec<u8>, String> {
        if pretty {
            Ok(indent_json_with_tabs(&bytes))
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
