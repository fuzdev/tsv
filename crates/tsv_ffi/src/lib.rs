//! C FFI bindings for tsv
//!
//! Provides parse and format functions with C ABI for use from any language
//! with C FFI support (Deno, Node.js via koffi/ffi-napi, Python ctypes, etc.).
//!
//! # The uniform signature
//!
//! Every export takes the same five arguments —
//! `(source_ptr, source_len, goal, out_len, out_status)` — and returns one
//! `*mut u8`. One shape per language and operation: there is no goalless twin of
//! a goal-aware export, and no arity that varies by language. `goal` is the parse
//! goal (`0` = module, `1` = script); a language with no goal axis (Svelte, CSS)
//! *rejects* a non-zero code rather than ignoring it, so a caller cannot believe
//! it selected a goal that was silently dropped. The source buffer is decoded
//! first, so a call that is wrong in both ways reports the buffer, not the goal —
//! there is one error per call and the earlier failure wins.
//!
//! # Errors: the status word, not the payload
//!
//! `out_status` receives [`TSV_STATUS_OK`] or [`TSV_STATUS_ERROR`], written
//! exactly once per call alongside `out_len`. That word — never the payload's
//! shape — is what tells success from failure. The payload of a failed call is a
//! `{"error": "…"}` JSON object, but a caller must not sniff for it: formatted
//! output is arbitrary source text, so no prefix test can be sound in general.
//!
//! # Memory Management
//!
//! All functions that return `*mut u8` allocate memory that the caller must free
//! by calling `tsv_free(ptr, len)` with the returned pointer and length — on the
//! error path too.
//!
//! # Safety
//!
//! These functions use raw pointers for FFI compatibility. The caller must ensure:
//! - `source_ptr` points to valid UTF-8 data of `source_len` bytes (a null
//!   `source_ptr` with `source_len == 0` is accepted as the empty source; a
//!   null pointer with a non-zero length reports an error)
//! - `out_len` points to a valid `usize` location for writing the output length
//! - `out_status` points to a valid `u32` location for writing the status
//! - The returned pointer is freed exactly once via `tsv_free`

#![allow(unsafe_code)]

use std::panic;
use std::slice;

// Per-thread reusable arenas live in the shared `tsv_arena` crate (used by both
// native bindings — see its module docs for the reuse rationale + soundness;
// the FFI path additionally relies on `reset()` recovering cleanly after a
// `catch_unwind`-caught panic). The goal-axis macros come from the same crate,
// so the three bindings share ONE definition of which languages have a goal
// rather than three hand-synced copies.
use tsv_arena::with_ast_arena;
#[cfg(feature = "format")]
use tsv_arena::with_doc_arena;
#[cfg(any(feature = "parse", feature = "format"))]
use tsv_arena::{goal_allowed, parse_ast};

/// `*out_status` for a call that produced its payload: the returned bytes are the
/// wire JSON, the formatted source, or (for `tsv_parse_internal_*`) empty.
pub const TSV_STATUS_OK: u32 = 0;

/// `*out_status` for a call that failed: the returned bytes are a
/// `{"error": "…"}` JSON object.
///
/// The status word is the whole test. An earlier revision had callers sniff the
/// payload for the `{"error"` prefix, which was sound only because tsv normalizes
/// strings to single quotes — a correctness dependency on a *style* setting, over
/// a channel that carries arbitrary formatted source.
pub const TSV_STATUS_ERROR: u32 = 1;

/// Decode the caller's source buffer and render a payload from it: either `f`'s,
/// or an error payload when the buffer can't be decoded.
///
/// The `&str` is handed *into* `f` rather than returned, which is what bounds
/// its lifetime to this call. Returning it would need a lifetime appearing only
/// in the return type — caller-chosen, `'static` included, with nothing tying it
/// to the host's buffer (the `CStr::from_ptr` shape). Passing it down makes that
/// unrepresentable, and matches the `with_*` idiom the rest of the crate and
/// `tsv_arena` already use.
///
/// Exactly one payload is rendered per call — `f` runs only on the paths that
/// don't render an error — so `out_len` and `out_status` are each written exactly
/// once either way.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 of `source_len` bytes (a null
///   `source_ptr` is tolerated when `source_len` is 0, and reported as an error
///   otherwise)
/// - `out_len` must be valid for writes of a `usize`
/// - `out_status` must be valid for writes of a `u32`
unsafe fn with_extracted_source(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
    out_status: *mut u32,
    f: impl FnOnce(&str) -> *mut u8,
) -> *mut u8 {
    // An empty source needs no read at all — and short-circuiting matters for
    // soundness: `slice::from_raw_parts` requires a non-null pointer even for
    // length 0, while FFI hosts commonly hand (null, 0) for an empty buffer
    // (e.g. Deno's `UnsafePointer.of` on an empty typed array is null).
    if source_len == 0 {
        return f("");
    }
    if source_ptr.is_null() {
        // SAFETY: out-param validity is the caller's contract, forwarded.
        return unsafe { error_result("Null source pointer", out_len, out_status) };
    }
    // SAFETY: caller guarantees `source_ptr` is valid for `source_len` bytes,
    // and the null/empty cases are handled above.
    let bytes = unsafe { slice::from_raw_parts(source_ptr, source_len) };
    match std::str::from_utf8(bytes) {
        Ok(s) => f(s),
        // SAFETY: out-param validity is the caller's contract, forwarded.
        Err(e) => unsafe { error_result(&format!("Invalid UTF-8: {e}"), out_len, out_status) },
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
/// UTF-8-by-construction output is never re-validated; the benchmark-only
/// internal-parse path returns an empty `Vec`, which renders as empty output).
/// Catches panics (when built with `panic = "unwind"`) and reports them through
/// the error status.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes.
unsafe fn with_source_string<F, B>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
    out_status: *mut u32,
    f: F,
) -> *mut u8
where
    F: FnOnce(&str) -> Result<B, String> + panic::UnwindSafe,
    B: Into<Vec<u8>>,
{
    let render = |source: &str| match panic::catch_unwind(|| f(source)) {
        // SAFETY: out-param validity is the caller's contract.
        Ok(Ok(result)) => unsafe { bytes_to_ptr(result, TSV_STATUS_OK, out_len, out_status) },
        Ok(Err(e)) => unsafe { error_result(&e, out_len, out_status) },
        Err(payload) => unsafe { error_result(&format_panic(&*payload), out_len, out_status) },
    };
    // SAFETY: the pointer contract is this function's own, forwarded verbatim.
    unsafe { with_extracted_source(source_ptr, source_len, out_len, out_status, render) }
}

/// Convert an output payload (JSON bytes or a formatted `String` — anything
/// byte-convertible) to a raw pointer, writing the length to `out_len` and
/// `status` to `out_status`.
///
/// The single site that writes either out-param, so the two can never disagree
/// about which call they describe.
///
/// # Safety
/// - `out_len` must be valid for writes of a `usize`, `out_status` for a `u32`
/// - the returned pointer owns a boxed slice of the written length, and is
///   released only by `tsv_free(ptr, len)` with that same length
unsafe fn bytes_to_ptr(
    payload: impl Into<Vec<u8>>,
    status: u32,
    out_len: *mut usize,
    out_status: *mut u32,
) -> *mut u8 {
    let bytes = payload.into().into_boxed_slice();
    // SAFETY: out-param validity is the caller's contract.
    unsafe {
        *out_len = bytes.len();
        *out_status = status;
    }
    Box::into_raw(bytes).cast::<u8>()
}

/// Report an error: [`TSV_STATUS_ERROR`] plus a `{"error": "…"}` JSON payload the
/// caller still owns and must free.
///
/// # Safety
/// Same contract as [`bytes_to_ptr`], which renders the payload.
unsafe fn error_result(message: &str, out_len: *mut usize, out_status: *mut u32) -> *mut u8 {
    let error = serde_json::json!({ "error": message });
    #[expect(clippy::unwrap_used)] // JSON serialization of simple object won't fail
    let json = serde_json::to_string(&error).unwrap();
    // SAFETY: out-param validity is the caller's contract, forwarded.
    unsafe { bytes_to_ptr(json, TSV_STATUS_ERROR, out_len, out_status) }
}

/// Map the C-ABI goal code to `tsv_ts::Goal` (`0` = Module, `1` = Script).
///
/// `allowed` is the language's goal axis ([`goal_allowed!`]). A non-zero code
/// against a language that has none is an **error**, not a silent Module: Svelte
/// hard-wires `Module` and CSS has no goal, so a caller passing `1` there asked
/// for something that cannot be honored and must be told — the same stance
/// `tsv_wasm`'s `read_options` takes when it rejects the `goal` key outright. Any
/// unrecognized code is an error whatever the language.
#[cfg(any(feature = "parse", feature = "format"))]
fn ffi_goal(goal: u32, allowed: bool) -> Result<tsv_ts::Goal, String> {
    match goal {
        0 => Ok(tsv_ts::Goal::Module),
        1 if allowed => Ok(tsv_ts::Goal::Script),
        1 => Err(
            "goal code 1 (script) is only supported for TypeScript (expected 0 = module)"
                .to_string(),
        ),
        other => Err(format!(
            "invalid goal code {other} (expected 0 = module or 1 = script)"
        )),
    }
}

/// Generate `tsv_parse_<lang>` / `tsv_parse_<lang>_no_locations` /
/// `tsv_parse_internal_<lang>` / `tsv_format_<lang>` C FFI functions for one
/// language module.
///
/// # Safety (applies to every generated function)
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - `out_status` must point to a valid `u32` for writing the status
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
// Per-language compound-op helpers: parse the source into a per-thread AST arena
// and run the conversion/format/no-op over it. Every language crate is
// interner-free (identifier and element/attribute names are span-identity), so
// these are uniform across svelte/typescript/css — no per-language arity split.
#[cfg(feature = "parse")]
macro_rules! parse_convert {
    ($goalness:ident, $lang:ident, $conv:ident, $source:expr, $goal:expr) => {
        with_ast_arena(|arena| {
            let ast =
                parse_ast!($goalness, $lang, $source, $goal, arena).map_err(|e| e.to_string())?;
            Ok($lang::$conv(&ast, $source))
        })
    };
}

// Benchmark-only: parse and throw the AST away, so the timing isolates the parse
// from the convert/serialize layers. Success renders as an empty payload.
//
// `black_box(&ast)` sits *inside* the arena closure on purpose — that is the only
// scope where the AST still exists, so nothing further out can stand in for it.
// Drop it and the whole parse becomes dead code the optimizer may delete, leaving
// a benchmark that got faster by measuring nothing.
#[cfg(feature = "parse")]
macro_rules! parse_internal {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr) => {
        with_ast_arena(|arena| {
            let ast =
                parse_ast!($goalness, $lang, $source, $goal, arena).map_err(|e| e.to_string())?;
            std::hint::black_box(&ast);
            Ok(Vec::new())
        })
    };
}

#[cfg(feature = "format")]
macro_rules! parse_format {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr) => {{
        // The format path's line-terminator fold, ahead of the parse — see
        // `tsv_lang::printing::normalize_carriage_returns`. `parse_convert!` deliberately
        // skips it: the wire's offsets are a drop-in contract over the author's own bytes.
        let normalized = tsv_lang::printing::normalize_carriage_returns($source);
        let source = normalized.as_ref();
        with_ast_arena(|arena| {
            let ast =
                parse_ast!($goalness, $lang, source, $goal, arena).map_err(|e| e.to_string())?;
            Ok(with_doc_arena(|doc_arena| {
                $lang::format_in(&ast, source, doc_arena)
            }))
        })
    }};
}

// One export per (language, operation) — every one taking the same five
// arguments. The `$goalness` axis decides only whether a non-Module goal CODE is
// accepted, never the arity: an FFI host writes one call shape and one symbol
// table, and there is no goalless twin to drift from its goal-aware sibling.
//
// At Script goal `await` is an ordinary identifier and `import`/`export`/
// `import.meta` are syntax errors. See `tsv parse --goal` / `tsv format --goal`
// and `tsv_ts::parse_with_goal`.
//
// `tsv_wasm` spells the same axis as one key of a per-call options bag
// (`format_typescript(src, {goal})`) and `tsv_napi` as a trailing optional goal
// string; each binding's own `lang_bindings!` reads the SAME `parse_ast!` /
// `goal_allowed!` pair out of `tsv_arena`, so which languages have a goal axis
// is one fact in one place rather than three that agree today. See
// `crates/tsv_wasm/CLAUDE.md` §Format Options.
macro_rules! lang_bindings {
    (
        $goalness:ident,
        $parse_fn:ident,
        $parse_no_loc_fn:ident,
        $parse_internal_fn:ident,
        $format_fn:ident,
        $lang:ident $(,)?
    ) => {
        /// Parse source code and return JSON AST.
        ///
        /// # Safety
        /// See the module-level safety contract.
        #[cfg(feature = "parse")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $parse_fn(
            source_ptr: *const u8,
            source_len: usize,
            goal: u32,
            out_len: *mut usize,
            out_status: *mut u32,
        ) -> *mut u8 {
            unsafe {
                with_source_string(source_ptr, source_len, out_len, out_status, |source| {
                    let goal = ffi_goal(goal, goal_allowed!($goalness))?;
                    parse_convert!($goalness, $lang, convert_ast_json_bytes, source, goal)
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
            goal: u32,
            out_len: *mut usize,
            out_status: *mut u32,
        ) -> *mut u8 {
            unsafe {
                with_source_string(source_ptr, source_len, out_len, out_status, |source| {
                    let goal = ffi_goal(goal, goal_allowed!($goalness))?;
                    parse_convert!(
                        $goalness,
                        $lang,
                        convert_ast_json_bytes_no_locations,
                        source,
                        goal
                    )
                })
            }
        }

        /// Parse source to internal AST only (no conversion, no serialization).
        /// Returns an empty payload on success for minimal overhead benchmarking.
        ///
        /// # Safety
        /// See the module-level safety contract.
        #[cfg(feature = "parse")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $parse_internal_fn(
            source_ptr: *const u8,
            source_len: usize,
            goal: u32,
            out_len: *mut usize,
            out_status: *mut u32,
        ) -> *mut u8 {
            unsafe {
                with_source_string(source_ptr, source_len, out_len, out_status, |source| {
                    let goal = ffi_goal(goal, goal_allowed!($goalness))?;
                    parse_internal!($goalness, $lang, source, goal)
                })
            }
        }

        /// Format source code. The goal shapes only the parse the formatter runs;
        /// formatting itself is non-configurable.
        ///
        /// # Safety
        /// See the module-level safety contract.
        #[cfg(feature = "format")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $format_fn(
            source_ptr: *const u8,
            source_len: usize,
            goal: u32,
            out_len: *mut usize,
            out_status: *mut u32,
        ) -> *mut u8 {
            unsafe {
                with_source_string(source_ptr, source_len, out_len, out_status, |source| {
                    let goal = ffi_goal(goal, goal_allowed!($goalness))?;
                    parse_format!($goalness, $lang, source, goal)
                })
            }
        }
    };
}

lang_bindings!(
    nogoal,
    tsv_parse_svelte,
    tsv_parse_svelte_no_locations,
    tsv_parse_internal_svelte,
    tsv_format_svelte,
    tsv_svelte,
);
lang_bindings!(
    goal,
    tsv_parse_typescript,
    tsv_parse_typescript_no_locations,
    tsv_parse_internal_typescript,
    tsv_format_typescript,
    tsv_ts,
);
lang_bindings!(
    nogoal,
    tsv_parse_css,
    tsv_parse_css_no_locations,
    tsv_parse_internal_css,
    tsv_format_css,
    tsv_css,
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
mod tests {
    use super::*;

    /// The uniform signature of every return-pointer FFI entry point.
    type FfiFn = unsafe extern "C" fn(*const u8, usize, u32, *mut usize, *mut u32) -> *mut u8;

    const MODULE: u32 = 0;
    const SCRIPT: u32 = 1;

    /// Drive an FFI entry point end to end at `goal`: pass the bytes, read the
    /// returned buffer back into a `String`, then free it via `tsv_free`,
    /// returning `(*out_status, payload)`. Every call exercises the real alloc →
    /// write `out_len`/`out_status` → free round-trip, so `tsv_free` is handed
    /// exactly the length the call reported — the pairing the ownership contract
    /// rests on.
    ///
    /// What that does **not** prove is that `out_len` is the length the payload
    /// actually has: the returned slice is *defined* by `out_len`, so any
    /// self-consistency check against it is a tautology. A too-large `out_len`
    /// reads past the allocation (UB, not a failed assertion) and a too-small one
    /// silently truncates; only the per-export tests below, which compare the
    /// payload to an expected exact string, can see either.
    ///
    /// Pins two things the status channel does contract for: `out_status` is
    /// written on **every** call (the `u32::MAX` sentinel below would survive an
    /// export that forgot), and an error status MUST carry an `{"error": …}`
    /// payload. The converse is deliberately unasserted — a success payload is
    /// arbitrary source text, and asserting it *isn't* an error object would
    /// rebuild the content sniff this channel exists to retire.
    fn call_raw(f: FfiFn, bytes: &[u8], goal: u32) -> (u32, String) {
        let mut out_len: usize = 0;
        // Never a valid status, so an export that writes neither out-param is a
        // failure here rather than an inherited verdict.
        let mut out_status: u32 = u32::MAX;
        // Safety: `bytes` is a valid slice; both out-params are live locals.
        let ptr = unsafe {
            f(
                bytes.as_ptr(),
                bytes.len(),
                goal,
                &raw mut out_len,
                &raw mut out_status,
            )
        };
        assert!(!ptr.is_null(), "FFI returned a null pointer");
        // Safety: the call wrote `out_len` bytes starting at `ptr`.
        let out = unsafe { slice::from_raw_parts(ptr, out_len) };
        let s = std::str::from_utf8(out)
            .expect("FFI output must be valid UTF-8")
            .to_owned();
        // Safety: `ptr`/`out_len` came from the call above; freed exactly once.
        unsafe { tsv_free(ptr, out_len) };
        assert!(
            out_status == TSV_STATUS_OK || out_status == TSV_STATUS_ERROR,
            "out_status must be written on every call, got {out_status}"
        );
        if out_status == TSV_STATUS_ERROR {
            assert!(
                error_message(&s).is_some(),
                "error status without an `{{error}}` payload: {s}"
            );
        }
        (out_status, s)
    }

    /// The success path at Module goal: asserts [`TSV_STATUS_OK`], returns the payload.
    fn call(f: FfiFn, source: &str) -> String {
        call_goal(f, source, MODULE)
    }

    /// The success path at an explicit goal.
    fn call_goal(f: FfiFn, source: &str, goal: u32) -> String {
        let (status, out) = call_raw(f, source.as_bytes(), goal);
        assert_eq!(status, TSV_STATUS_OK, "expected success, got: {out}");
        out
    }

    /// The failure path at Module goal: asserts [`TSV_STATUS_ERROR`], returns the message.
    fn call_err(f: FfiFn, source: &str) -> String {
        call_err_goal(f, source, MODULE)
    }

    /// The failure path at an explicit goal.
    fn call_err_goal(f: FfiFn, source: &str, goal: u32) -> String {
        let (status, out) = call_raw(f, source.as_bytes(), goal);
        assert_eq!(status, TSV_STATUS_ERROR, "expected an error, got: {out}");
        error_message(&out).expect("checked by `call_raw`")
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
        }
    }

    // --- parse_internal: empty payload on success, error status on failure ---

    #[test]
    fn parse_internal_empty_on_success() {
        assert_eq!(call(tsv_parse_internal_typescript, "const x = 1;\n"), "");
        assert_eq!(call(tsv_parse_internal_svelte, "<div>x</div>\n"), "");
        assert_eq!(call(tsv_parse_internal_css, "a {\n\tcolor: red;\n}\n"), "");
    }

    #[test]
    fn parse_internal_reports_errors() {
        // Cover the error arm of the internal-parse exports for all three
        // languages (success arm is covered above for all three). The empty
        // success payload is why this export needs the status word most: it
        // carries no shape a caller could read a verdict off.
        let cases: [(&str, FfiFn, &str); 3] = [
            ("typescript", tsv_parse_internal_typescript, "const ="),
            ("svelte", tsv_parse_internal_svelte, "<div {"),
            ("css", tsv_parse_internal_css, "a {"),
        ];
        for (label, f, src) in cases {
            assert!(
                !call_err(f, src).is_empty(),
                "{label}: expected a non-empty error message"
            );
        }
    }

    // --- the status word is the verdict, not the payload's text ---

    #[test]
    fn success_status_survives_an_error_shaped_payload() {
        // A successful format whose output carries the error envelope's text
        // verbatim. The verdict must still be OK, because it comes from
        // `out_status` and nothing reads the bytes.
        //
        // ⚠️ This is NOT a counterexample to the retired `{"error"` prefix sniff
        // — that sniff would also have called this a success, since the output
        // starts `const`. No counterexample exists to write: see
        // `no_format_output_can_open_the_error_envelope` below for why, and why
        // that is exactly the problem.
        let (status, out) = call_raw(
            tsv_format_typescript,
            br#"const s = "{\"error\": \"not really\"}";"#,
            MODULE,
        );
        assert_eq!(status, TSV_STATUS_OK, "formatting succeeded: {out}");
        assert_eq!(
            out, "const s = '{\"error\": \"not really\"}';\n",
            "the envelope text must survive into the payload verbatim"
        );
    }

    #[test]
    fn no_format_output_can_open_the_error_envelope() {
        // Why the retired sniff was sound, and why soundness on those terms was
        // the reason to retire it.
        //
        // `{"error"` can only OPEN a formatted document through a Svelte mustache
        // (a `{` at TS statement position is a block; CSS has no such form). And
        // a mustache's string comes back single-quoted, because `singleQuote` is
        // on and `error` needs no escaping — so the envelope is unreachable at
        // position 0. That is a correctness property of the FFI error channel
        // resting on a *style* setting, over a channel that otherwise carries
        // arbitrary formatted source. Flip the quote preference and the old sniff
        // starts scoring successful formats as refusals; the status word does not
        // care either way.
        assert_eq!(call(tsv_format_svelte, "{\"error\"}"), "{'error'}\n");
        // The one spelling that keeps its double quotes — a single quote inside
        // the string — cannot spell `error` and so still can't open the envelope.
        assert_eq!(call(tsv_format_svelte, "{\"err'or\"}"), "{\"err'or\"}\n");
    }

    // --- the goal axis: script accepts `await` as an identifier, module rejects ---

    #[test]
    fn typescript_goal_switches_await() {
        // `await` is an ordinary identifier at Script goal, reserved at Module goal.
        let src = "var await = 1;\n";
        for f in [tsv_parse_typescript, tsv_parse_typescript_no_locations] {
            call_goal(f, src, SCRIPT);
            call_err_goal(f, src, MODULE);
        }
        // parse_internal's payload is empty either way — only the status differs.
        assert_eq!(call_goal(tsv_parse_internal_typescript, src, SCRIPT), "");
        call_err_goal(tsv_parse_internal_typescript, src, MODULE);
        // The format twin: the goal shapes the parse the formatter runs.
        assert_eq!(
            call_goal(tsv_format_typescript, "var   await=1", SCRIPT),
            "var await = 1;\n"
        );
        call_err_goal(tsv_format_typescript, src, MODULE);
    }

    /// Every export of one goalless language, so a refusal lost on a single
    /// generated entry point can't hide behind its siblings. CSS has no
    /// `no_locations` binding in the bench harness, but the export exists and
    /// owes the same refusal, so it is driven here.
    ///
    /// Returned as a fresh array per language rather than one flat table because
    /// each export takes its own source.
    fn goalless_exports(language: &str) -> [(&'static str, FfiFn); 4] {
        assert!(language == "svelte" || language == "css", "{language}");
        if language == "svelte" {
            [
                ("parse", tsv_parse_svelte),
                ("parse_no_locations", tsv_parse_svelte_no_locations),
                ("parse_internal", tsv_parse_internal_svelte),
                ("format", tsv_format_svelte),
            ]
        } else {
            [
                ("parse", tsv_parse_css),
                ("parse_no_locations", tsv_parse_css_no_locations),
                ("parse_internal", tsv_parse_internal_css),
                ("format", tsv_format_css),
            ]
        }
    }

    #[test]
    fn unknown_goal_code_is_rejected() {
        // Never a silent Script (or Module) default — and the refusal is the
        // CODE's, not the language's, so a goalless language owes it too (its own
        // `only supported for TypeScript` message covers code 1 alone).
        let ts: [FfiFn; 4] = [
            tsv_parse_typescript,
            tsv_parse_typescript_no_locations,
            tsv_parse_internal_typescript,
            tsv_format_typescript,
        ];
        for f in ts {
            let msg = call_err_goal(f, "var x = 1;\n", 2);
            assert!(msg.contains("invalid goal code 2"), "{msg}");
        }
        for (language, src) in [
            ("svelte", "<div>x</div>\n"),
            ("css", "a {\n\tcolor: red;\n}\n"),
        ] {
            for (op, f) in goalless_exports(language) {
                let msg = call_err_goal(f, src, 2);
                assert!(
                    msg.contains("invalid goal code 2"),
                    "{language} {op}: {msg}"
                );
            }
        }
    }

    #[test]
    fn goalless_languages_reject_a_script_goal() {
        // Svelte hard-wires Module and CSS has no goal, so `1` asks for something
        // that cannot be honored — the caller is told rather than silently served
        // a Module parse. The same stance `tsv_wasm`'s `read_options` takes.
        //
        // Every export, not one per language: each is a separately generated
        // entry point that calls `ffi_goal` on its own line, so a refusal can be
        // lost on exactly one of them.
        for (language, src) in [
            ("svelte", "<div>x</div>\n"),
            ("css", "a {\n\tcolor: red;\n}\n"),
        ] {
            for (op, f) in goalless_exports(language) {
                let msg = call_err_goal(f, src, SCRIPT);
                assert!(
                    msg.contains("only supported for TypeScript"),
                    "{language} {op}: {msg}"
                );
                // Module is the one code they accept.
                call_goal(f, src, MODULE);
            }
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
        call(tsv_parse_typescript, src);
        let formatted = call(tsv_format_typescript, src);
        assert!(
            formatted.contains("€🦀"),
            "multibyte content lost: {formatted}"
        );
        // Re-formatting is stable (idempotent) across the boundary.
        assert_eq!(call(tsv_format_typescript, &formatted), formatted);
    }

    // --- error path: invalid syntax surfaces as an error status (and still frees) ---

    #[test]
    fn invalid_syntax_reports_an_error() {
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
                !call_err(parse_fn, src).is_empty(),
                "{label} parse: expected an error message for {src:?}"
            );
            assert!(
                !call_err(format_fn, src).is_empty(),
                "{label} format: expected an error message for {src:?}"
            );
        }
    }

    // --- invalid UTF-8 is reported, not a crash (module safety contract) ---

    #[test]
    fn invalid_utf8_returns_error() {
        // 0xFF is never valid in UTF-8.
        let (status, out) = call_raw(tsv_format_typescript, &[b'a', 0xFF, b'b'], MODULE);
        assert_eq!(status, TSV_STATUS_ERROR);
        let msg = error_message(&out).expect("checked by `call_raw`");
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
        // Parse of empty input succeeds (a valid root) for every language.
        let parsers: [FfiFn; 3] = [tsv_parse_typescript, tsv_parse_css, tsv_parse_svelte];
        for f in parsers {
            call(f, "");
        }
    }

    // --- null source pointer: (null, 0) is the empty source; (null, n>0) errors ---

    #[test]
    fn null_source_pointer_is_handled() {
        let mut out_len: usize = 0;
        let mut out_status: u32 = u32::MAX;
        // (null, 0) — the empty source, as FFI hosts commonly pass it (e.g.
        // Deno's `UnsafePointer.of` on an empty typed array is null). Formats
        // to empty output, success status.
        // SAFETY: `with_extracted_source` short-circuits before any read.
        let ptr = unsafe {
            tsv_format_typescript(
                std::ptr::null(),
                0,
                MODULE,
                &raw mut out_len,
                &raw mut out_status,
            )
        };
        assert!(!ptr.is_null(), "FFI returned a null pointer");
        let out = unsafe { slice::from_raw_parts(ptr, out_len) };
        assert_eq!(out, b"", "(null, 0) must format as the empty source");
        assert_eq!(out_status, TSV_STATUS_OK);
        unsafe { tsv_free(ptr, out_len) };

        // (null, n>0) — an invalid buffer; must report an error, not UB.
        // Safety: the null check precedes any read of the (bogus) 5 bytes.
        let ptr = unsafe {
            tsv_format_typescript(
                std::ptr::null(),
                5,
                MODULE,
                &raw mut out_len,
                &raw mut out_status,
            )
        };
        assert!(!ptr.is_null(), "FFI returned a null pointer");
        let out = unsafe { slice::from_raw_parts(ptr, out_len) };
        assert_eq!(out_status, TSV_STATUS_ERROR);
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
