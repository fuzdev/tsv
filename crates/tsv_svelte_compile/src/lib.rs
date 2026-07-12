//! Svelte-to-JS compiler and JavaScript canonicalizer.
//!
//! This crate compiles Svelte components to JavaScript, pinned to Svelte's own
//! `compile()` as the correctness oracle. Parity is judged not on raw output
//! bytes but on the *canonical reprint* of both sides: [`canonicalize_js`] parses
//! JavaScript and reprints it with newline-derived authoring intent erased, so a
//! diff between two canonical forms reflects only a real code difference, never
//! incidental whitespace.
//!
//! The compiler itself is a walking skeleton for this slice: [`compile`] parses
//! the component (so genuine parse errors surface) and then reports that code
//! generation is not yet implemented. The canonicalizer is complete and is the
//! comparison substrate the compiler will be measured against.

use tsv_ts::Goal;

/// Which runtime the compiler targets.
///
/// Mirrors Svelte's `generate` option. Defaults to [`Generate::Server`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Generate {
    /// Server-side rendering output (the default).
    #[default]
    Server,
    /// Client-side output.
    Client,
}

/// Options controlling a [`compile`] run.
///
/// Defaults to server generation, non-development output — matching the
/// deterministic oracle configuration used for parity comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompileOptions {
    /// Target runtime.
    pub generate: Generate,
    /// Development-mode output (extra runtime checks / metadata).
    pub dev: bool,
}

/// A diagnostic emitted during compilation.
///
/// Minimal for this slice — a stable code and a human-readable message. It grows
/// as the compiler produces real warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileWarning {
    /// Stable warning code (e.g. an `a11y-*` identifier).
    pub code: String,
    /// Human-readable description.
    pub message: String,
}

/// The product of a successful [`compile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    /// The generated JavaScript module.
    pub js: String,
    /// The extracted, scoped CSS, if the component had a `<style>`.
    pub css: Option<String>,
    /// Warnings produced during compilation.
    pub warnings: Vec<CompileWarning>,
}

/// An error from [`compile`].
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// The component failed to parse (a real syntax error in the `.svelte`
    /// source, its `<script>`, or its `<style>`).
    #[error("failed to parse Svelte component: {0}")]
    Parse(#[from] tsv_lang::ParseError),
    /// The component parsed, but code generation is not yet implemented.
    #[error("Svelte code generation is not yet implemented")]
    Codegen,
}

/// An error from [`canonicalize_js`].
#[derive(Debug, thiserror::Error)]
pub enum CanonicalizeError {
    /// The input did not parse as a JavaScript/TypeScript module.
    #[error("failed to parse JavaScript for canonicalization: {0}")]
    Parse(#[from] tsv_lang::ParseError),
}

/// Compile a Svelte component to JavaScript.
///
/// Parses `source` (surfacing any real parse error as [`CompileError::Parse`]);
/// code generation is not yet implemented, so a well-formed component currently
/// returns [`CompileError::Codegen`]. The walking skeleton makes this real.
pub fn compile(source: &str, options: &CompileOptions) -> Result<CompileOutput, CompileError> {
    // Reserved for the code-generation phase; validated here so the signature is
    // stable while the skeleton fills in.
    let _ = options;
    let arena = bumpalo::Bump::new();
    let _root = tsv_svelte::parse(source, &arena)?;
    Err(CompileError::Codegen)
}

/// Reprint JavaScript with newline-derived authoring intent erased — the
/// canonical form used for parity comparison.
///
/// Parses `source` as a strict module ([`Goal::Module`]) and reprints it via
/// `tsv_ts`'s canonical formatter, which:
///
/// - **drops blank lines** between statements,
/// - **turns off expansion heuristics** — a construct that fits the print width
///   collapses to one line regardless of whether the source had a newline after
///   its opening delimiter; it breaks only when width forces it,
/// - **preserves comments** (content and relative order) — placement is
///   normalized deterministically (an own-line comment may become a trailing
///   comment of the preceding node), never dropped or merged.
///
/// The result is idempotent: canonicalizing an already-canonical string
/// reproduces it. Because both an oracle's output and the compiler's output pass
/// through the same normalization, a byte difference between their canonical
/// forms is a genuine code difference.
pub fn canonicalize_js(source: &str) -> Result<String, CanonicalizeError> {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse_with_goal(source, Goal::Module, &arena)?;
    Ok(tsv_ts::format_canonical(&program, source))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonicalize twice and assert the result is a fixed point.
    fn assert_idempotent(source: &str) -> String {
        let once = canonicalize_js(source).expect("first canonicalize");
        let twice = canonicalize_js(&once).expect("second canonicalize");
        assert_eq!(once, twice, "canonicalize_js must be idempotent for:\n{source}");
        once
    }

    #[test]
    fn multiline_but_fitting_object_collapses() {
        // A short object authored expanded and the same object authored inline
        // must reach the SAME canonical form (expansion intent erased).
        let expanded = canonicalize_js("const x = {\n\ta: 1,\n\tb: 2\n};\n").unwrap();
        let inline = canonicalize_js("const x = {a: 1, b: 2};\n").unwrap();
        assert_eq!(expanded, inline, "multiline-but-fitting object must collapse");
        assert!(!expanded.contains("a: 1,\n"), "should be single-line: {expanded:?}");
    }

    #[test]
    fn blank_lines_are_dropped() {
        let with_blanks = canonicalize_js("const a = 1;\n\n\nconst b = 2;\n").unwrap();
        let without = canonicalize_js("const a = 1;\nconst b = 2;\n").unwrap();
        assert_eq!(with_blanks, without, "blank lines must be erased");
        assert!(!with_blanks.contains("\n\n"), "no blank line survives: {with_blanks:?}");
    }

    #[test]
    fn over_width_construct_still_breaks() {
        // An object whose inline form exceeds the 100-col print width must break,
        // and both authorings (inline vs expanded) canonicalize identically.
        let long = "const config = {alpha: 1, bravo: 2, charlie: 3, delta: 4, echo: 5, \
                     foxtrot: 6, golf: 7, hotel: 8};\n";
        let inline = canonicalize_js(long).unwrap();
        assert!(inline.contains('\n'), "over-width object must break across lines");
        // Same content, authored expanded, reaches the same canonical form.
        let expanded = canonicalize_js(
            "const config = {\n\talpha: 1,\n\tbravo: 2,\n\tcharlie: 3,\n\tdelta: 4,\n\techo: 5,\n\
             \tfoxtrot: 6,\n\tgolf: 7,\n\thotel: 8\n};\n",
        )
        .unwrap();
        assert_eq!(inline, expanded, "width-broken forms must be authoring-independent");
    }

    #[test]
    fn trailing_comment_survives() {
        let out = canonicalize_js("const x = 1; // keep me\n").unwrap();
        assert!(out.contains("// keep me"), "trailing comment lost: {out:?}");
    }

    #[test]
    fn leading_comment_survives() {
        let out = canonicalize_js("// heading\nconst x = 1;\n").unwrap();
        assert!(out.contains("// heading"), "leading comment lost: {out:?}");
    }

    #[test]
    fn consecutive_line_comments_do_not_merge() {
        // The losslessness edge case: two own-line line comments must stay on two
        // lines (never merge onto one, which would swallow the second `//`).
        let out = canonicalize_js("// first\n// second\nconst x = 1;\n").unwrap();
        assert!(out.contains("// first"), "first comment lost: {out:?}");
        assert!(out.contains("// second"), "second comment lost: {out:?}");
        // "// first // second" on one line would be the merge bug.
        assert!(!out.contains("// first // second"), "comments merged: {out:?}");
    }

    #[test]
    fn block_comment_survives() {
        let out = canonicalize_js("const x = /* inline */ 1;\n").unwrap();
        assert!(out.contains("/* inline */"), "block comment lost: {out:?}");
    }

    #[test]
    fn idempotent_on_samples() {
        assert_idempotent("const x = {\n\ta: 1\n};\n");
        assert_idempotent("const a = 1;\n\nconst b = 2;\n");
        assert_idempotent("// lead\nexport function f(x) {\n\treturn x + 1;\n}\n");
        assert_idempotent("import {a, b} from 'mod';\nconst t = `line\nbreak`;\n");
        assert_idempotent("const x = 1; // trailing\n// own line\nconst y = 2;\n");
    }

    #[test]
    fn template_literal_newline_is_content_not_intent() {
        // A real newline inside a template literal is content, not layout intent —
        // it must survive canonicalization verbatim.
        let out = canonicalize_js("const t = `a\nb`;\n").unwrap();
        assert!(out.contains("`a\nb`"), "template literal newline not preserved: {out:?}");
    }

    #[test]
    fn compile_reports_unimplemented_codegen() {
        let err = compile("<div>hi</div>", &CompileOptions::default()).unwrap_err();
        assert!(matches!(err, CompileError::Codegen), "expected Codegen, got {err:?}");
    }

    #[test]
    fn compile_surfaces_parse_errors() {
        let err = compile("<script>const x = ;</script>", &CompileOptions::default()).unwrap_err();
        assert!(matches!(err, CompileError::Parse(_)), "expected Parse, got {err:?}");
    }

    #[test]
    fn canonicalize_surfaces_parse_errors() {
        let err = canonicalize_js("const x = ;").unwrap_err();
        assert!(matches!(err, CanonicalizeError::Parse(_)), "expected Parse, got {err:?}");
    }
}
