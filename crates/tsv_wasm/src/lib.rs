//! WebAssembly bindings for tsv.
//!
//! Default build (`@fuzdev/tsv_fmt`): `format_*` exports only.
//! `--features ast` (`@fuzdev/tsv_parse`): adds `parse_*` and
//! `parse_internal_*` plus the convert layer that serializes ASTs to JS.

use wasm_bindgen::prelude::*;

fn err(e: impl ToString) -> JsError {
    JsError::new(&e.to_string())
}

/// Re-export every type from the bundled `./tsv_ast` declaration file
/// so consumers of `@fuzdev/tsv_parse` can `import type { Program } from
/// '@fuzdev/tsv_parse'` without reaching into the bundled `.d.ts`.
#[cfg(feature = "ast")]
#[wasm_bindgen(typescript_custom_section)]
const TS_AST_REEXPORT: &'static str = r#"
export type * from "./tsv_ast";
"#;

/// Typed return types for `parse_*` exports. Each extern type points at
/// the matching interface in the bundled `tsv_ast.d.ts`, so the
/// wasm-pack-generated `tsv_wasm.d.ts` declares `parse_typescript` as
/// returning `Program`, etc.
#[cfg(feature = "ast")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "import('./tsv_ast').Program")]
    pub type TsProgram;

    #[wasm_bindgen(typescript_type = "import('./tsv_ast').StyleSheetFile")]
    pub type CssStyleSheet;

    #[wasm_bindgen(typescript_type = "import('./tsv_ast').Root")]
    pub type SvelteRoot;
}

/// Generate `parse_<lang>` / `parse_internal_<lang>` / `format_<lang>` WASM
/// functions for one language module. `parse_*` and `parse_internal_*` are
/// gated on `ast` so the format-only build excludes the convert layer.
/// `$parse_ret` is the extern type from the block above whose
/// `typescript_type` attribute names the matching interface in
/// `tsv_ast.d.ts`.
macro_rules! lang_bindings {
    (
        $parse_fn:ident,
        $parse_internal_fn:ident,
        $format_fn:ident,
        $lang:ident,
        $parse_ret:ident $(,)?
    ) => {
        #[cfg(feature = "ast")]
        #[wasm_bindgen]
        pub fn $parse_fn(source: &str) -> Result<$parse_ret, JsError> {
            let ast = $lang::parse(source).map_err(err)?;
            let json_value = $lang::convert_ast_json(&ast, source);
            let js_value = serde_wasm_bindgen::to_value(&json_value).map_err(err)?;
            Ok(js_value.unchecked_into::<$parse_ret>())
        }

        #[cfg(feature = "ast")]
        #[wasm_bindgen]
        pub fn $parse_internal_fn(source: &str) -> Result<(), JsError> {
            let ast = $lang::parse(source).map_err(err)?;
            std::hint::black_box(ast);
            Ok(())
        }

        #[wasm_bindgen]
        pub fn $format_fn(source: &str) -> Result<String, JsError> {
            let ast = $lang::parse(source).map_err(err)?;
            Ok($lang::format(&ast, source))
        }
    };
}

lang_bindings!(
    parse_svelte,
    parse_internal_svelte,
    format_svelte,
    tsv_svelte,
    SvelteRoot,
);
lang_bindings!(
    parse_typescript,
    parse_internal_typescript,
    format_typescript,
    tsv_ts,
    TsProgram,
);
lang_bindings!(
    parse_css,
    parse_internal_css,
    format_css,
    tsv_css,
    CssStyleSheet,
);
