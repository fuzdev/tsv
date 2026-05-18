//! Embedded Deno sidecar for JS tool access
//!
//! Provides access to prettier, Svelte parser, acorn-typescript parser, and
//! Svelte's CSS parser (parseCss) via a lazily-spawned Deno process. The
//! process is only started when one of these functions is first called.
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
/// AST as a JSON Value (caller serializes with desired formatting)
///
/// # Errors
/// Returns an error if Deno is not available or parsing fails.
pub async fn parse_svelte(source: &str) -> Result<serde_json::Value, DenoError> {
    get_actor().await?.call("svelte-parse", source, None).await
}

/// Parse TypeScript source code using acorn with TypeScript plugin
///
/// # Arguments
/// * `source` - The TypeScript source code
///
/// # Returns
/// AST as a JSON Value (caller serializes with desired formatting)
///
/// # Errors
/// Returns an error if Deno is not available or parsing fails.
pub async fn parse_typescript(source: &str) -> Result<serde_json::Value, DenoError> {
    get_actor()
        .await?
        .call("acorn-typescript-parse", source, None)
        .await
}

/// Parse CSS source code using Svelte's `parseCss`
///
/// # Arguments
/// * `source` - The CSS source code
///
/// # Returns
/// AST as a JSON Value (caller serializes with desired formatting)
///
/// # Errors
/// Returns an error if Deno is not available or parsing fails.
pub async fn parse_css(source: &str) -> Result<serde_json::Value, DenoError> {
    get_actor().await?.call("css-parse", source, None).await
}

/// Version information from the Deno sidecar
#[derive(Debug, Clone)]
pub struct VersionInfo {
    /// Deno runtime version
    pub deno: String,
    /// TypeScript version (bundled with Deno)
    pub typescript: String,
    /// prettier version
    pub prettier: String,
    /// prettier-plugin-svelte version
    pub prettier_plugin_svelte: String,
    /// svelte compiler version
    pub svelte: String,
    /// acorn parser version
    pub acorn: String,
    /// @sveltejs/acorn-typescript version
    pub acorn_typescript: String,
}

/// Check that Deno sidecar is available and return version info
///
/// This spawns the sidecar if not already running, making it useful for
/// verifying the environment is correctly set up.
///
/// # Errors
/// Returns an error if Deno is not installed or the sidecar fails to start.
pub async fn check() -> Result<VersionInfo, DenoError> {
    let result = get_actor().await?.call("__version_info", "", None).await?;

    let info = result.as_object().ok_or(DenoError::MissingOutput)?;
    let deps = info
        .get("dependencies")
        .and_then(|d| d.as_object())
        .ok_or(DenoError::MissingOutput)?;

    let get_str = |obj: &serde_json::Map<String, serde_json::Value>, key: &str| {
        obj.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string()
    };

    Ok(VersionInfo {
        deno: get_str(info, "runtime"),
        typescript: get_str(info, "typescript"),
        prettier: get_str(deps, "prettier"),
        prettier_plugin_svelte: get_str(deps, "prettier-plugin-svelte"),
        svelte: get_str(deps, "svelte"),
        acorn: get_str(deps, "acorn"),
        acorn_typescript: get_str(deps, "@sveltejs/acorn-typescript"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Test all deno tools in a single test to avoid race conditions
    /// with the shared static actor across multiple tokio runtimes.
    #[tokio::test]
    async fn test_deno_tools() {
        // Test check (version info)
        let result = check().await;
        assert!(result.is_ok(), "check failed: {result:?}");
        let info = result.unwrap();
        assert!(!info.deno.is_empty());
        assert!(!info.prettier.is_empty());

        // Test prettier
        let result = run_prettier("<div>hello</div>", PrettierParser::Parser("svelte")).await;
        assert!(result.is_ok(), "prettier failed: {result:?}");
        assert_eq!(result.unwrap(), "<div>hello</div>\n");

        // Test svelte parser
        let result = parse_svelte("<div>hello</div>").await;
        assert!(result.is_ok(), "parse_svelte failed: {result:?}");
        let ast = result.unwrap();
        assert_eq!(ast.get("type").and_then(|v| v.as_str()), Some("Root"));

        // Test typescript parser
        let result = parse_typescript("const x: number = 1;").await;
        assert!(result.is_ok(), "parse_typescript failed: {result:?}");
        let ast = result.unwrap();
        assert_eq!(ast.get("type").and_then(|v| v.as_str()), Some("Program"));

        // Test CSS parser
        let result = parse_css(".a { color: red; }").await;
        assert!(result.is_ok(), "parse_css failed: {result:?}");
        let ast = result.unwrap();
        assert_eq!(
            ast.get("type").and_then(|v| v.as_str()),
            Some("StyleSheetFile")
        );
    }
}
