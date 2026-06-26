//! C FFI bindings for tsv
//!
//! Provides parse and format functions with C ABI for use from any language
//! with C FFI support (Deno, Node.js via koffi/ffi-napi, Python ctypes, etc.).
//!
//! # Memory Management
//!
//! All functions that return `*mut u8` allocate memory that the caller must free
//! by calling `tsv_free(ptr, len)` with the returned pointer and length.
//!
//! # Safety
//!
//! These functions use raw pointers for FFI compatibility. The caller must ensure:
//! - `source_ptr` points to valid UTF-8 data of `source_len` bytes
//! - `out_len` points to a valid `usize` location for writing the output length
//! - The returned pointer is freed exactly once via `tsv_free`

#![allow(unsafe_code)]

use std::panic;
use std::slice;

/// Extract a &str from source pointer, or return an error result.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes.
unsafe fn extract_source<'a>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> Result<&'a str, *mut u8> {
    let bytes = unsafe { slice::from_raw_parts(source_ptr, source_len) };
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(e) => Err(error_result(&format!("Invalid UTF-8: {e}"), out_len)),
    }
}

/// Format a panic payload into a string for error reporting.
fn format_panic(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        format!("panic: {s}")
    } else if let Some(s) = payload.downcast_ref::<String>() {
        format!("panic: {s}")
    } else {
        "panic: <unknown>".to_string()
    }
}

/// Helper to convert source pointer to &str and run a closure returning the
/// output string verbatim (formatted source, or already-serialized JSON).
/// Catches panics (when built with `panic = "unwind"`) and returns them as error JSON.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes.
unsafe fn with_source_string<F>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
    f: F,
) -> *mut u8
where
    F: FnOnce(&str) -> Result<String, String> + panic::UnwindSafe,
{
    let source = match unsafe { extract_source(source_ptr, source_len, out_len) } {
        Ok(s) => s,
        Err(ptr) => return ptr,
    };

    match panic::catch_unwind(|| f(source)) {
        Ok(Ok(result)) => string_to_ptr(result, out_len),
        Ok(Err(e)) => error_result(&e, out_len),
        Err(payload) => error_result(&format_panic(&*payload), out_len),
    }
}

/// Helper for internal parse (no conversion, no JSON serialization).
/// Returns empty string on success, error JSON on failure.
/// Catches panics (when built with `panic = "unwind"`) and returns them as error JSON.
///
/// Uses `std::hint::black_box` to prevent the compiler from optimizing away
/// the parse when the AST result is unused.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes.
#[cfg(feature = "parse")]
unsafe fn with_source_parse_internal<F, T>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
    f: F,
) -> *mut u8
where
    F: FnOnce(&str) -> Result<T, String> + panic::UnwindSafe,
{
    let source = match unsafe { extract_source(source_ptr, source_len, out_len) } {
        Ok(s) => s,
        Err(ptr) => return ptr,
    };

    match panic::catch_unwind(|| f(source)) {
        Ok(Ok(ast)) => {
            // Prevent compiler from optimizing away the parse
            std::hint::black_box(ast);
            string_to_ptr(String::new(), out_len) // Success: empty string
        }
        Ok(Err(e)) => error_result(&e, out_len),
        Err(payload) => error_result(&format_panic(&*payload), out_len),
    }
}

/// Convert a String to a raw pointer, writing the length to `out_len`.
fn string_to_ptr(s: String, out_len: *mut usize) -> *mut u8 {
    let bytes = s.into_bytes().into_boxed_slice();
    // Safety: out_len is guaranteed valid by caller contract
    unsafe { *out_len = bytes.len() };
    Box::into_raw(bytes).cast::<u8>()
}

/// Return an error as a JSON object.
fn error_result(message: &str, out_len: *mut usize) -> *mut u8 {
    let error = serde_json::json!({ "error": message });
    #[allow(clippy::unwrap_used)] // JSON serialization of simple object won't fail
    let json = serde_json::to_string(&error).unwrap();
    string_to_ptr(json, out_len)
}

/// Run `f` with a per-thread reusable AST arena.
///
/// The native bindings are called once per file in tight loops (formatters,
/// editor save hooks, benchmarks). Allocating a fresh `Bump` per call — and
/// freeing it at call end — churns the system allocator's heap high-water on
/// every call, which is measurable through a host FFI layer even when the engine
/// work is unchanged. Instead each thread keeps one `Bump` and `reset()`s it
/// between calls: `reset()` rewinds the bump pointer and retains the largest
/// chunk, so once a thread warms to its high-water mark there is no per-call
/// malloc/free (this supersedes per-call `with_capacity` pre-sizing — the first
/// few calls pay the chunk-growth tail once, then it amortizes to zero).
///
/// Soundness: the per-file AST borrows `&Bump` and is fully consumed inside `f`
/// (the returned value owns its bytes — a formatted `String`, a JSON `String`,
/// or `()`), so no AST outlives the `reset()` at the start of the next call;
/// `reset()` also recovers cleanly after a `catch_unwind`-caught panic. A future
/// `tsv_napi` should mirror this shape (the shared native-binding reuse path).
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

/// Run `f` with a per-thread reusable doc arena (the `format` path's analogue of
/// [`with_ast_arena`]).
///
/// The doc IR (`DocArena`) is rebuilt per call from the parsed AST; reusing one
/// arena per thread and `reset()`ing it between calls retains the backing buffers
/// instead of allocating a fresh doc arena per file — the same per-call heap-churn
/// argument as the AST `Bump`. Soundness mirrors `with_ast_arena`: the doc tree is
/// fully rendered to the returned `String` inside `f`, so no `DocId` outlives the
/// next call's `reset()`. A future `tsv_napi` mirrors this shape.
#[cfg(feature = "format")]
fn with_doc_arena<R>(f: impl FnOnce(&tsv_lang::doc::arena::DocArena) -> R) -> R {
    thread_local! {
        static DOC_ARENA: std::cell::RefCell<tsv_lang::doc::arena::DocArena> =
            std::cell::RefCell::new(tsv_lang::doc::arena::DocArena::new());
    }
    DOC_ARENA.with(|cell| {
        let mut arena = cell.borrow_mut();
        arena.reset();
        f(&arena)
    })
}

/// Generate `tsv_parse_<lang>` / `tsv_parse_internal_<lang>` / `tsv_format_<lang>`
/// C FFI functions for one language module.
///
/// # Safety (applies to every generated function)
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
macro_rules! lang_bindings {
    ($parse_fn:ident, $parse_internal_fn:ident, $format_fn:ident, $lang:ident) => {
        /// Parse source code and return JSON AST.
        ///
        /// # Safety
        /// See the module-level safety contract.
        #[cfg(feature = "parse")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $parse_fn(
            source_ptr: *const u8,
            source_len: usize,
            out_len: *mut usize,
        ) -> *mut u8 {
            unsafe {
                with_source_string(source_ptr, source_len, out_len, |source| {
                    with_ast_arena(|arena| {
                        let ast = $lang::parse(source, arena).map_err(|e| e.to_string())?;
                        Ok($lang::convert_ast_json_string(&ast, source))
                    })
                })
            }
        }

        /// Parse source to internal AST only (no conversion, no serialization).
        /// Returns empty string on success for minimal overhead benchmarking.
        ///
        /// # Safety
        /// See the module-level safety contract.
        #[cfg(feature = "parse")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $parse_internal_fn(
            source_ptr: *const u8,
            source_len: usize,
            out_len: *mut usize,
        ) -> *mut u8 {
            unsafe {
                with_source_parse_internal(source_ptr, source_len, out_len, |source| {
                    with_ast_arena(|arena| {
                        let ast = $lang::parse(source, arena).map_err(|e| e.to_string())?;
                        // Consume the borrowed AST before `with_ast_arena` resets
                        // it on the next call (the AST borrows from `arena`).
                        std::hint::black_box(&ast);
                        Ok(())
                    })
                })
            }
        }

        /// Format source code.
        ///
        /// # Safety
        /// See the module-level safety contract.
        #[cfg(feature = "format")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $format_fn(
            source_ptr: *const u8,
            source_len: usize,
            out_len: *mut usize,
        ) -> *mut u8 {
            unsafe {
                with_source_string(source_ptr, source_len, out_len, |source| {
                    with_ast_arena(|arena| {
                        let ast = $lang::parse(source, arena).map_err(|e| e.to_string())?;
                        Ok(with_doc_arena(|doc_arena| {
                            $lang::format_in(&ast, source, doc_arena)
                        }))
                    })
                })
            }
        }
    };
}

lang_bindings!(
    tsv_parse_svelte,
    tsv_parse_internal_svelte,
    tsv_format_svelte,
    tsv_svelte
);
lang_bindings!(
    tsv_parse_typescript,
    tsv_parse_internal_typescript,
    tsv_format_typescript,
    tsv_ts
);
lang_bindings!(
    tsv_parse_css,
    tsv_parse_internal_css,
    tsv_format_css,
    tsv_css
);

//
// Memory Management
//

/// Free memory allocated by tsv_* functions.
///
/// # Safety
/// - `ptr` must be a pointer previously returned by a tsv_* function
/// - `len` must be the length written to `out_len` by that function
/// - Must be called exactly once per allocation
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        // Safety: Caller guarantees ptr was allocated by us with the given len
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// The shared signature of every return-pointer FFI entry point.
    type FfiFn = unsafe extern "C" fn(*const u8, usize, *mut usize) -> *mut u8;

    /// Drive an FFI entry point end to end: pass `source`, read the returned
    /// buffer back into a `String`, then free it via `tsv_free`. Every call
    /// exercises the real alloc → write `out_len` → free round-trip, so a
    /// mismatch between the returned length and the buffer is caught here.
    fn call(f: FfiFn, source: &str) -> String {
        call_bytes(f, source.as_bytes())
    }

    /// Like `call` but takes raw bytes, so tests can pass invalid UTF-8.
    fn call_bytes(f: FfiFn, bytes: &[u8]) -> String {
        let mut out_len: usize = 0;
        // Safety: `bytes` is a valid slice; `out_len` is a live `usize`.
        let ptr = unsafe { f(bytes.as_ptr(), bytes.len(), &raw mut out_len) };
        assert!(!ptr.is_null(), "FFI returned a null pointer");
        // Safety: the call wrote `out_len` bytes starting at `ptr`.
        let out = unsafe { slice::from_raw_parts(ptr, out_len) };
        let s = std::str::from_utf8(out)
            .expect("FFI output must be valid UTF-8")
            .to_owned();
        assert_eq!(
            out_len,
            s.len(),
            "out_len must match the returned byte count"
        );
        // Safety: `ptr`/`out_len` came from the call above; freed exactly once.
        unsafe { tsv_free(ptr, out_len) };
        s
    }

    /// Return the `error` message if `output` is an `{"error": "..."}` object.
    fn error_message(output: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(output).ok()?;
        value.get("error")?.as_str().map(str::to_owned)
    }

    // --- format: happy path (one per language exercises the macro expansion) ---

    #[test]
    fn format_typescript_normalizes() {
        assert_eq!(call(tsv_format_typescript, "const   x=1"), "const x = 1;\n");
    }

    #[test]
    fn format_css_normalizes() {
        assert_eq!(
            call(tsv_format_css, "a{color:red}"),
            "a {\n\tcolor: red;\n}\n"
        );
    }

    #[test]
    fn format_svelte_normalizes() {
        assert_eq!(
            call(tsv_format_svelte, "<div   >x</div   >"),
            "<div>x</div>\n"
        );
    }

    // --- parse: returns a JSON AST keyed by `type`, no error ---

    #[test]
    fn parse_returns_json_ast() {
        // Annotate the array type so the fn items coerce to `FfiFn` (no casts).
        let cases: [(&str, FfiFn, &str); 3] = [
            ("typescript", tsv_parse_typescript, "const x = 1;\n"),
            ("svelte", tsv_parse_svelte, "<div>x</div>\n"),
            ("css", tsv_parse_css, "a {\n\tcolor: red;\n}\n"),
        ];
        for (label, f, src) in cases {
            let out = call(f, src);
            let value: serde_json::Value =
                serde_json::from_str(&out).unwrap_or_else(|e| panic!("{label}: not JSON: {e}"));
            assert!(
                value
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .is_some(),
                "{label}: AST root missing a string `type` field: {out}"
            );
            assert!(
                error_message(&out).is_none(),
                "{label}: unexpected error: {out}"
            );
        }
    }

    // --- parse_internal: empty string on success, error JSON on failure ---

    #[test]
    fn parse_internal_empty_on_success() {
        assert_eq!(call(tsv_parse_internal_typescript, "const x = 1;\n"), "");
        assert_eq!(call(tsv_parse_internal_svelte, "<div>x</div>\n"), "");
        assert_eq!(call(tsv_parse_internal_css, "a {\n\tcolor: red;\n}\n"), "");
    }

    #[test]
    fn parse_internal_reports_errors() {
        // Cover the error arm of `with_source_parse_internal` for all three
        // languages (success arm is covered above for all three).
        let cases: [(&str, FfiFn, &str); 3] = [
            ("typescript", tsv_parse_internal_typescript, "const ="),
            ("svelte", tsv_parse_internal_svelte, "<div {"),
            ("css", tsv_parse_internal_css, "a {"),
        ];
        for (label, f, src) in cases {
            let out = call(f, src);
            assert!(
                error_message(&out).is_some(),
                "{label}: expected error JSON, got: {out}"
            );
        }
    }

    // --- format_panic renders each payload variant (pure, no panic needed) ---

    #[test]
    fn format_panic_renders_payload_variants() {
        use std::any::Any;
        let as_str: Box<dyn Any + Send> = Box::new("boom");
        assert_eq!(format_panic(&*as_str), "panic: boom");
        let owned: Box<dyn Any + Send> = Box::new(String::from("kaboom"));
        assert_eq!(format_panic(&*owned), "panic: kaboom");
        let other: Box<dyn Any + Send> = Box::new(42i32);
        assert_eq!(format_panic(&*other), "panic: <unknown>");
    }

    // --- multibyte sources survive the UTF-8 / char-offset marshalling boundary ---

    #[test]
    fn parse_and_format_preserve_multibyte_source() {
        let src = "const x = '€🦀';\n";
        let parsed = call(tsv_parse_typescript, src);
        assert!(
            error_message(&parsed).is_none(),
            "unexpected error: {parsed}"
        );
        let formatted = call(tsv_format_typescript, src);
        assert!(
            formatted.contains("€🦀"),
            "multibyte content lost: {formatted}"
        );
        // Re-formatting is stable (idempotent) across the boundary.
        assert_eq!(call(tsv_format_typescript, &formatted), formatted);
    }

    // --- error path: invalid syntax surfaces as JSON error (and still frees) ---

    #[test]
    fn invalid_syntax_returns_json_error() {
        let cases: [(&str, FfiFn, FfiFn, &str); 3] = [
            (
                "typescript",
                tsv_parse_typescript,
                tsv_format_typescript,
                "const =",
            ),
            ("css", tsv_parse_css, tsv_format_css, "a {"),
            ("svelte", tsv_parse_svelte, tsv_format_svelte, "<div {"),
        ];
        for (label, parse_fn, format_fn, src) in cases {
            assert!(
                error_message(&call(parse_fn, src)).is_some(),
                "{label} parse: expected error JSON for {src:?}"
            );
            assert!(
                error_message(&call(format_fn, src)).is_some(),
                "{label} format: expected error JSON for {src:?}"
            );
        }
    }

    // --- invalid UTF-8 is reported, not a crash (module safety contract) ---

    #[test]
    fn invalid_utf8_returns_error() {
        // 0xFF is never valid in UTF-8.
        let out = call_bytes(tsv_format_typescript, &[b'a', 0xFF, b'b']);
        let msg = error_message(&out).expect("expected an error object");
        assert!(
            msg.starts_with("Invalid UTF-8"),
            "expected a UTF-8 error, got: {msg}"
        );
    }

    // --- empty input formats to empty output and round-trips through free ---

    #[test]
    fn empty_input_is_handled() {
        // Format of empty input is empty for every language.
        assert_eq!(call(tsv_format_typescript, ""), "");
        assert_eq!(call(tsv_format_css, ""), "");
        assert_eq!(call(tsv_format_svelte, ""), "");
        // Parse of empty input succeeds (a valid root, no error) for every language.
        let parsers: [(&str, FfiFn); 3] = [
            ("typescript", tsv_parse_typescript),
            ("css", tsv_parse_css),
            ("svelte", tsv_parse_svelte),
        ];
        for (label, f) in parsers {
            let out = call(f, "");
            assert!(
                error_message(&out).is_none(),
                "{label}: empty parse errored: {out}"
            );
        }
    }

    // --- tsv_free tolerates null / zero-length (documented no-op) ---

    #[test]
    fn tsv_free_null_and_zero_are_noops() {
        // Safety: null and zero-length are the explicit no-op cases.
        unsafe {
            tsv_free(std::ptr::null_mut(), 0);
            tsv_free(std::ptr::null_mut(), 8);
            tsv_free(std::ptr::dangling_mut(), 0);
        }
    }
}
