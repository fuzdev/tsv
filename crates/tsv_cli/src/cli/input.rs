use std::fs;
use std::io::{self, Read as _};
use std::str::FromStr;

/// Input source for parsing or formatting
#[derive(Debug)]
pub enum Input {
    File(String),    // File path (original) + content
    Content(String), // Direct string content
    Stdin(String),   // Content read from stdin
}

impl Input {
    pub fn content(&self) -> &str {
        match self {
            Input::File(s) | Input::Content(s) | Input::Stdin(s) => s,
        }
    }

    /// Read from file path
    pub fn from_file(path: &str) -> Result<Self, String> {
        let source = fs::read_to_string(path)
            .map_err(|e| format!("Error reading file '{}': {}", path, e))?;
        Ok(Input::File(source))
    }

    /// Read from stdin
    pub fn from_stdin() -> Result<Self, String> {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|e| format!("Error reading from stdin: {}", e))?;
        Ok(Input::Stdin(buffer))
    }

    /// Direct string content
    pub fn from_content(content: String) -> Self {
        Input::Content(content)
    }
}

/// Parser/formatter type
#[derive(Clone, Copy, Debug)]
pub enum ParserType {
    Svelte,
    TypeScript,
    Css,
}

impl ParserType {
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
                "Unknown parser type: '{}'. Valid types: svelte, typescript, css",
                s
            )),
        }
    }
}
