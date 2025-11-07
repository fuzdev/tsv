use std::fs;
use std::io::{self, Read as _};
use std::str::FromStr;

/// Input source for parsing or formatting
#[derive(Debug)]
pub enum Input {
    File { path: String, content: String }, // File path + content
    Content(String),                        // Direct string content
    Stdin(String),                          // Content read from stdin
}

impl Input {
    pub fn content(&self) -> &str {
        match self {
            Input::File { content, .. } => content,
            Input::Content(s) | Input::Stdin(s) => s,
        }
    }

    /// Get parser type from input source
    pub fn parser_type(&self) -> Option<ParserType> {
        match self {
            Input::File { path, .. } => Some(ParserType::from_extension(path)),
            Input::Content(_) | Input::Stdin(_) => None,
        }
    }

    /// Read from file path
    pub fn from_file(path: &str) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Error reading file '{}': {}", path, e))?;
        Ok(Input::File {
            path: path.to_string(),
            content,
        })
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
