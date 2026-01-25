//! N-API bindings for tsv
//!
//! Provides parse and format functions for Node.js and Bun via N-API.
//! Use `@napi-rs/cli` to build and publish to npm.
//!
//! ESM only - `CommonJS` is not supported.
//!
//! # Usage
//!
//! ```js
//! import { parseSvelte, formatSvelte } from 'tsv';
//!
//! const ast = parseSvelte('<div>Hello</div>');
//! const formatted = formatSvelte('<div>Hello</div>');
//! ```

use napi_derive::napi;

//
// Svelte
//

/// Parse Svelte source code and return JSON AST.
#[napi]
pub fn parse_svelte(source: String) -> napi::Result<String> {
    let ast = tsv_svelte::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let public_ast = tsv_svelte::convert_ast(&ast, &source);
    serde_json::to_string(&public_ast)
        .map_err(|e| napi::Error::from_reason(format!("JSON serialization error: {e}")))
}

/// Parse Svelte source code to internal AST only (no conversion, for benchmarking).
#[napi]
pub fn parse_internal_svelte(source: String) -> napi::Result<()> {
    let ast = tsv_svelte::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    // Prevent compiler from optimizing away the parse
    std::hint::black_box(ast);
    Ok(())
}

/// Format Svelte source code.
#[napi]
pub fn format_svelte(source: String) -> napi::Result<String> {
    let ast = tsv_svelte::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(tsv_svelte::format(&ast, &source))
}

//
// TypeScript
//

/// Parse TypeScript source code and return JSON AST.
#[napi]
pub fn parse_typescript(source: String) -> napi::Result<String> {
    let ast = tsv_ts::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let public_ast = tsv_ts::convert_ast(&ast, &source);
    serde_json::to_string(&public_ast)
        .map_err(|e| napi::Error::from_reason(format!("JSON serialization error: {e}")))
}

/// Parse TypeScript source code to internal AST only (no conversion, for benchmarking).
#[napi]
pub fn parse_internal_typescript(source: String) -> napi::Result<()> {
    let ast = tsv_ts::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    std::hint::black_box(ast);
    Ok(())
}

/// Format TypeScript source code.
#[napi]
pub fn format_typescript(source: String) -> napi::Result<String> {
    let ast = tsv_ts::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(tsv_ts::format(&ast, &source))
}

//
// CSS
//

/// Parse CSS source code and return JSON AST.
#[napi]
pub fn parse_css(source: String) -> napi::Result<String> {
    let ast = tsv_css::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let public_ast = tsv_css::convert_ast(&ast, &source);
    serde_json::to_string(&public_ast)
        .map_err(|e| napi::Error::from_reason(format!("JSON serialization error: {e}")))
}

/// Parse CSS source code to internal AST only (no conversion, for benchmarking).
#[napi]
pub fn parse_internal_css(source: String) -> napi::Result<()> {
    let ast = tsv_css::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    std::hint::black_box(ast);
    Ok(())
}

/// Format CSS source code.
#[napi]
pub fn format_css(source: String) -> napi::Result<String> {
    let ast = tsv_css::parse(&source).map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(tsv_css::format(&ast, &source))
}
