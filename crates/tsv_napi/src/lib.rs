//! N-API bindings for tsv (Node.js / Bun).
//!
//! The Node-runtime sibling of [`tsv_ffi`](../tsv_ffi) (the Deno/C-FFI path) and
//! [`tsv_wasm`](../tsv_wasm) (the universal WASM path). napi-rs marshals the
//! JS string into a Rust `String` and the returned `String` back out, so there
//! are no raw pointers and no manual free — the cleanest of the three bindings.
//!
//! Transport mirrors `tsv_wasm`'s deliberate choice: `parse_<lang>` returns a
//! JSON **string** for the host to `JSON.parse`, rather than building the object
//! graph node-by-node across the boundary (measurably slower). `format_<lang>`
//! returns the formatted source directly. Engine errors surface as thrown JS
//! errors (`napi::Error`); `parse_internal_<lang>` parses without converting
//! (benchmark-only, AST kept live via `black_box`).
//!
//! Built as a `cdylib` and loaded by Node as a `.node` addon. The `format` /
//! `parse` cargo features gate which entry points are emitted (mirrors
//! `tsv_ffi` / `tsv_wasm`).

use napi_derive::napi;
// Per-thread reusable arenas live in the shared `tsv_arena` crate (used by both
// native bindings — see its module docs for the reuse rationale + soundness).
use tsv_arena::with_ast_arena;
#[cfg(feature = "format")]
use tsv_arena::with_doc_arena;

/// Generate `parse_<lang>` / `parse_internal_<lang>` / `format_<lang>` N-API
/// functions for one language module. The `js_name` literals keep the JS export
/// names snake_case for parity with `tsv_wasm` (napi-rs would otherwise
/// camelCase them).
macro_rules! lang_bindings {
    (
        $lang:ident,
        $parse_fn:ident, $parse_js:literal,
        $parse_internal_fn:ident, $parse_internal_js:literal,
        $format_fn:ident, $format_js:literal
    ) => {
        /// Parse source code and return its public JSON AST as a string.
        #[cfg(feature = "parse")]
        #[napi(js_name = $parse_js)]
        pub fn $parse_fn(source: String) -> napi::Result<String> {
            with_ast_arena(|arena| {
                let ast = $lang::parse(&source, arena)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
                Ok($lang::convert_ast_json_string(&ast, &source))
            })
        }

        /// Parse source to the internal AST only (no conversion, no
        /// serialization). Benchmark-only: `black_box` keeps the AST live so the
        /// parse can't be optimized away.
        #[cfg(feature = "parse")]
        #[napi(js_name = $parse_internal_js)]
        pub fn $parse_internal_fn(source: String) -> napi::Result<()> {
            with_ast_arena(|arena| {
                let ast = $lang::parse(&source, arena)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
                std::hint::black_box(&ast);
                Ok(())
            })
        }

        /// Format source code and return the formatted string.
        #[cfg(feature = "format")]
        #[napi(js_name = $format_js)]
        pub fn $format_fn(source: String) -> napi::Result<String> {
            with_ast_arena(|arena| {
                let ast = $lang::parse(&source, arena)
                    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
                Ok(with_doc_arena(|doc_arena| {
                    $lang::format_in(&ast, &source, doc_arena)
                }))
            })
        }
    };
}

lang_bindings!(
    tsv_svelte,
    parse_svelte,
    "parse_svelte",
    parse_internal_svelte,
    "parse_internal_svelte",
    format_svelte,
    "format_svelte"
);
lang_bindings!(
    tsv_ts,
    parse_typescript,
    "parse_typescript",
    parse_internal_typescript,
    "parse_internal_typescript",
    format_typescript,
    "format_typescript"
);
lang_bindings!(
    tsv_css,
    parse_css,
    "parse_css",
    parse_internal_css,
    "parse_internal_css",
    format_css,
    "format_css"
);

// Drive every entry point in-process so `cargo test` exercises the native
// binding without a Node host (the Deno/WASM smoke paths don't cover napi).
// These call the plain Rust functions the `#[napi]` macro wraps; the JS
// marshalling layer is what `scripts/test_napi.ts` covers under Node.
#[cfg(test)]
mod tests {
    use super::*;

    /// Signature shared by every `parse_<lang>` / `format_<lang>` entry point.
    type StringFn = fn(String) -> napi::Result<String>;
    /// Signature shared by every `parse_internal_<lang>` entry point.
    type UnitFn = fn(String) -> napi::Result<()>;

    // --- format: normalizes, every language (exact output) ---

    #[test]
    fn format_normalizes_per_language() {
        // Annotate the array type so the fn items coerce to `StringFn` (no casts).
        let cases: [(&str, StringFn, &str, &str); 3] = [
            (
                "typescript",
                format_typescript,
                "const   x=1",
                "const x = 1;\n",
            ),
            ("css", format_css, "a{color:red}", "a {\n\tcolor: red;\n}\n"),
            (
                "svelte",
                format_svelte,
                "<div   >x</div   >",
                "<div>x</div>\n",
            ),
        ];
        for (label, f, input, expected) in cases {
            assert_eq!(f(input.to_owned()).unwrap(), expected, "{label} format");
        }
    }

    // --- parse: returns the language's own JSON root type ---

    #[test]
    fn parse_returns_language_root_type() {
        // The root `type` is distinct per language, so asserting it also guards
        // the `lang_bindings!` wiring: a transposed invocation (e.g. `parse_css`
        // pointed at `tsv_ts`) would return the wrong root, not just "some JSON."
        let cases: [(&str, StringFn, &str, &str); 3] = [
            ("typescript", parse_typescript, "const x = 1;", "Program"),
            ("css", parse_css, "a { color: red }", "StyleSheetFile"),
            ("svelte", parse_svelte, "<div>x</div>", "Root"),
        ];
        for (label, f, src, root_type) in cases {
            let json = f(src.to_owned()).unwrap();
            let value: serde_json::Value =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{label}: not JSON: {e}"));
            assert_eq!(
                value.get("type").and_then(serde_json::Value::as_str),
                Some(root_type),
                "{label}: unexpected root type in {json}"
            );
        }
    }

    // --- parse_internal: parses without converting (Ok, no JSON), every language ---

    #[test]
    fn parse_internal_ok_per_language() {
        let cases: [(&str, UnitFn, &str); 3] = [
            ("typescript", parse_internal_typescript, "const x = 1;"),
            ("css", parse_internal_css, "a { color: red }"),
            ("svelte", parse_internal_svelte, "<div>x</div>"),
        ];
        for (label, f, src) in cases {
            f(src.to_owned()).unwrap_or_else(|e| panic!("{label}: {}", e.reason));
        }
    }

    // --- errors surface as a thrown napi::Error carrying the engine message ---

    #[test]
    fn invalid_syntax_errors_per_language() {
        // Both parse and format wrap the engine error into a napi::Error (which
        // napi-rs throws — there is no `{"error": …}` envelope, unlike FFI).
        // Cover the error arm for every language across both entry-point kinds.
        let cases: [(&str, StringFn, StringFn, &str); 3] = [
            (
                "typescript",
                parse_typescript,
                format_typescript,
                "const = ;",
            ),
            ("css", parse_css, format_css, "a {"),
            ("svelte", parse_svelte, format_svelte, "<div {"),
        ];
        for (label, parse_fn, format_fn, src) in cases {
            let parse_err = parse_fn(src.to_owned()).unwrap_err();
            assert!(
                !parse_err.reason.is_empty(),
                "{label} parse: error must carry a reason"
            );
            let format_err = format_fn(src.to_owned()).unwrap_err();
            assert!(
                !format_err.reason.is_empty(),
                "{label} format: error must carry a reason"
            );
        }
    }

    // --- the per-thread arenas are reset+reused across calls (warm-path soundness) ---

    #[test]
    fn repeated_calls_reuse_arenas() {
        // This crate's distinctive risk: `with_ast_arena` / `with_doc_arena`
        // keep one arena per thread and `reset()` it at the start of each call,
        // so nothing built in a prior call may leak past the next reset. Two
        // back-to-back formats on a warm arena must produce identical output,
        // and interleaving a parse (which drives the AST arena on its own)
        // between them must not perturb the format result.
        let once = format_typescript("const   x=1".to_owned()).unwrap();
        let twice = format_typescript("const   x=1".to_owned()).unwrap();
        assert_eq!(once, twice, "second format on a warm arena diverged");
        parse_typescript("const y = 2;".to_owned()).unwrap();
        let after_parse = format_typescript("const   x=1".to_owned()).unwrap();
        assert_eq!(once, after_parse, "interleaved parse perturbed format");
    }

    // --- multibyte source survives the JS-string marshalling + char-offset boundary ---

    #[test]
    fn parse_and_format_preserve_multibyte_source() {
        // napi-rs marshals JS strings in/out and the AST carries char offsets;
        // this is the same boundary risk tsv_ffi's same-named test guards.
        let src = "const x = '€🦀';\n";
        let json = parse_typescript(src.to_owned()).unwrap();
        assert!(json.contains("\"type\""), "parse produced no AST: {json}");
        let formatted = format_typescript(src.to_owned()).unwrap();
        assert!(
            formatted.contains("€🦀"),
            "multibyte content lost: {formatted}"
        );
        // Re-formatting is stable (idempotent) across the boundary.
        assert_eq!(
            format_typescript(formatted.clone()).unwrap(),
            formatted,
            "re-format not idempotent across the boundary"
        );
    }
}
