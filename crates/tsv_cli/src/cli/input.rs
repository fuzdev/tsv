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
    /// extension.
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
            let parser_type = self
                .parser
                .unwrap_or_else(|| ParserType::from_extension(&path));
            Ok((Input::from_file(&path)?, parser_type))
        } else {
            Err("No input provided. Use a file path, --content, or --stdin".to_string())
        }
    }
}
