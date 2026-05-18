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

fn err(e: impl ToString) -> napi::Error {
    napi::Error::from_reason(e.to_string())
}

fn to_json(value: &serde_json::Value) -> napi::Result<String> {
    serde_json::to_string(value)
        .map_err(|e| napi::Error::from_reason(format!("JSON serialization error: {e}")))
}

/// Generate `parse_<lang>` / `parse_internal_<lang>` / `format_<lang>` N-API
/// functions for one language module.
macro_rules! lang_bindings {
    ($parse_fn:ident, $parse_internal_fn:ident, $format_fn:ident, $lang:ident) => {
        #[napi]
        pub fn $parse_fn(source: String) -> napi::Result<String> {
            let ast = $lang::parse(&source).map_err(err)?;
            to_json(&$lang::convert_ast_json(&ast, &source))
        }

        #[napi]
        pub fn $parse_internal_fn(source: String) -> napi::Result<()> {
            let ast = $lang::parse(&source).map_err(err)?;
            std::hint::black_box(ast);
            Ok(())
        }

        #[napi]
        pub fn $format_fn(source: String) -> napi::Result<String> {
            let ast = $lang::parse(&source).map_err(err)?;
            Ok($lang::format(&ast, &source))
        }
    };
}

lang_bindings!(
    parse_svelte,
    parse_internal_svelte,
    format_svelte,
    tsv_svelte
);
lang_bindings!(
    parse_typescript,
    parse_internal_typescript,
    format_typescript,
    tsv_ts
);
lang_bindings!(parse_css, parse_internal_css, format_css, tsv_css);
