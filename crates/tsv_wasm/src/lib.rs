//! WebAssembly bindings for tsv.
//!
//! Three builds from two features:
//! - default (`@fuzdev/tsv_wasm`): everything — `format_*` plus the parse exports.
//! - `--no-default-features --features format` (`@fuzdev/tsv_format_wasm`):
//!   `format_*` exports only.
//! - `--no-default-features --features parse` (`@fuzdev/tsv_parse_wasm`):
//!   `parse_*`, `parse_*_json`, and `parse_internal_*` plus the convert layer
//!   that serializes ASTs to JS; the printers drop out at link time.
//!
//! The AST crosses the JS boundary as a single JSON string: `parse_*` calls
//! the engine's native `JSON.parse` on it (via `js_sys`) and returns the
//! typed object; `parse_*_json` returns the string itself for consumers that
//! forward the wire format without materializing. Building the JS object
//! graph node-by-node with `serde_wasm_bindgen` is measurably slower.

use wasm_bindgen::prelude::*;

fn err(e: impl ToString) -> JsError {
    JsError::new(&e.to_string())
}

/// Re-export every type from the bundled `./tsv_ast` declaration file
/// so consumers of `@fuzdev/tsv_parse_wasm` can `import type { Program } from
/// '@fuzdev/tsv_parse_wasm'` without reaching into the bundled `.d.ts`.
#[cfg(feature = "parse")]
#[wasm_bindgen(typescript_custom_section)]
const TS_AST_REEXPORT: &'static str = r#"
export type * from "./tsv_ast";
"#;

/// Typed return types for `parse_*` exports. Each extern type points at
/// the matching interface in the bundled `tsv_ast.d.ts`, so the
/// wasm-pack-generated `tsv_wasm.d.ts` declares `parse_typescript` as
/// returning `Program`, etc.
#[cfg(feature = "parse")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "import('./tsv_ast').Program")]
    pub type TsProgram;

    #[wasm_bindgen(typescript_type = "import('./tsv_ast').StyleSheetFile")]
    pub type CssStyleSheet;

    #[wasm_bindgen(typescript_type = "import('./tsv_ast').Root")]
    pub type SvelteRoot;
}

/// Generate `parse_<lang>` / `parse_<lang>_json` / `parse_internal_<lang>` /
/// `format_<lang>` WASM functions for one language module. `parse_*`,
/// `parse_*_json`, and `parse_internal_*` are gated on `parse` (so the
/// format-only build excludes the convert layer) and `format_*` on `format`
/// (so the parse-only build drops the printers at link time). `$parse_ret`
/// is the extern type from the block above whose `typescript_type` attribute
/// names the matching interface in `tsv_ast.d.ts`.
macro_rules! lang_bindings {
    (
        $parse_fn:ident,
        $parse_json_fn:ident,
        $parse_internal_fn:ident,
        $format_fn:ident,
        $lang:ident,
        $parse_ret:ident $(,)?
    ) => {
        /// Parse source into the typed JSON AST.
        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_fn(source: &str) -> Result<$parse_ret, JsError> {
            let json = $parse_json_fn(source)?;
            let js_value = js_sys::JSON::parse(&json)
                .map_err(|_| err("internal error: AST serialized to invalid JSON"))?;
            Ok(js_value.unchecked_into::<$parse_ret>())
        }

        /// Parse source into the JSON AST as a compact JSON string, skipping
        /// JS object materialization (for consumers forwarding the wire format).
        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_json_fn(source: &str) -> Result<String, JsError> {
            let ast = $lang::parse(source).map_err(err)?;
            Ok($lang::convert_ast_json_string(&ast, source))
        }

        #[cfg(feature = "parse")]
        #[wasm_bindgen]
        pub fn $parse_internal_fn(source: &str) -> Result<(), JsError> {
            let ast = $lang::parse(source).map_err(err)?;
            std::hint::black_box(ast);
            Ok(())
        }

        #[cfg(feature = "format")]
        #[wasm_bindgen]
        pub fn $format_fn(source: &str) -> Result<String, JsError> {
            let ast = $lang::parse(source).map_err(err)?;
            Ok($lang::format(&ast, source))
        }
    };
}

lang_bindings!(
    parse_svelte,
    parse_svelte_json,
    parse_internal_svelte,
    format_svelte,
    tsv_svelte,
    SvelteRoot,
);
lang_bindings!(
    parse_typescript,
    parse_typescript_json,
    parse_internal_typescript,
    format_typescript,
    tsv_ts,
    TsProgram,
);
lang_bindings!(
    parse_css,
    parse_css_json,
    parse_internal_css,
    format_css,
    tsv_css,
    CssStyleSheet,
);
