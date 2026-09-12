use std::fs;
use std::io::{self, Read as _};
use std::str::FromStr;

/// Input source for parsing or formatting (just the content string)
#[derive(Debug)]
pub struct Input(String);

impl Input {
    pub fn content(&self) -> &str {
        &self.0
    }

    /// Read from file path
    pub fn from_file(path: &str) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Error reading file '{path}': {e}"))?;
        Ok(Input(content))
    }

    /// Read from stdin
    pub fn from_stdin() -> Result<Self, String> {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|e| format!("Error reading from stdin: {e}"))?;
        Ok(Input(buffer))
    }

    /// Direct string content
    pub fn from_content(content: String) -> Self {
        Input(content)
    }
}

/// Parser/formatter type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserType {
    Svelte,
    TypeScript,
    Css,
}

impl ParserType {
    /// Canonical lowercase name — the `--parser` value and the
    /// `tsv_debug` sidecar tool key.
    pub const fn name(self) -> &'static str {
        match self {
            ParserType::Svelte => "svelte",
            ParserType::TypeScript => "typescript",
            ParserType::Css => "css",
        }
    }

    pub fn from_extension(path: &str) -> Self {
        if path.ends_with(".svelte") {
            ParserType::Svelte
        } else if path.ends_with(".css") {
            ParserType::Css
        } else {
            ParserType::TypeScript
        }
    }
}

impl FromStr for ParserType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "svelte" => Ok(ParserType::Svelte),
            "typescript" | "ts" => Ok(ParserType::TypeScript),
            "css" => Ok(ParserType::Css),
            _ => Err(format!(
                "Unknown parser type: '{s}'. Valid types: svelte, typescript, css"
            )),
        }
    }
}

/// Shared input arguments for commands that accept a file path, `--content`, or `--stdin`.
///
/// Each command declares the four argh fields on its own struct and assembles an
/// `InputArgs` to call [`InputArgs::resolve`]. argh has no struct-flattening
/// attribute, so the field declarations are repeated per command.
#[derive(Debug)]
pub struct InputArgs {
    pub content: Option<String>,
    pub stdin: bool,
    pub parser: Option<ParserType>,
    pub file: Option<String>,
}

impl InputArgs {
    /// Resolve to an `Input` + `ParserType`.
    ///
    /// Precedence: `--content` > `--stdin` > file positional. `--content` and
    /// `--stdin` require `--parser`. On a file path, `--parser` overrides the
    /// extension-based detection when present; otherwise it's inferred from the
    /// extension — which must then be one tsv handles. The dispatch behind a path has
    /// no unknown arm (everything that isn't `.svelte` or `.css` is TypeScript), so
    /// without the check `tsv parse README.md` reports a baffling TypeScript syntax
    /// error rather than saying the file is out of scope. Same check, same message, as
    /// `tsv format <file>` (`tsv_discover::unsupported_extension_error`), where it is an
    /// argument error for the same reason.
    pub fn resolve(self) -> Result<(Input, ParserType), String> {
        if let Some(content) = self.content {
            let parser_type = self
                .parser
                .ok_or("--content requires --parser <svelte|typescript|css>")?;
            Ok((Input::from_content(content), parser_type))
        } else if self.stdin {
            let parser_type = self
                .parser
                .ok_or("--stdin requires --parser <svelte|typescript|css>")?;
            Ok((Input::from_stdin()?, parser_type))
        } else if let Some(path) = self.file {
            let parser_type = match self.parser {
                Some(parser_type) => parser_type,
                None => {
                    if let Some(error) = tsv_discover::unsupported_extension_error(&path) {
                        return Err(error);
                    }
                    ParserType::from_extension(&path)
                }
            };
            Ok((Input::from_file(&path)?, parser_type))
        } else {
            Err("No input provided. Use a file path, --content, or --stdin".to_string())
        }
    }
}

/// Parse the `--source-type` argument into a [`tsv_ts::Goal`]. Absent stays
/// **absent** — `module`/`script` map to the goals, and what an unnamed source
/// type means is the caller's to decide: `parse` reads it as `Module` (the wire's
/// `Program.sourceType` is a claim, so one grammar produces it), `format` as the
/// module-then-script fallback. Shared by both.
pub fn parse_source_type_arg(source_type: Option<&str>) -> Result<Option<tsv_ts::Goal>, String> {
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
pub fn check_source_type_language(
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
