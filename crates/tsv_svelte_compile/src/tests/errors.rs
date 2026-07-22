//! Error surfacing: parse errors and the output self-check.

use crate::*;

#[test]
fn compile_surfaces_parse_errors() {
    let err = compile("<script>const x = ;</script>", &CompileOptions::default()).unwrap_err();
    assert!(
        matches!(err, CompileError::Parse(_)),
        "expected Parse, got {err:?}"
    );
}

#[test]
fn canonicalize_surfaces_parse_errors() {
    let err = canonicalize_js("const x = ;").unwrap_err();
    assert!(
        matches!(err, CanonicalizeError::Parse(_)),
        "expected Parse, got {err:?}"
    );
}

#[test]
fn validate_output_js_rejects_corrupt_output_loudly() {
    // The self-validation seam: hypothetical corrupt generated JS (the
    // divergent-shape-slipped-every-guard class, e.g. a nested `export`)
    // must surface as CorruptOutput, not as a silently invalid module.
    // Note the net's reach: it catches output the parser REJECTS; output
    // that parses as TypeScript (a passed-through type annotation) is not
    // a parse rejection and is caught at parity-comparison time instead.
    for corrupt in [
        // Invalid nesting the transform must never emit.
        "export default function Input($$renderer) {\n\texport const a = 1;\n}\n",
        // A hard syntax error.
        "export default function Input($$renderer) {\n\tconst x = ;\n}\n",
    ] {
        let err = validate_output_js(corrupt).unwrap_err();
        assert!(
            matches!(err, CompileError::CorruptOutput(_)),
            "expected CorruptOutput for {corrupt:?}, got {err:?}"
        );
    }
    // Valid generated-shaped JS passes.
    validate_output_js(
            "import * as $ from 'svelte/internal/server';\nexport default function Input($$renderer) {\n\t$$renderer.push(`<p>x</p>`);\n}\n",
        )
        .expect("valid output must validate");
}
