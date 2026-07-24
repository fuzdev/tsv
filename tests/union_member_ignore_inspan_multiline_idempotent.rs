// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![allow(clippy::expect_used)]

//! An own-line `format-ignore` directive that sits in a union's IN-SPAN leading gap
//! (after a leading `|`, before a MULTI-LINE first member) freezes that member and is
//! re-emitted own-line BEFORE the `|` — never relocated to trail it.
//!
//! Regression this guards (the FIX 3 non-idempotency): the printer used to emit a frozen
//! first member's leading run AFTER the `| ` prefix, relocating the directive to trail the
//! pipe (`| // prettier-ignore`). On the next pass that trailing directive is no longer
//! own-line, so the freeze is lost and the member reformats — an F1 fixed-point violation.
//! The fix emits the run before the `| ` in both the line-comment path (own-line line
//! directive) and the main loop (own-line block directive).
//!
//! Not a fixture: the in-span form is not tsv-stable (tsv normalizes the directive to the
//! leading-run position, which the `union_prettier_ignore_multiline_member` fixture already
//! pins) and the pipe/directive reorder is a token move that the `unformatted_*` rules
//! exclude. So the idempotency of the NORMALIZED output is pinned here directly.

fn format(source: &str) -> String {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse(source, &arena).expect("parse failed");
    tsv_ts::format(&program, source)
}

/// Line directive after a leading `|`, before a multi-line object member. The `a:  1`
/// double space is inside the frozen slice, so it survives verbatim.
const INSPAN_LINE: &str = "type U =\n\t|\n\t// prettier-ignore\n\t{\n\t\ta:  1\n\t} | b;\n";

/// Block-directive analog (routes through the main loop rather than the line-comment path).
const INSPAN_BLOCK: &str = "type U =\n\t|\n\t/* prettier-ignore */\n\t{\n\t\ta:  1\n\t} | b;\n";

/// The normalized output is a fixed point: pass 2 equals pass 1 byte-for-byte, and the
/// directive + the frozen member's verbatim interior both survive.
fn assert_inspan_multiline_stable(input: &str, directive: &str) {
    let pass1 = format(input);
    let pass2 = format(&pass1);
    assert_eq!(
        pass1, pass2,
        "in-span multiline freeze must reach a fixed point (no directive relocation):\n{pass1:?}"
    );
    assert!(
        pass1.contains(directive),
        "the directive must survive in the output: {pass1:?}"
    );
    assert!(
        pass1.contains("a:  1"),
        "the frozen member's verbatim interior must survive: {pass1:?}"
    );
    // The directive must be own-line before the `|`, never trailing it (the relocation
    // that broke idempotency).
    assert!(
        !pass1.contains(&format!("| {directive}")),
        "the directive must not be relocated to trail the pipe: {pass1:?}"
    );
}

#[test]
fn inspan_line_directive_multiline_member_is_idempotent() {
    assert_inspan_multiline_stable(INSPAN_LINE, "// prettier-ignore");
}

#[test]
fn inspan_block_directive_multiline_member_is_idempotent() {
    assert_inspan_multiline_stable(INSPAN_BLOCK, "/* prettier-ignore */");
}
