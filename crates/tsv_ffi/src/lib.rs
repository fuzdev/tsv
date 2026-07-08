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
//! - `source_ptr` points to valid UTF-8 data of `source_len` bytes (a null
//!   `source_ptr` with `source_len == 0` is accepted as the empty source; a
//!   null pointer with a non-zero length returns an error JSON)
//! - `out_len` points to a valid `usize` location for writing the output length
//! - The returned pointer is freed exactly once via `tsv_free`

#![allow(unsafe_code)]

use std::panic;
use std::slice;

// Per-thread reusable arenas live in the shared `tsv_arena` crate (used by both
// native bindings — see its module docs for the reuse rationale + soundness;
// the FFI path additionally relies on `reset()` recovering cleanly after a
// `catch_unwind`-caught panic).
use tsv_arena::with_ast_arena;
#[cfg(feature = "format")]
use tsv_arena::with_doc_arena;

/// Extract a &str from source pointer, or return an error result.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes
/// (a null `source_ptr` is tolerated when `source_len` is 0, and reported as an
/// error otherwise).
unsafe fn extract_source<'a>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> Result<&'a str, *mut u8> {
    // An empty source needs no read at all — and short-circuiting matters for
    // soundness: `slice::from_raw_parts` requires a non-null pointer even for
    // length 0, while FFI hosts commonly hand (null, 0) for an empty buffer
    // (e.g. Deno's `UnsafePointer.of` on an empty typed array is null).
    if source_len == 0 {
        return Ok("");
    }
    if source_ptr.is_null() {
        return Err(error_result("Null source pointer", out_len));
    }
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
/// output payload verbatim (formatted source `String`, or already-serialized
/// JSON wire bytes — the parse path returns `Vec<u8>` so the writer's
/// UTF-8-by-construction output is never re-validated).
/// Catches panics (when built with `panic = "unwind"`) and returns them as error JSON.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes.
unsafe fn with_source_string<F, B>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
    f: F,
) -> *mut u8
where
    F: FnOnce(&str) -> Result<B, String> + panic::UnwindSafe,
    B: Into<Vec<u8>>,
{
    let source = match unsafe { extract_source(source_ptr, source_len, out_len) } {
        Ok(s) => s,
        Err(ptr) => return ptr,
    };

    match panic::catch_unwind(|| f(source)) {
        Ok(Ok(result)) => bytes_to_ptr(result, out_len),
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
            bytes_to_ptr(Vec::new(), out_len) // Success: empty output
        }
        Ok(Err(e)) => error_result(&e, out_len),
        Err(payload) => error_result(&format_panic(&*payload), out_len),
    }
}

/// Convert an output payload (JSON bytes or a formatted `String` — anything
/// byte-convertible) to a raw pointer, writing the length to `out_len`.
fn bytes_to_ptr(payload: impl Into<Vec<u8>>, out_len: *mut usize) -> *mut u8 {
    let bytes = payload.into().into_boxed_slice();
    // Safety: out_len is guaranteed valid by caller contract
    unsafe { *out_len = bytes.len() };
    Box::into_raw(bytes).cast::<u8>()
}

/// Return an error as a JSON object.
fn error_result(message: &str, out_len: *mut usize) -> *mut u8 {
    let error = serde_json::json!({ "error": message });
    #[allow(clippy::unwrap_used)] // JSON serialization of simple object won't fail
    let json = serde_json::to_string(&error).unwrap();
    bytes_to_ptr(json, out_len)
}

/// Generate `tsv_parse_<lang>` / `tsv_parse_internal_<lang>` / `tsv_format_<lang>`
/// C FFI functions for one language module.
///
/// # Safety (applies to every generated function)
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
macro_rules! lang_bindings {
    ($parse_fn:ident, $parse_no_loc_fn:ident, $parse_internal_fn:ident, $format_fn:ident, $lang:ident) => {
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
                        Ok($lang::convert_ast_json_bytes(&ast, source))
                    })
                })
            }
        }

        /// Parse source and return JSON AST **without** per-node `loc` (the
        /// span-only `no-locations` wire — see the language crate's
        /// `convert_ast_json_bytes_no_locations`). CSS is identical to
        /// `$parse_fn` (`parseCss` emits no `loc`).
        ///
        /// # Safety
        /// See the module-level safety contract.
        #[cfg(feature = "parse")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $parse_no_loc_fn(
            source_ptr: *const u8,
            source_len: usize,
            out_len: *mut usize,
        ) -> *mut u8 {
            unsafe {
                with_source_string(source_ptr, source_len, out_len, |source| {
                    with_ast_arena(|arena| {
                        let ast = $lang::parse(source, arena).map_err(|e| e.to_string())?;
                        Ok($lang::convert_ast_json_bytes_no_locations(&ast, source))
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
    tsv_parse_svelte_no_locations,
    tsv_parse_internal_svelte,
    tsv_format_svelte,
    tsv_svelte
);
lang_bindings!(
    tsv_parse_typescript,
    tsv_parse_typescript_no_locations,
    tsv_parse_internal_typescript,
    tsv_format_typescript,
    tsv_ts
);
lang_bindings!(
    tsv_parse_css,
    tsv_parse_css_no_locations,
    tsv_parse_internal_css,
    tsv_format_css,
    tsv_css
);

//
// Goal-aware TypeScript parse (script vs module)
//
// The parse goal is TypeScript-only (Svelte `<script>` is always a module; CSS
// has no goal), so — like `tsv_wasm` — these live outside `lang_bindings!`
// rather than threading a meaningless goal through svelte/css. The goalless
// `tsv_parse_typescript*` exports remain the `Module` default; these mirror them
// against an explicit goal code (`0` = Module, anything else = Script). At Script
// goal, `await` is an ordinary identifier and `import`/`export`/`import.meta` are
// syntax errors. See `tsv parse --goal` and `tsv_ts::parse_with_goal`.

/// Map the C-ABI goal code to `tsv_ts::Goal` (`0` = Module, else Script).
#[cfg(feature = "parse")]
fn ffi_goal(goal: u32) -> tsv_ts::Goal {
    if goal == 0 {
        tsv_ts::Goal::Module
    } else {
        tsv_ts::Goal::Script
    }
}

/// `tsv_parse_typescript` (JSON AST) against an explicit goal.
///
/// # Safety
/// See the module-level safety contract.
#[cfg(feature = "parse")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_typescript_with_goal(
    source_ptr: *const u8,
    source_len: usize,
    goal: u32,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_string(source_ptr, source_len, out_len, |source| {
            with_ast_arena(|arena| {
                let ast = tsv_ts::parse_with_goal(source, ffi_goal(goal), arena)
                    .map_err(|e| e.to_string())?;
                Ok(tsv_ts::convert_ast_json_bytes(&ast, source))
            })
        })
    }
}

/// `tsv_parse_typescript_no_locations` (span-only JSON AST) against an explicit goal.
///
/// # Safety
/// See the module-level safety contract.
#[cfg(feature = "parse")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_typescript_no_locations_with_goal(
    source_ptr: *const u8,
    source_len: usize,
    goal: u32,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_string(source_ptr, source_len, out_len, |source| {
            with_ast_arena(|arena| {
                let ast = tsv_ts::parse_with_goal(source, ffi_goal(goal), arena)
                    .map_err(|e| e.to_string())?;
                Ok(tsv_ts::convert_ast_json_bytes_no_locations(&ast, source))
            })
        })
    }
}

/// `tsv_parse_internal_typescript` (parse-only, no serialization) against an
/// explicit goal — the coverage/throughput probe.
///
/// # Safety
/// See the module-level safety contract.
#[cfg(feature = "parse")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_internal_typescript_with_goal(
    source_ptr: *const u8,
    source_len: usize,
    goal: u32,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_parse_internal(source_ptr, source_len, out_len, |source| {
            with_ast_arena(|arena| {
                let ast = tsv_ts::parse_with_goal(source, ffi_goal(goal), arena)
                    .map_err(|e| e.to_string())?;
                std::hint::black_box(&ast);
                Ok(())
            })
        })
    }
}

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

    // --- goal-aware TS parse: script accepts `await` as an identifier, module rejects ---

    #[test]
    fn parse_typescript_with_goal_switches_await() {
        type GoalFn = unsafe extern "C" fn(*const u8, usize, u32, *mut usize) -> *mut u8;
        fn call_goal(f: GoalFn, source: &str, goal: u32) -> String {
            let bytes = source.as_bytes();
            let mut out_len: usize = 0;
            // Safety: `bytes` is a valid slice; `out_len` is a live `usize`; the
            // call writes `out_len` bytes at `ptr`, freed exactly once.
            unsafe {
                let ptr = f(bytes.as_ptr(), bytes.len(), goal, &raw mut out_len);
                let s = String::from_utf8_lossy(slice::from_raw_parts(ptr, out_len)).into_owned();
                tsv_free(ptr, out_len);
                s
            }
        }
        const MODULE: u32 = 0;
        const SCRIPT: u32 = 1;
        // `await` is an ordinary identifier at Script goal, reserved at Module goal.
        let src = "var await = 1;\n";
        for f in [
            tsv_parse_typescript_with_goal,
            tsv_parse_typescript_no_locations_with_goal,
        ] {
            assert!(
                error_message(&call_goal(f, src, SCRIPT)).is_none(),
                "script goal should accept `await` as identifier"
            );
            assert!(
                error_message(&call_goal(f, src, MODULE)).is_some(),
                "module goal should reject `await` as identifier"
            );
        }
        // parse_internal returns "" on success, error JSON on failure.
        assert_eq!(
            call_goal(tsv_parse_internal_typescript_with_goal, src, SCRIPT),
            ""
        );
        assert!(
            error_message(&call_goal(
                tsv_parse_internal_typescript_with_goal,
                src,
                MODULE
            ))
            .is_some()
        );
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

    // --- null source pointer: (null, 0) is the empty source; (null, n>0) errors ---

    #[test]
    fn null_source_pointer_is_handled() {
        let mut out_len: usize = 0;
        // (null, 0) — the empty source, as FFI hosts commonly pass it (e.g.
        // Deno's `UnsafePointer.of` on an empty typed array is null). Formats
        // to empty output, no error.
        // Safety: extract_source short-circuits before any read.
        let ptr = unsafe { tsv_format_typescript(std::ptr::null(), 0, &raw mut out_len) };
        assert!(!ptr.is_null(), "FFI returned a null pointer");
        let out = unsafe { slice::from_raw_parts(ptr, out_len) };
        assert_eq!(out, b"", "(null, 0) must format as the empty source");
        unsafe { tsv_free(ptr, out_len) };

        // (null, n>0) — an invalid buffer; must surface as an error JSON, not UB.
        // Safety: the null check precedes any read of the (bogus) 5 bytes.
        let ptr = unsafe { tsv_format_typescript(std::ptr::null(), 5, &raw mut out_len) };
        assert!(!ptr.is_null(), "FFI returned a null pointer");
        let out = unsafe { slice::from_raw_parts(ptr, out_len) };
        let msg = error_message(std::str::from_utf8(out).expect("error JSON is UTF-8"))
            .expect("expected an error object");
        assert!(
            msg.contains("Null source pointer"),
            "expected a null-pointer error, got: {msg}"
        );
        unsafe { tsv_free(ptr, out_len) };
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
