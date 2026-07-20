//! Shared assertion helpers for the compiler test suite.
//!
//! One helper per intent, and reaching for an existing one is the rule — a second
//! spelling of the same assertion is how this suite drifted before, and a weaker
//! spelling is worse than a second one: an assertion that only checks a shape
//! refuses *somehow* passes when it refuses for the wrong reason.
//!
//! - `compile_js` — compiles. Asserts server output AND a canonicalize fixed
//!   point, returning the JS so a caller can assert its shape; a bare
//!   `let _ = compile_js(src)` is the "this shape compiles" pin.
//! - `compile_css` — the scoped CSS of a component that compiles.
//! - `assert_unsupported` — refuses, pinning WHICH refusal by a substring of
//!   `Refusal`'s `Display`.
//! - `assert_parse_rejected` — fails at the parse stage.
//! - `assert_idempotent` / `assert_comments_lossless` — the canonicalizer's own
//!   contract, independent of compilation.

use crate::*;

/// Canonicalize twice and assert the result is a fixed point.
pub(super) fn assert_idempotent(source: &str) -> String {
    let once = canonicalize_js(source).expect("first canonicalize");
    let twice = canonicalize_js(&once).expect("second canonicalize");
    assert_eq!(
        once, twice,
        "canonicalize_js must be idempotent for:\n{source}"
    );
    once
}

/// Losslessness assertions for a canonicalize run over a source carrying the
/// given comment texts: idempotent output, each comment present exactly once,
/// original relative order preserved.
pub(super) fn assert_comments_lossless(source: &str, comments: &[&str]) -> String {
    let out = assert_idempotent(source);
    let mut prev_pos = 0;
    for comment in comments {
        let pos = out
            .find(comment)
            .unwrap_or_else(|| panic!("comment {comment:?} lost:\n{out}"));
        assert_eq!(
            out.matches(comment).count(),
            1,
            "comment {comment:?} duplicated:\n{out}"
        );
        assert!(
            pos >= prev_pos,
            "comment {comment:?} reordered (found at {pos}, previous comment ends at {prev_pos}):\n{out}"
        );
        prev_pos = pos + comment.len();
    }
    out
}

/// Compile `source`, asserting the acceptance contract every accepting test
/// relies on: the output is server output (`$$renderer` present) and its JS is a
/// canonicalize fixed point (every block emitter prints through
/// `format_canonical`, so this must hold).
///
/// The single place that contract lives — `compile_js` and `compile_css` are
/// views onto it, so a test that only reads one output still pins both.
pub(super) fn compile_checked(source: &str) -> CompileOutput {
    let out = compile(source, &CompileOptions::default())
        .unwrap_or_else(|e| panic!("compile failed for {source:?}: {e:?}"));
    assert!(
        out.js.contains("$$renderer"),
        "expected server output for:\n{source}\ngot:\n{}",
        out.js
    );
    assert_eq!(
        canonicalize_js(&out.js).unwrap(),
        out.js,
        "block output must be a canonicalize fixed point:\n{}",
        out.js
    );
    out
}

/// The generated JS of a component that compiles. A bare
/// `let _ = compile_js(src)` is the "this shape compiles" pin; the returned JS
/// carries the shape assertions.
pub(super) fn compile_js(source: &str) -> String {
    compile_checked(source).js
}

/// The scoped CSS a component compiles to (panicking if it declines).
pub(super) fn compile_css(source: &str) -> String {
    compile_checked(source)
        .css
        .unwrap_or_else(|| panic!("expected scoped css for {source:?}"))
}

/// Assert `compile` refuses with an `Unsupported` message containing `what`.
pub(super) fn assert_unsupported(source: &str, what: &str) {
    let err = compile(source, &CompileOptions::default()).unwrap_err();
    assert!(
        matches!(&err, CompileError::Unsupported(reason) if reason.to_string().contains(what)),
        "expected Unsupported({what}), got {err:?} for:\n{source}"
    );
}

/// Assert `compile` fails at the parse stage with a message containing `what`.
pub(super) fn assert_parse_rejected(source: &str, what: &str) {
    let err = compile(source, &CompileOptions::default()).unwrap_err();
    assert!(
        matches!(&err, CompileError::Parse(e) if e.to_string().contains(what)),
        "expected Parse({what}), got {err:?} for:\n{source}"
    );
}
