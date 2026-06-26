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

/// Run `f` with a per-thread reusable AST arena.
///
/// Duplicates `tsv_ffi`'s `with_ast_arena` (see its rationale): the bindings are
/// called once per file in tight loops, so a fresh `Bump` per call churns the
/// system allocator's heap high-water in a way that is measurable through the
/// binding boundary. Each thread keeps one `Bump` and `reset()`s it between
/// calls — `reset()` rewinds the bump pointer and retains the largest chunk, so
/// a warm thread does no per-call malloc/free. The per-file AST is fully consumed
/// inside `f` (the returned value owns its bytes — a formatted `String`, a JSON
/// `String`, or `()`), so no AST outlives the next call's `reset()`.
///
/// Kept in lockstep with `tsv_ffi::with_ast_arena` by hand for now; factoring
/// both onto one shared helper is a follow-up (lore TODO_NAPI_BINDINGS §arena
/// reuse / TODO_BUMPALO_ARENA item 1).
fn with_ast_arena<R>(f: impl FnOnce(&bumpalo::Bump) -> R) -> R {
    thread_local! {
        static AST_ARENA: std::cell::RefCell<bumpalo::Bump> =
            std::cell::RefCell::new(bumpalo::Bump::new());
    }
    AST_ARENA.with(|cell| {
        let mut arena = cell.borrow_mut();
        arena.reset();
        f(&arena)
    })
}

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
                Ok($lang::format(&ast, &source))
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn error_message(err: &napi::Error) -> String {
        err.reason.clone()
    }

    // --- format: happy path (one per language exercises the macro expansion) ---

    #[test]
    fn format_typescript_normalizes() {
        assert_eq!(
            format_typescript("const   x=1".to_owned()).unwrap(),
            "const x = 1;\n"
        );
    }

    #[test]
    fn format_css_normalizes() {
        assert_eq!(
            format_css("a{color:red}".to_owned()).unwrap(),
            "a {\n\tcolor: red;\n}\n"
        );
    }

    #[test]
    fn format_svelte_normalizes() {
        assert_eq!(
            format_svelte("<div   >x</div   >".to_owned()).unwrap(),
            "<div>x</div>\n"
        );
    }

    // --- parse: returns JSON AST; internal returns unit ---

    #[test]
    fn parse_typescript_returns_json() {
        let json = parse_typescript("const x = 1;".to_owned()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value.get("type").and_then(serde_json::Value::as_str),
            Some("Program")
        );
    }

    #[test]
    fn parse_internal_css_ok() {
        parse_internal_css("a { color: red }".to_owned()).unwrap();
    }

    // --- errors surface as napi::Error with the engine message ---

    #[test]
    fn format_typescript_invalid_errors() {
        let err = format_typescript("const = ;".to_owned()).unwrap_err();
        assert!(!error_message(&err).is_empty(), "error must carry a reason");
    }
}
