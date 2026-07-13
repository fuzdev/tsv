//! Svelte-to-JS compiler and JavaScript canonicalizer.
//!
//! This crate compiles Svelte components to JavaScript, pinned to Svelte's own
//! `compile()` as the correctness oracle. Parity is judged not on raw output
//! bytes but on the *canonical reprint* of both sides: [`canonicalize_js`] parses
//! JavaScript and reprints it with newline-derived authoring intent erased, so a
//! diff between two canonical forms reflects only a real code difference, never
//! incidental whitespace.
//!
//! [`compile`] generates server (SSR) output by constructing a synthetic
//! `tsv_ts` AST over the hybrid appendix buffer (`build`) and printing it
//! through `tsv_ts::format_canonical` — generated JS is canonical-form by
//! construction, so the parity comparison verifies rather than transforms it.
//! The server transform (`transform_server`) covers a deliberately small
//! language subset today; unhandled shapes surface as
//! [`CompileError::Unsupported`] rather than guessed output.

mod analyze;
mod build;
mod needs_context;
mod rune_guard;
mod transform_server;

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
    /// The component parsed, but uses a shape the compiler does not cover yet.
    /// Always a clear refusal — never guessed output.
    #[error("not yet supported by the Svelte compiler: {0}")]
    Unsupported(String),
    /// The generated JS failed to reparse — a divergent shape slipped every
    /// guard and the transform emitted invalid JavaScript. Always a compiler
    /// bug; surfaced loudly instead of returning the corrupt module (the same
    /// contract as [`CanonicalizeError::CorruptOutput`]).
    #[error("generated JS failed to reparse (compiler bug): {0}")]
    CorruptOutput(tsv_lang::ParseError),
}

/// An error from [`canonicalize_js`].
#[derive(Debug, thiserror::Error)]
pub enum CanonicalizeError {
    /// The input did not parse as a JavaScript/TypeScript module.
    #[error("failed to parse JavaScript for canonicalization: {0}")]
    Parse(#[from] tsv_lang::ParseError),
    /// The canonical reprint itself failed to reparse — the canonicalizer
    /// corrupted the program (e.g. content trailed onto a `//` comment's line).
    /// Always a canonicalizer bug; surfaced loudly instead of returning the
    /// corrupt string.
    #[error("canonical output failed to reparse (canonicalizer bug): {0}")]
    CorruptOutput(tsv_lang::ParseError),
}

/// Compile a Svelte component to JavaScript.
///
/// Parses `source` (surfacing any real parse error as [`CompileError::Parse`])
/// and runs the server transform. The generated JS is already in canonical form
/// (it prints through `tsv_ts::format_canonical`), so
/// `canonicalize_js(output.js)` is a fixed point. Client generation and dev
/// mode are not implemented yet ([`CompileError::Unsupported`]).
///
/// The output is self-validated by reparse before it is returned — generated JS
/// the parser rejects surfaces as [`CompileError::CorruptOutput`] instead of a
/// silently invalid module. Always on: the reparse costs ~13% of the compile
/// itself (microseconds per component), cheap insurance for a dev-stage
/// compiler whose refusal contract depends on never shipping guessed output.
pub fn compile(source: &str, options: &CompileOptions) -> Result<CompileOutput, CompileError> {
    if options.generate == Generate::Client {
        return Err(CompileError::Unsupported("client generation".to_string()));
    }
    if options.dev {
        return Err(CompileError::Unsupported("dev mode output".to_string()));
    }
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena)?;
    let output = transform_server::compile_server(&root, source, &arena)?;
    validate_output_js(&output.js)?;
    Ok(output)
}

/// The self-validation seam: assert `js` reparses as a strict module.
///
/// Split from [`compile`] so the corrupt-output path is unit-testable without
/// weakening the public API (no test-only hooks on `compile` itself).
fn validate_output_js(js: &str) -> Result<(), CompileError> {
    let arena = bumpalo::Bump::new();
    match tsv_ts::parse_with_goal(js, Goal::Module, &arena) {
        Ok(_) => Ok(()),
        Err(err) => Err(CompileError::CorruptOutput(err)),
    }
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
///
/// The output is self-validated by reparse before it is returned — a reprint the
/// parser rejects (canonicalizer corruption) surfaces as
/// [`CanonicalizeError::CorruptOutput`] instead of a silently broken string.
/// This is a comparison harness, so the extra parse is cheap insurance.
pub fn canonicalize_js(source: &str) -> Result<String, CanonicalizeError> {
    let arena = bumpalo::Bump::new();
    let program = tsv_ts::parse_with_goal(source, Goal::Module, &arena)?;
    let output = tsv_ts::format_canonical(&program, source);
    let check_arena = bumpalo::Bump::new();
    if let Err(err) = tsv_ts::parse_with_goal(&output, Goal::Module, &check_arena) {
        return Err(CanonicalizeError::CorruptOutput(err));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonicalize twice and assert the result is a fixed point.
    fn assert_idempotent(source: &str) -> String {
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
    fn assert_comments_lossless(source: &str, comments: &[&str]) -> String {
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

    #[test]
    fn multiline_but_fitting_object_collapses() {
        // A short object authored expanded and the same object authored inline
        // must reach the SAME canonical form (expansion intent erased).
        let expanded = canonicalize_js("const x = {\n\ta: 1,\n\tb: 2\n};\n").unwrap();
        let inline = canonicalize_js("const x = {a: 1, b: 2};\n").unwrap();
        assert_eq!(
            expanded, inline,
            "multiline-but-fitting object must collapse"
        );
        assert!(
            !expanded.contains("a: 1,\n"),
            "should be single-line: {expanded:?}"
        );
    }

    #[test]
    fn blank_lines_are_dropped() {
        let with_blanks = canonicalize_js("const a = 1;\n\n\nconst b = 2;\n").unwrap();
        let without = canonicalize_js("const a = 1;\nconst b = 2;\n").unwrap();
        assert_eq!(with_blanks, without, "blank lines must be erased");
        assert!(
            !with_blanks.contains("\n\n"),
            "no blank line survives: {with_blanks:?}"
        );
    }

    #[test]
    fn over_width_construct_still_breaks() {
        // An object whose inline form exceeds the 100-col print width must break,
        // and both authorings (inline vs expanded) canonicalize identically.
        let long = "const config = {alpha: 1, bravo: 2, charlie: 3, delta: 4, echo: 5, \
                     foxtrot: 6, golf: 7, hotel: 8};\n";
        let inline = canonicalize_js(long).unwrap();
        assert!(
            inline.contains('\n'),
            "over-width object must break across lines"
        );
        // Same content, authored expanded, reaches the same canonical form.
        let expanded = canonicalize_js(
            "const config = {\n\talpha: 1,\n\tbravo: 2,\n\tcharlie: 3,\n\tdelta: 4,\n\techo: 5,\n\
             \tfoxtrot: 6,\n\tgolf: 7,\n\thotel: 8\n};\n",
        )
        .unwrap();
        assert_eq!(
            inline, expanded,
            "width-broken forms must be authoring-independent"
        );
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
        assert!(
            !out.contains("// first // second"),
            "comments merged: {out:?}"
        );
    }

    #[test]
    fn template_interpolation_chain_trailing_comment_stays_valid() {
        // D1: a `+` chain inside a template interpolation with an operand-trailing
        // `//` comment. Collapsing would trail the comment inside `${...}` and
        // swallow the closer (`${x + y // c})z`), making the output unparseable —
        // the chain must stay broken so the comment ends at a real line end.
        let out = assert_comments_lossless("const r = `(${x + // c\n\ty})z`;\n", &["// c"]);
        // The output must reparse (canonicalize_js validates this itself, but pin
        // the invariant explicitly at the test level too).
        canonicalize_js(&out).expect("D1 output must reparse");
    }

    #[test]
    fn binary_chain_multiple_trailing_comments_do_not_merge() {
        // D2 (`+` chain): two operand-trailing comments must not merge onto one
        // trailing line (which also reorders them: `a + b + c; // two // one`).
        assert_comments_lossless(
            "const q = a + // one\n\tb + // two\n\tc;\n",
            &["// one", "// two"],
        );
    }

    #[test]
    fn logical_chain_multiple_trailing_comments_do_not_merge() {
        // D2 (`||` chain): same class through the logical-expression path.
        assert_comments_lossless(
            "const ok = first || // one\n\tsecond || // two\n\tthird;\n",
            &["// one", "// two"],
        );
    }

    #[test]
    fn chain_with_trailing_comments_as_call_arg_stays_lossless() {
        // Not-statement-final variant: the commented chain is a call argument, so
        // there is no statement end for a trailing comment to legally land on.
        assert_comments_lossless("f(a + // one\n\tb + // two\n\tc);\n", &["// one", "// two"]);
    }

    #[test]
    fn chain_with_trailing_comments_as_array_element_stays_lossless() {
        // Not-statement-final variant: the commented chain is an array element
        // followed by another element — trailing past the `,` must not swallow it.
        assert_comments_lossless(
            "const xs = [a + // one\n\tb, // two\n\tc];\n",
            &["// one", "// two"],
        );
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
        assert!(
            out.contains("`a\nb`"),
            "template literal newline not preserved: {out:?}"
        );
    }

    #[test]
    fn compile_static_element() {
        let out = compile("<p>text</p>", &CompileOptions::default()).unwrap();
        assert_eq!(
            out.js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \t$$renderer.push(`<p>text</p>`);\n\
             }\n"
        );
        assert!(out.css.is_none(), "unstyled component has no css");
        // Generated output is canonical-form by construction (a fixed point).
        assert_eq!(canonicalize_js(&out.js).unwrap(), out.js);
    }

    #[test]
    fn compile_props_and_interpolation() {
        let out = compile(
            "<script>\n\tlet { prop } = $props();\n</script>\n\n<p>{prop}</p>\n",
            &CompileOptions::default(),
        )
        .unwrap();
        assert_eq!(
            out.js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { prop } = $$props;\n\
             \t$$renderer.push(`<p>${$.escape(prop)}</p>`);\n\
             }\n"
        );
        assert_eq!(canonicalize_js(&out.js).unwrap(), out.js);
    }

    #[test]
    fn compile_template_escapes_backtick_and_backslash() {
        // Static text containing template-literal metacharacters must be escaped
        // in the minted quasi so the output reparses to the same text. (`${` can't
        // appear as static Svelte text — `{` opens an expression tag — so the
        // template-escape cases reachable from a component are backtick/backslash.)
        let out = compile("<p>a`b\\c</p>", &CompileOptions::default()).unwrap();
        assert!(
            out.js.contains("`<p>a\\`b\\\\c</p>`"),
            "template metachars must be escaped: {}",
            out.js
        );
        assert_eq!(canonicalize_js(&out.js).unwrap(), out.js);
    }

    /// Compile `source` and return the generated JS, asserting it is a
    /// canonicalize fixed point (every block emitter prints through
    /// `format_canonical`, so this must hold).
    fn compile_js(source: &str) -> String {
        let out = compile(source, &CompileOptions::default())
            .unwrap_or_else(|e| panic!("compile failed for {source:?}: {e:?}"));
        assert_eq!(
            canonicalize_js(&out.js).unwrap(),
            out.js,
            "block output must be a canonicalize fixed point:\n{}",
            out.js
        );
        out.js
    }

    #[test]
    fn compile_if_else_block() {
        // Branch anchors are single-quoted string pushes; the closer `<!--]-->`
        // is its own template push. A missing branch synthesizes nothing here.
        let js = compile_js("{#if a}<p>1</p>{:else}<p>2</p>{/if}");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tif (a) {\n\
             \t\t$$renderer.push('<!--[0-->');\n\
             \t\t$$renderer.push(`<p>1</p>`);\n\
             \t} else {\n\
             \t\t$$renderer.push('<!--[-1-->');\n\
             \t\t$$renderer.push(`<p>2</p>`);\n\
             \t}\n\
             \t$$renderer.push(`<!--]-->`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_if_synthesizes_missing_else() {
        // No `{:else}` → an anchor-only `else` branch with `<!--[-1-->`.
        let js = compile_js("{#if a}<p>1</p>{/if}");
        assert!(
            js.contains("} else {\n\t\t$$renderer.push('<!--[-1-->');\n\t}"),
            "missing else must be synthesized: {js}"
        );
    }

    #[test]
    fn compile_else_if_chain_numbers_branches() {
        // Consequents number 0,1,…; the terminal else is -1; `else if` nests.
        let js = compile_js("{#if a}<p>1</p>{:else if b}<p>2</p>{:else}<p>3</p>{/if}");
        assert!(js.contains("if (a) {"), "{js}");
        assert!(js.contains("} else if (b) {"), "{js}");
        assert!(js.contains("$$renderer.push('<!--[0-->');"), "{js}");
        assert!(js.contains("$$renderer.push('<!--[1-->');"), "{js}");
        assert!(js.contains("$$renderer.push('<!--[-1-->');"), "{js}");
    }

    #[test]
    fn compile_each_block() {
        let js = compile_js(
            "<script>let { items } = $props();</script>\n{#each items as item}<li>{item}</li>{/each}",
        );
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { items } = $$props;\n\
             \t$$renderer.push(`<!--[-->`);\n\
             \tconst each_array = $.ensure_array_like(items);\n\
             \tfor (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {\n\
             \t\tlet item = each_array[$$index];\n\
             \t\t$$renderer.push(`<li>${$.escape(item)}</li>`);\n\
             \t}\n\
             \t$$renderer.push(`<!--]-->`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_each_with_else_hoists_and_uses_authored_index() {
        // `{:else}` hoists `each_array` before an `if (…length !== 0)`; the
        // authored index name replaces `$$index` everywhere.
        let js = compile_js(
            "<script>let { items } = $props();</script>\n{#each items as item, i}<li>{i}</li>{:else}<p>none</p>{/each}",
        );
        assert!(
            js.contains(
                "const each_array = $.ensure_array_like(items);\n\tif (each_array.length !== 0) {"
            ),
            "each_array must hoist before the if: {js}"
        );
        assert!(js.contains("$$renderer.push('<!--[-->');"), "{js}");
        assert!(js.contains("$$renderer.push('<!--[!-->');"), "{js}");
        assert!(
            js.contains("for (let i = 0, $$length = each_array.length; i < $$length; i++) {"),
            "authored index must replace $$index: {js}"
        );
    }

    #[test]
    fn compile_sibling_each_blocks_number_names() {
        // Sibling eachs get suffixed names in source order.
        let js = compile_js(
            "<script>let { a, b } = $props();</script>\n{#each a as x}<p>{x}</p>{/each}{#each b as y}<p>{y}</p>{/each}",
        );
        assert!(
            js.contains("const each_array = $.ensure_array_like(a);"),
            "{js}"
        );
        assert!(
            js.contains("const each_array_1 = $.ensure_array_like(b);"),
            "second each must be each_array_1: {js}"
        );
        assert!(js.contains("let x = each_array[$$index];"), "{js}");
        assert!(js.contains("let y = each_array_1[$$index_1];"), "{js}");
    }

    #[test]
    fn compile_await_block_drops_catch() {
        // Always 4-arg `$.await`; the `{:catch}` branch is dropped entirely.
        let js = compile_js(
            "<script>let { p } = $props();</script>\n{#await p}<p>load</p>{:then v}<p>{v}</p>{:catch e}<p>err</p>{/await}",
        );
        assert!(js.contains("$.await("), "{js}");
        assert!(
            js.contains("(value) => {") || js.contains("(v) => {"),
            "then param: {js}"
        );
        assert!(js.contains("`<p>load</p>`"), "{js}");
        assert!(js.contains("$.escape(v)"), "{js}");
        assert!(!js.contains("err"), "catch content must be dropped: {js}");
        assert!(js.contains("$$renderer.push(`<!--]-->`);"), "{js}");
    }

    #[test]
    fn compile_await_pending_only_has_empty_then() {
        // Pending-only await still emits 4 args with an empty `() => {}` then.
        let js =
            compile_js("<script>let { p } = $props();</script>\n{#await p}<p>load</p>{/await}");
        assert!(js.contains("() => {}"), "empty then arrow expected: {js}");
        assert!(js.contains("`<p>load</p>`"), "{js}");
    }

    #[test]
    fn compile_key_block() {
        let js = compile_js("{#key a}<p>c</p>{/key}");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \t$$renderer.push(`<!---->`);\n\
             \t{\n\
             \t\t$$renderer.push(`<p>c</p>`);\n\
             \t}\n\
             \t$$renderer.push(`<!---->`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_const_tag_folds_static_read() {
        // A `{@const}` enters the evaluator: a statically-known init folds a read
        // into the template while the declaration still emits.
        let js = compile_js("{#if true}{@const x = 2}<p>{x}</p>{/if}");
        assert!(js.contains("const x = 2;"), "const decl must emit: {js}");
        assert!(
            js.contains("`<p>2</p>`"),
            "static const read must fold: {js}"
        );
        assert!(
            !js.contains("$.escape(x)"),
            "known read must not stay dynamic: {js}"
        );
    }

    #[test]
    fn compile_const_tag_dynamic_read_stays_escaped() {
        // A `{@const}` over an unknown (each-local) value stays dynamic.
        let js = compile_js(
            "<script>let { items } = $props();</script>\n{#each items as item}{@const d = item}<p>{d}</p>{/each}",
        );
        assert!(js.contains("const d = item;"), "{js}");
        assert!(
            js.contains("$.escape(d)"),
            "dynamic const read must escape: {js}"
        );
    }

    #[test]
    fn compile_marks_text_first_each_body_not_if_branch() {
        // The each body gets a `<!---->` text-first marker; the if branch does not.
        let each = compile_js(
            "<script>let { items } = $props();</script>\n{#each items as item}hi {item}{/each}",
        );
        assert!(each.contains("`<!---->hi ${$.escape(item)}`"), "{each}");
        let iff = compile_js("<script>let { a } = $props();</script>\n{#if a}hi {a}{/if}");
        assert!(
            iff.contains("$$renderer.push(`hi ${$.escape(a)}`);"),
            "if branch must NOT get a text-first marker: {iff}"
        );
    }

    #[test]
    fn compile_rejects_nested_each() {
        assert_unsupported(
            "<script>let { m } = $props();</script>\n{#each m as row}{#each row as cell}<p>{cell}</p>{/each}{/each}",
            "nested {#each}",
        );
    }

    #[test]
    fn compile_rejects_const_at_root() {
        assert_unsupported(
            "{@const x = 1}<p>text</p>",
            "{@const} at the component root",
        );
    }

    #[test]
    fn compile_rejects_comments_with_blocks() {
        assert_unsupported(
            "<script>\n\t// note\n\tlet { a } = $props();\n</script>\n{#if a}<p>x</p>{/if}",
            "comments in a script alongside template blocks",
        );
    }

    #[test]
    fn compile_rejects_snippet_block() {
        assert_unsupported("{#snippet foo()}<p>x</p>{/snippet}", "{#snippet}");
    }

    #[test]
    fn compile_rejects_rune_inside_block() {
        // The guard runs on block test / body expressions too.
        assert_unsupported("{#if $state(0)}<p>x</p>{/if}", "$state");
        assert_unsupported(
            "<script>let { items } = $props();</script>\n{#each items as item}<p>{$state(0)}</p>{/each}",
            "$state",
        );
    }

    #[test]
    fn compile_state_rune_folds_known_read() {
        // `$state(0)` drops the wrapper; the never-updated binding is
        // statically known, so `{a}` folds into the template (the oracle's
        // evaluator behavior).
        let out = compile(
            "<script>let a = $state(0);</script>\n<p>{a}</p>",
            &CompileOptions::default(),
        )
        .unwrap();
        assert_eq!(
            out.js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tlet a = 0;\n\
             \t$$renderer.push(`<p>0</p>`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_state_rune_escapes_updated_read() {
        // A mutated state binding is not foldable — the read stays dynamic.
        let out = compile(
            "<script>\n\tlet a = $state(0);\n\tfunction inc() {\n\t\ta += 1;\n\t}\n</script>\n<p>{a}</p>",
            &CompileOptions::default(),
        )
        .unwrap();
        assert!(
            out.js.contains("`<p>${$.escape(a)}</p>`"),
            "updated state read must stay dynamic: {}",
            out.js
        );
    }

    #[test]
    fn compile_derived_rune_rewrites_init_and_read() {
        // `$derived(e)` → `$.derived(() => e)`; a bare template read of the
        // (non-foldable) derived binding becomes `d()`.
        let out = compile(
            "<script>\n\tlet a = $state(1);\n\tlet d = $derived(a * 2);\n\tfunction inc() {\n\t\ta += 1;\n\t}\n</script>\n<p>{d}</p>",
            &CompileOptions::default(),
        )
        .unwrap();
        assert!(
            out.js.contains("let d = $.derived(() => a * 2);"),
            "derived init not rewritten: {}",
            out.js
        );
        assert!(
            out.js.contains("`<p>${$.escape(d())}</p>`"),
            "derived read must become a call: {}",
            out.js
        );
    }

    /// Assert `compile` refuses with an `Unsupported` message containing `what`.
    fn assert_unsupported(source: &str, what: &str) {
        let err = compile(source, &CompileOptions::default()).unwrap_err();
        assert!(
            matches!(&err, CompileError::Unsupported(msg) if msg.contains(what)),
            "expected Unsupported({what}), got {err:?} for:\n{source}"
        );
    }

    #[test]
    fn compile_effect_forces_component_wrapper() {
        // Statement-position `$effect(…)` is dropped; the whole body moves
        // inside `$$renderer.component(($$renderer) => { … })`.
        let out = compile(
            "<script>\n\tlet { a } = $props();\n\t$effect(() => {});\n</script>\n<p>{a}</p>",
            &CompileOptions::default(),
        )
        .unwrap();
        assert_eq!(
            out.js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \t$$renderer.component(($$renderer) => {\n\
             \t\tlet { a } = $$props;\n\
             \t\t$$renderer.push(`<p>${$.escape(a)}</p>`);\n\
             \t});\n\
             }\n"
        );
    }

    #[test]
    fn compile_rejects_rune_in_nested_function() {
        assert_unsupported(
            "<script>\n\tfunction f() {\n\t\tlet c = $state(0);\n\t\treturn c;\n\t}\n</script>\n<p>text</p>",
            "$state",
        );
    }

    #[test]
    fn compile_state_raw_drops_wrapper() {
        // `$state.raw(v)` is a sanctioned init: the wrapper drops; an array
        // value isn't statically foldable, so the read stays dynamic.
        let out = compile(
            "<script>let a = $state.raw([1]);</script>\n<p>{a}</p>",
            &CompileOptions::default(),
        )
        .unwrap();
        assert!(out.js.contains("let a = [1];"), "got: {}", out.js);
        assert!(
            out.js.contains("`<p>${$.escape(a)}</p>`"),
            "got: {}",
            out.js
        );
    }

    #[test]
    fn compile_rejects_member_form_rune_misuse() {
        // Non-sanctioned member-form rune calls still refuse.
        assert_unsupported(
            "<script>\n\tlet { id } = $props;\n\tlet b = $props.id();\n</script>\n<p>{b}</p>",
            "$props",
        );
    }

    #[test]
    fn compile_rejects_rune_in_arrow_and_template_expression() {
        assert_unsupported(
            "<script>\n\tconst f = () => $inspect(1);\n</script>\n<p>text</p>",
            "$inspect",
        );
        assert_unsupported("<p>{$state(0)}</p>", "$state");
        // A rune buried inside a foldable expression must refuse — the guard
        // runs before evaluation, so the fold can't paper over it.
        assert_unsupported("<p>{true ? 1 : $state(2)}</p>", "$state");
    }

    #[test]
    fn compile_exponentiation_fold_matches_js_semantics() {
        // ECMAScript `**` special cases (oracle-verified): a NaN exponent and
        // |base| == 1 with an infinite exponent both fold to NaN, where IEEE
        // `pow` would give 1.
        for source in [
            "<p>{1 ** (1 / 0)}</p>",
            "<p>{(0 - 1) ** (1 / 0)}</p>",
            "<p>{1 ** (0 / 0)}</p>",
        ] {
            let out = compile(source, &CompileOptions::default()).unwrap();
            assert!(
                out.js.contains("`<p>NaN</p>`"),
                "{source} must fold to NaN: {}",
                out.js
            );
        }
        // The plain case stays IEEE.
        let out = compile("<p>{2 ** 3}</p>", &CompileOptions::default()).unwrap();
        assert!(out.js.contains("`<p>8</p>`"), "got: {}", out.js);
    }

    #[test]
    fn compile_carries_script_comments_losslessly() {
        // Leading, trailing-same-line, and between-statement comments carry
        // through: each present exactly once, relative order preserved, and
        // the output is a canonicalize fixed point.
        let out = compile(
            "<script>\n\t// leading\n\tlet { prop } = $props();\n\tlet a = 1; // trailing\n\t// between one\n\t// between two\n\tlet b = 2;\n</script>\n\n<p>{prop}</p>\n",
            &CompileOptions::default(),
        )
        .unwrap();
        let mut prev = 0;
        for comment in [
            "// leading",
            "// trailing",
            "// between one",
            "// between two",
        ] {
            let pos = out
                .js
                .find(comment)
                .unwrap_or_else(|| panic!("comment {comment:?} lost:\n{}", out.js));
            assert_eq!(
                out.js.matches(comment).count(),
                1,
                "comment {comment:?} duplicated:\n{}",
                out.js
            );
            assert!(pos >= prev, "comment {comment:?} reordered:\n{}", out.js);
            prev = pos + comment.len();
        }
        assert_eq!(canonicalize_js(&out.js).unwrap(), out.js);
    }

    #[test]
    fn compile_rejects_divergent_comment_classes() {
        // After the last script statement: the oracle re-attaches into the
        // template — refused.
        assert_unsupported(
            "<script>\n\tlet a = 1;\n\t// after last\n</script>\n<p>text</p>",
            "after the last script statement",
        );
        // Template-expression comments aren't carried yet.
        assert_unsupported("<p>{/* c */ 1}</p>", "template comments");
    }

    #[test]
    fn compile_rejects_bare_rune_reference() {
        // A bare $-prefixed identifier reference is oracle-rejected input —
        // refuse instead of compiling a broken passthrough.
        assert_unsupported(
            "<script>\n\tlet x = $state;\n</script>\n<p>text</p>",
            "$state",
        );
        assert_unsupported("<p>{$foo}</p>", "$foo");
    }

    #[test]
    fn compile_allows_dollar_member_names() {
        // A `$`-prefixed *name* (non-computed member property) is not a rune
        // reference — it stays compilable. The member access roots in the prop
        // `a`, so `needs_context` wraps the body. Full-string equality (not a
        // substring check) so the wrapper can't silently regress.
        let js = compile_js("<script>let { a } = $props();</script>\n<p>{a.$foo}</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \t$$renderer.component(($$renderer) => {\n\
             \t\tlet { a } = $$props;\n\
             \t\t$$renderer.push(`<p>${$.escape(a.$foo)}</p>`);\n\
             \t});\n\
             }\n"
        );
    }

    #[test]
    fn compile_member_on_prop_wraps() {
        // A member/call rooted in a prop is `needs_context`-unsafe — the whole
        // body wraps in `$$renderer.component(($$renderer) => …)`.
        let js = compile_js("<script>let { a } = $props();</script>\n<p>{a.b}</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \t$$renderer.component(($$renderer) => {\n\
             \t\tlet { a } = $$props;\n\
             \t\t$$renderer.push(`<p>${$.escape(a.b)}</p>`);\n\
             \t});\n\
             }\n"
        );
    }

    #[test]
    fn compile_member_on_local_does_not_wrap() {
        // A member rooted in a plain local binding is safe — no wrapper, and the
        // `$$props` parameter stays absent.
        let js = compile_js("<script>let a = { b: 1 };</script>\n<p>{a.b}</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tlet a = { b: 1 };\n\
             \t$$renderer.push(`<p>${$.escape(a.b)}</p>`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_new_expression_wraps_and_injects_props() {
        // A `new` expression sets `needs_context` even with no props; the wrapper
        // and the `$$props` parameter are both injected.
        let js = compile_js("<p>{new Date()}</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \t$$renderer.component(($$renderer) => {\n\
             \t\t$$renderer.push(`<p>${$.escape(new Date())}</p>`);\n\
             \t});\n\
             }\n"
        );
    }

    #[test]
    fn compile_refuses_member_on_shadowed_prop() {
        // A prop name reused as a nested binding makes a member/call root
        // ambiguous for this name-based analysis — refuse rather than guess.
        assert_unsupported(
            "<script>let { a } = $props();\n\tfunction f(a) {\n\t\treturn a.b;\n\t}</script>\n<p>{f(1)}</p>",
            "also bound in a nested scope",
        );
    }

    #[test]
    fn compile_hoists_instance_imports() {
        // A side-effect import hoists to module scope (an import inside the
        // component function is invalid JS).
        let js = compile_js("<script>import './x.js';</script>\n<p>text</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             import './x.js';\n\
             export default function Input($$renderer) {\n\
             \t$$renderer.push(`<p>text</p>`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_hoists_import_and_wraps_on_member_use() {
        // A named import hoists to module scope; a member access on the import
        // root also triggers the wrapper — the two fixes compose.
        let js = compile_js("<script>import { x } from './x.js';</script>\n<p>{x.y}</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             import { x } from './x.js';\n\
             export default function Input($$renderer, $$props) {\n\
             \t$$renderer.component(($$renderer) => {\n\
             \t\t$$renderer.push(`<p>${$.escape(x.y)}</p>`);\n\
             \t});\n\
             }\n"
        );
    }

    #[test]
    fn compile_refuses_components() {
        // Components compile to calls (`Foo($$renderer, {})`), not static markup —
        // component rendering is a later milestone, so they refuse now.
        assert_unsupported("<Foo />", "<Foo> component");
        assert_unsupported("<Foo>text</Foo>", "<Foo> component");
        assert_unsupported("<Foo.Bar />", "component");
    }

    #[test]
    fn compile_refuses_const_tag_shadowing_derived() {
        // A `{@const}` that shadows a top-level `$derived` refuses (the
        // name-based derived-read rewrite would wrongly call the const as `d()`).
        assert_unsupported(
            "<script>\n\tlet a = $state(1);\n\tlet d = $derived(a * 2);\n\tlet { items } = $props();\n\tfunction f() {\n\t\ta++;\n\t}\n</script>\n{#each items as item}{@const d = item.x}<p>{d}</p>{/each}",
            "shadows a $derived binding",
        );
    }

    #[test]
    fn compile_refuses_typed_script() {
        // A `lang="ts"` instance script passes type annotations through verbatim
        // (type stripping not implemented) — refuse at the entry, before output.
        assert_unsupported(
            "<script lang=\"ts\">let x: number = 5;</script>\n<p>text</p>",
            "lang=\"ts\" instance script",
        );
        // `generics` implies TS — refuse it too.
        assert_unsupported(
            "<script generics=\"T\">let x = 5;</script>\n<p>text</p>",
            "generics attribute",
        );
        // A plain instance script still compiles.
        compile(
            "<script>let x = 5;</script>\n<p>text</p>",
            &CompileOptions::default(),
        )
        .expect("plain script compiles");
    }

    #[test]
    fn compile_refuses_comment_glued_to_script_line() {
        // A leading comment glued to the `<script>` line (no newline before it)
        // would trail after the function brace — refuse rather than misplace it.
        assert_unsupported(
            "<script>// note\n\tlet { a } = $props();</script>\n<p>{a}</p>",
            "glued to the <script> line",
        );
    }

    #[test]
    fn compile_splits_multi_declarator_declaration() {
        // The oracle splits a multi-declarator top-level declaration into one
        // declaration per declarator, source order preserved.
        let js = compile_js("<script>let a = 1, b = a + 1;</script>\n<p>x</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tlet a = 1;\n\
             \tlet b = a + 1;\n\
             \t$$renderer.push(`<p>x</p>`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_splits_mixed_rune_and_plain_declarators() {
        // The per-declarator rune rewrites compose with the split.
        let js = compile_js(
            "<script>let a = $state(1), d = $derived(a * 2);\n\tfunction f() {\n\t\ta++;\n\t}</script>\n<p>{d}</p>",
        );
        assert!(
            js.contains("\tlet a = 1;\n\tlet d = $.derived(() => a * 2);\n"),
            "mixed declarators must split with rewrites applied: {js}"
        );
    }

    #[test]
    fn compile_keeps_nested_multi_declarator_joined() {
        // Only instance-script top-level declarations split; a declaration
        // inside a function body stays joined as ONE statement (the oracle
        // leaves it alone). The canonical reprint breaks its declarators across
        // continuation lines (multi-init declarations always break) — the same
        // on both sides of the parity diff, so still one `let`.
        let js = compile_js(
            "<script>function f() {\n\t\tlet a = 1,\n\t\t\tb = 2;\n\t\treturn a + b;\n\t}</script>\n<p>{f()}</p>",
        );
        assert!(
            js.contains("let a = 1,\n\t\t\tb = 2;"),
            "nested declaration must stay one statement: {js}"
        );
        assert_eq!(
            js.matches("let").count(),
            1,
            "nested declaration must not split: {js}"
        );
    }

    #[test]
    fn compile_refuses_comment_with_multi_declarator() {
        // The oracle re-anchors a comment INSIDE the split (`let // c` then the
        // declarator on the next line) — not reproducible, refuse.
        assert_unsupported(
            "<script>\n\t// lead\n\tlet a = 1, b = 2;\n</script>\n<p>x</p>",
            "multi-declarator declaration",
        );
    }

    #[test]
    fn compile_refuses_instance_script_exports() {
        // Every instance-script export form refuses: the oracle compiles
        // `export const`/`function`/`{a}` via `$.bind_props` (not implemented),
        // rejects `export default`/`export let` (runes mode), and drops
        // `export * from` — a verbatim passthrough would nest an `export`
        // inside the component function (invalid JS).
        for source in [
            "<script>export const a = 1;</script>\n<p>x</p>",
            "<script>export let a = 1;</script>\n<p>x</p>",
            "<script>export var a = 1;</script>\n<p>x</p>",
            "<script>export function f() {}</script>\n<p>x</p>",
            "<script>export class C {}</script>\n<p>x</p>",
            "<script>let a = 1;\n\texport { a };</script>\n<p>x</p>",
            "<script>export default 5;</script>\n<p>x</p>",
            "<script>export * from './x.js';</script>\n<p>x</p>",
            "<script>export { a } from './x.js';</script>\n<p>x</p>",
        ] {
            assert_unsupported(source, "instance-script export");
        }
    }

    #[test]
    fn compile_injects_slots_events_before_props_rest() {
        // A rest element in the `$props()` pattern gains the oracle's
        // `$$slots, $$events` injection immediately before it.
        let js = compile_js("<script>let { a, ...rest } = $props();</script>\n<p>{a}</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { a, $$slots, $$events, ...rest } = $$props;\n\
             \t$$renderer.push(`<p>${$.escape(a)}</p>`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_wraps_non_destructured_props_in_rest_pattern() {
        // `let props = $props()` becomes the oracle's
        // `let { $$slots, $$events, ...props } = $$props;`.
        let js = compile_js("<script>let props = $props();</script>\n<p>x</p>");
        assert_eq!(
            js,
            "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { $$slots, $$events, ...props } = $$props;\n\
             \t$$renderer.push(`<p>x</p>`);\n\
             }\n"
        );
    }

    #[test]
    fn compile_plain_props_destructure_gets_no_injection() {
        // No rest element → no `$$slots`/`$$events` (probe-verified).
        let js = compile_js("<script>let { a } = $props();</script>\n<p>{a}</p>");
        assert!(
            !js.contains("$$slots") && !js.contains("$$events"),
            "plain destructure must not gain the injection: {js}"
        );
    }

    #[test]
    fn compile_refuses_props_injection_with_comments() {
        // The injected properties' appendix spans between host-span siblings
        // would sweep host comments — refuse.
        assert_unsupported(
            "<script>\n\t// note\n\tlet { a, ...rest } = $props();\n</script>\n<p>{a}</p>",
            "rest-element $props()",
        );
        assert_unsupported(
            "<script>\n\t// note\n\tlet props = $props();\n</script>\n<p>x</p>",
            "non-destructured $props()",
        );
    }

    #[test]
    fn compile_refuses_array_pattern_props() {
        // The oracle rejects a non-identifier/non-object `$props()` binding
        // (props_invalid_identifier) — refuse rather than compile it.
        assert_unsupported(
            "<script>let [a] = $props();</script>\n<p>x</p>",
            "$props() binding pattern",
        );
    }

    #[test]
    fn compile_allows_lang_js_and_empty() {
        // The oracle compiles `lang="js"` and `lang=""` exactly like no lang
        // attribute; other values stay refused.
        for source in [
            "<script lang=\"js\">let x = 5;</script>\n<p>text</p>",
            "<script lang=\"\">let x = 5;</script>\n<p>text</p>",
        ] {
            compile(source, &CompileOptions::default())
                .unwrap_or_else(|e| panic!("{source} must compile: {e:?}"));
        }
        assert_unsupported(
            "<script lang=\"coffee\">let x = 5;</script>\n<p>text</p>",
            "lang=\"coffee\"",
        );
    }

    #[test]
    fn compile_rejects_option_and_populated_select() {
        // The oracle compiles <option> into $$renderer.option closures, and a
        // populated <select>/<optgroup> gets a `<!>` anchor — static emission
        // would diverge.
        assert_unsupported("<option value=\"a\">text</option>", "<option>");
        assert_unsupported(
            "<datalist><option value=\"a\">text</option></datalist>",
            "<option>",
        );
        assert_unsupported("<select><p>text</p></select>", "<select> with children");
        assert_unsupported(
            "<optgroup><p>text</p></optgroup>",
            "<optgroup> with children",
        );
    }

    #[test]
    fn compile_allows_empty_select() {
        // An empty <select> emits statically and matches the oracle.
        let out = compile("<select name=\"n\"></select>", &CompileOptions::default()).unwrap();
        assert!(
            out.js.contains("`<select name=\"n\"></select>`"),
            "got: {}",
            out.js
        );
    }

    #[test]
    fn compile_collapses_sibling_whitespace() {
        // Inter-sibling whitespace runs (newlines, blank lines) collapse to one
        // space; element-boundary whitespace trims (the oracle's clean_nodes).
        let out = compile(
            "<p>text1</p>\n\n<div>\n\t<p>text2</p>\n\t<p>text3</p>\n</div>\n",
            &CompileOptions::default(),
        )
        .unwrap();
        assert!(
            out.js
                .contains("`<p>text1</p> <div><p>text2</p> <p>text3</p></div>`"),
            "sibling/boundary whitespace not normalized: {}",
            out.js
        );
    }

    #[test]
    fn compile_preserves_text_interior_whitespace() {
        // Interior whitespace of a content text node is verbatim; edge runs
        // adjacent to {expr} tags stay (text + expr count as one text).
        let out = compile(
            "<script>let { a } = $props();</script>\n<p>text  x {a} y</p>",
            &CompileOptions::default(),
        )
        .unwrap();
        assert!(
            out.js.contains("`<p>text  x ${$.escape(a)} y</p>`"),
            "interior/expr-adjacent whitespace mangled: {}",
            out.js
        );
    }

    #[test]
    fn compile_preserves_pre_whitespace() {
        let out = compile("<pre>  a\n  b  </pre>", &CompileOptions::default()).unwrap();
        assert!(
            out.js.contains("`<pre>  a\n  b  </pre>`"),
            "pre whitespace not preserved: {}",
            out.js
        );
    }

    #[test]
    fn compile_marks_text_first_root_fragment() {
        let out = compile(" x <p>text</p> ", &CompileOptions::default()).unwrap();
        assert!(
            out.js.contains("`<!---->x <p>text</p>`"),
            "text-first root fragment must be <!----> prefixed: {}",
            out.js
        );
    }

    #[test]
    fn compile_decodes_and_reescapes_entities() {
        // Entities decode, then text re-escapes only & and < (the oracle's
        // escape_html content rule): &gt; becomes a literal >.
        let out = compile("<p>&amp; &lt; &gt; &quot;</p>", &CompileOptions::default()).unwrap();
        assert!(
            out.js.contains("`<p>&amp; &lt; > \"</p>`"),
            "entity decode/re-escape wrong: {}",
            out.js
        );
        // Attribute values re-escape &, ", and < (escape_html attr rule).
        let out = compile(
            "<p title=\"&amp; &lt; &gt; &quot;q\">text</p>",
            &CompileOptions::default(),
        )
        .unwrap();
        assert!(
            out.js.contains(" title=\"&amp; &lt; > &quot;q\""),
            "attribute entity escaping wrong: {}",
            out.js
        );
    }

    #[test]
    fn compile_mixed_attribute_full_fold_emits_static() {
        // Every part of a mixed attribute folding statically emits a STATIC
        // attribute (oracle-probed), not a $.attr*/$.attr_class call: value
        // attr-escaped [&"<] (> stays raw), no trim, boolean attributes keep
        // the folded value, null → ''.
        let js = compile_js(
            "<script>\n\tlet a = 1;\n\tlet b = 2;\n</script>\n\n<div class=\"{a}{b}\"></div>\n",
        );
        assert!(js.contains("`<div class=\"12\"></div>`"), "{js}");
        assert!(!js.contains("$.attr_class"), "must be static: {js}");
        let js = compile_js(
            "<script>\n\tlet a = `x\"y<z>&w`;\n\tlet b = 1;\n</script>\n\n<div title=\"p{a}q{b}\"></div>\n",
        );
        assert!(
            js.contains("`<div title=\"px&quot;y&lt;z>&amp;wq1\"></div>`"),
            "folded value must attr-escape [&\"<] with > raw: {js}"
        );
        let js = compile_js(
            "<script>\n\tlet a = null;\n\tlet b = 1;\n</script>\n\n<input disabled=\"x{a}{b}\" />\n",
        );
        assert!(
            js.contains("disabled=\"x1\""),
            "boolean attr keeps folded value; null folds to '': {js}"
        );
        // A folded-empty class stays `class=""` (the empty-class drop is
        // static-path-only, probe-verified).
        let js = compile_js(
            "<script>\n\tlet a = ``;\n\tlet b = ``;\n</script>\n\n<div class=\"{a}{b}\"></div>\n",
        );
        assert!(js.contains("`<div class=\"\"></div>`"), "{js}");
        // One non-foldable part keeps the whole attribute dynamic with the
        // known parts folded inline (the pre-existing path).
        let js = compile_js(
            "<script>\n\tlet a = 1;\n\tlet { b } = $props();\n</script>\n\n<div title=\"x{a}y{b}\"></div>\n",
        );
        assert!(
            js.contains("$.attr('title', `x1y${$.stringify(b)}`)"),
            "partial fold must stay dynamic: {js}"
        );
    }

    #[test]
    fn compile_empty_class_attribute_drops() {
        // A static string-valued class that collapses+trims to empty is
        // dropped entirely (oracle-probed); a bare `class` (boolean form)
        // keeps `class=""`, and empty style/id stay.
        let js = compile_js("<div class=\"\"></div>\n<div class=\"   \"></div>\n");
        assert!(js.contains("`<div></div> <div></div>`"), "{js}");
        let js = compile_js("<div class></div>\n");
        assert!(js.contains("`<div class=\"\"></div>`"), "{js}");
        let js = compile_js("<div id=\"\" style=\"\" class=\"\" title=\"t\"></div>\n");
        assert!(
            js.contains("`<div id=\"\" style=\"\" title=\"t\"></div>`"),
            "only class drops: {js}"
        );
    }

    #[test]
    fn compile_void_and_boolean_attributes() {
        let out = compile(
            "<p>text1<br />text2</p>\n<input value=\"value\" disabled />",
            &CompileOptions::default(),
        )
        .unwrap();
        assert!(
            out.js
                .contains("`<p>text1<br/>text2</p> <input value=\"value\" disabled=\"\"/>`"),
            "void self-close / boolean attribute wrong: {}",
            out.js
        );
    }

    #[test]
    fn compile_rejects_client_generation() {
        let options = CompileOptions {
            generate: Generate::Client,
            dev: false,
        };
        let err = compile("<p>text</p>", &options).unwrap_err();
        assert!(
            matches!(err, CompileError::Unsupported(_)),
            "expected Unsupported, got {err:?}"
        );
    }

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
}
