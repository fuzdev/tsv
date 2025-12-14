//! WebAssembly bindings for tsv
//!
//! Provides parse and format functions for Svelte, TypeScript, and CSS
//! that can be called from JavaScript/TypeScript via WebAssembly.

use wasm_bindgen::prelude::*;

// ============================================================================
// Svelte
// ============================================================================

/// Parse Svelte source code and return the AST as a JavaScript object.
///
/// Returns a JSON-compatible AST matching Svelte's official parser output.
#[wasm_bindgen]
pub fn parse_svelte(source: &str) -> Result<JsValue, JsError> {
    let ast = tsv_svelte::parse(source).map_err(|e| JsError::new(&e.to_string()))?;
    let public = tsv_svelte::convert_ast(&ast, source);
    serde_wasm_bindgen::to_value(&public).map_err(|e| JsError::new(&e.to_string()))
}

/// Format Svelte source code.
///
/// Parses the source and returns formatted output matching Prettier's style.
#[wasm_bindgen]
pub fn format_svelte(source: &str) -> Result<String, JsError> {
    let ast = tsv_svelte::parse(source).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(tsv_svelte::format(&ast, source))
}

// ============================================================================
// TypeScript
// ============================================================================

/// Parse TypeScript source code and return the AST as a JavaScript object.
///
/// Returns a JSON-compatible AST matching acorn + acorn-typescript output.
#[wasm_bindgen]
pub fn parse_typescript(source: &str) -> Result<JsValue, JsError> {
    let ast = tsv_ts::parse(source).map_err(|e| JsError::new(&e.to_string()))?;
    let public = tsv_ts::convert_ast(&ast, source);
    serde_wasm_bindgen::to_value(&public).map_err(|e| JsError::new(&e.to_string()))
}

/// Format TypeScript source code.
///
/// Parses the source and returns formatted output matching Prettier's style.
#[wasm_bindgen]
pub fn format_typescript(source: &str) -> Result<String, JsError> {
    let ast = tsv_ts::parse(source).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(tsv_ts::format(&ast, source))
}

// ============================================================================
// CSS
// ============================================================================

/// Parse CSS source code and return the AST as a JavaScript object.
#[wasm_bindgen]
pub fn parse_css(source: &str) -> Result<JsValue, JsError> {
    let ast = tsv_css::parse(source).map_err(|e| JsError::new(&e.to_string()))?;
    let public = tsv_css::convert_ast(&ast, source);
    serde_wasm_bindgen::to_value(&public).map_err(|e| JsError::new(&e.to_string()))
}

/// Format CSS source code.
///
/// Parses the source and returns formatted output matching Prettier's style.
#[wasm_bindgen]
pub fn format_css(source: &str) -> Result<String, JsError> {
    let ast = tsv_css::parse(source).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(tsv_css::format(&ast, source))
}
