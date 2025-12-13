//! Embedded Deno sidecar for JS tool access
//!
//! Provides access to prettier, Svelte parser, and acorn-typescript parser
//! via a lazily-spawned Deno process. The process is only started when
//! one of these functions is first called.
//!
//! # Example
//!
//! ```ignore
//! use tsv_debug::deno;
//!
//! // Deno is spawned lazily on first call
//! let formatted = deno::run_prettier("<div>hi</div>", "svelte").await?;
//! let ast = deno::parse_svelte("<div>hi</div>").await?;
//! ```

mod actor;
mod error;
mod protocol;

pub use error::DenoError;

use actor::DenoActor;
use tokio::sync::OnceCell;

/// Global lazy-initialized Deno actor
static DENO_ACTOR: OnceCell<DenoActor> = OnceCell::const_new();

/// Get or spawn the Deno actor (lazy initialization)
async fn get_actor() -> Result<&'static DenoActor, DenoError> {
    DENO_ACTOR
        .get_or_try_init(|| async { DenoActor::spawn() })
        .await
}

/// Specifies how prettier should determine the parser
#[derive(Debug, Clone, Copy)]
pub enum PrettierParser<'a> {
    /// Explicit parser name (e.g., "svelte", "typescript", "css")
    Parser(&'a str),
    /// Infer parser from filepath extension (e.g., "foo.svelte", "bar.ts")
    Filepath(&'a str),
}

/// Run prettier on content
///
/// # Arguments
/// * `content` - The code to format
/// * `parser` - How to determine the parser (explicit name or infer from filepath)
///
/// # Errors
/// Returns an error if Deno is not available or formatting fails.
pub async fn run_prettier(content: &str, parser: PrettierParser<'_>) -> Result<String, DenoError> {
    let options = match parser {
        PrettierParser::Parser(p) => serde_json::json!({ "parser": p }),
        PrettierParser::Filepath(f) => serde_json::json!({ "filepath": f }),
    };

    let result = get_actor()
        .await?
        .call("prettier", content, Some(options))
        .await?;

    result
        .as_str()
        .map(ToString::to_string)
        .ok_or(DenoError::MissingOutput)
}

/// Parse Svelte source code using the official Svelte compiler
///
/// # Arguments
/// * `source` - The Svelte source code
///
/// # Returns
/// JSON AST as a string
///
/// # Errors
/// Returns an error if Deno is not available or parsing fails.
pub async fn parse_svelte(source: &str) -> Result<String, DenoError> {
    let result = get_actor()
        .await?
        .call("svelte-parse", source, None)
        .await?;

    result
        .as_str()
        .map(ToString::to_string)
        .ok_or(DenoError::MissingOutput)
}

/// Parse TypeScript source code using acorn with TypeScript plugin
///
/// # Arguments
/// * `source` - The TypeScript source code
///
/// # Returns
/// JSON AST as a string
///
/// # Errors
/// Returns an error if Deno is not available or parsing fails.
pub async fn parse_typescript(source: &str) -> Result<String, DenoError> {
    let result = get_actor()
        .await?
        .call("acorn-ts-parse", source, None)
        .await?;

    result
        .as_str()
        .map(ToString::to_string)
        .ok_or(DenoError::MissingOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test all deno tools in a single test to avoid race conditions
    /// with the shared static actor across multiple tokio runtimes.
    #[tokio::test]
    async fn test_deno_tools() {
        // Test prettier
        let result = run_prettier("<div>hello</div>", PrettierParser::Parser("svelte")).await;
        assert!(result.is_ok(), "prettier failed: {result:?}");
        assert_eq!(result.unwrap(), "<div>hello</div>\n");

        // Test svelte parser
        let result = parse_svelte("<div>hello</div>").await;
        assert!(result.is_ok(), "parse_svelte failed: {result:?}");
        let ast = result.unwrap();
        assert!(ast.contains("\"type\": \"Root\""));

        // Test typescript parser
        let result = parse_typescript("const x: number = 1;").await;
        assert!(result.is_ok(), "parse_typescript failed: {result:?}");
        let ast = result.unwrap();
        assert!(ast.contains("\"type\": \"Program\""));
    }
}
