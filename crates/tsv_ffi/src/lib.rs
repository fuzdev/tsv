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

use std::slice;

/// Helper to convert source pointer to &str and run a closure, returning serialized JSON.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes.
unsafe fn with_source<F, T>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
    f: F,
) -> *mut u8
where
    F: FnOnce(&str) -> Result<T, String>,
    T: serde::Serialize,
{
    // Safety: Caller guarantees valid UTF-8 pointer and length
    let source = unsafe {
        let bytes = slice::from_raw_parts(source_ptr, source_len);
        match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                return error_result(&format!("Invalid UTF-8: {e}"), out_len);
            }
        }
    };

    match f(source) {
        Ok(result) => {
            let json = match serde_json::to_string(&result) {
                Ok(j) => j,
                Err(e) => {
                    return error_result(&format!("JSON serialization error: {e}"), out_len);
                }
            };
            string_to_ptr(json, out_len)
        }
        Err(e) => error_result(&e, out_len),
    }
}

/// Helper to convert source pointer to &str and run a formatter.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes.
unsafe fn with_source_format<F>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
    f: F,
) -> *mut u8
where
    F: FnOnce(&str) -> Result<String, String>,
{
    // Safety: Caller guarantees valid UTF-8 pointer and length
    let source = unsafe {
        let bytes = slice::from_raw_parts(source_ptr, source_len);
        match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                return error_result(&format!("Invalid UTF-8: {e}"), out_len);
            }
        }
    };

    match f(source) {
        Ok(result) => string_to_ptr(result, out_len),
        Err(e) => error_result(&e, out_len),
    }
}

/// Helper for internal parse (no conversion, no JSON serialization).
/// Returns empty string on success, error JSON on failure.
///
/// Uses `std::hint::black_box` to prevent the compiler from optimizing away
/// the parse when the AST result is unused.
///
/// # Safety
/// Caller must ensure `source_ptr` points to valid UTF-8 of `source_len` bytes.
unsafe fn with_source_parse_internal<F, T>(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
    f: F,
) -> *mut u8
where
    F: FnOnce(&str) -> Result<T, String>,
{
    // Safety: Caller guarantees valid UTF-8 pointer and length
    let source = unsafe {
        let bytes = slice::from_raw_parts(source_ptr, source_len);
        match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                return error_result(&format!("Invalid UTF-8: {e}"), out_len);
            }
        }
    };

    match f(source) {
        Ok(ast) => {
            // Prevent compiler from optimizing away the parse
            std::hint::black_box(ast);
            string_to_ptr(String::new(), out_len) // Success: empty string
        }
        Err(e) => error_result(&e, out_len),
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

//
// Svelte
//

/// Parse Svelte source code and return JSON AST.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_svelte(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source(source_ptr, source_len, out_len, |source| {
            let ast = tsv_svelte::parse(source).map_err(|e| e.to_string())?;
            Ok(tsv_svelte::convert_ast(&ast, source))
        })
    }
}

/// Parse Svelte source to internal AST only (no conversion, no serialization).
/// Returns empty string on success for minimal overhead benchmarking.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_internal_svelte(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_parse_internal(source_ptr, source_len, out_len, |source| {
            tsv_svelte::parse(source).map_err(|e| e.to_string())
        })
    }
}

/// Format Svelte source code.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_format_svelte(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_format(source_ptr, source_len, out_len, |source| {
            let ast = tsv_svelte::parse(source).map_err(|e| e.to_string())?;
            Ok(tsv_svelte::format(&ast, source))
        })
    }
}

//
// TypeScript
//

/// Parse TypeScript source code and return JSON AST.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_typescript(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source(source_ptr, source_len, out_len, |source| {
            let ast = tsv_ts::parse(source).map_err(|e| e.to_string())?;
            Ok(tsv_ts::convert_ast(&ast, source))
        })
    }
}

/// Parse TypeScript source to internal AST only (no conversion, no serialization).
/// Returns empty string on success for minimal overhead benchmarking.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_internal_typescript(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_parse_internal(source_ptr, source_len, out_len, |source| {
            tsv_ts::parse(source).map_err(|e| e.to_string())
        })
    }
}

/// Format TypeScript source code.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_format_typescript(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_format(source_ptr, source_len, out_len, |source| {
            let ast = tsv_ts::parse(source).map_err(|e| e.to_string())?;
            Ok(tsv_ts::format(&ast, source))
        })
    }
}

//
// CSS
//

/// Parse CSS source code and return JSON AST.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_css(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source(source_ptr, source_len, out_len, |source| {
            let ast = tsv_css::parse(source).map_err(|e| e.to_string())?;
            Ok(tsv_css::convert_ast(&ast, source))
        })
    }
}

/// Parse CSS source to internal AST only (no conversion, no serialization).
/// Returns empty string on success for minimal overhead benchmarking.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_parse_internal_css(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_parse_internal(source_ptr, source_len, out_len, |source| {
            tsv_css::parse(source).map_err(|e| e.to_string())
        })
    }
}

/// Format CSS source code.
///
/// # Safety
/// - `source_ptr` must point to valid UTF-8 data of `source_len` bytes
/// - `out_len` must point to a valid `usize` for writing output length
/// - Caller must free returned pointer via `tsv_free(ptr, *out_len)`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsv_format_css(
    source_ptr: *const u8,
    source_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    unsafe {
        with_source_format(source_ptr, source_len, out_len, |source| {
            let ast = tsv_css::parse(source).map_err(|e| e.to_string())?;
            Ok(tsv_css::format(&ast, source))
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
            drop(Box::from_raw(slice::from_raw_parts_mut(ptr, len)));
        }
    }
}
