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

mod build;
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
pub fn compile(source: &str, options: &CompileOptions) -> Result<CompileOutput, CompileError> {
    if options.generate == Generate::Client {
        return Err(CompileError::Unsupported("client generation".to_string()));
    }
    if options.dev {
        return Err(CompileError::Unsupported("dev mode output".to_string()));
    }
    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena)?;
    transform_server::compile_server(&root, source, &arena)
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

    #[test]
    fn compile_rejects_unsupported_block() {
        let err = compile("{#if a}<p>text</p>{/if}", &CompileOptions::default()).unwrap_err();
        assert!(
            matches!(&err, CompileError::Unsupported(what) if what.contains("{#if}")),
            "expected Unsupported({{#if}}), got {err:?}"
        );
    }

    #[test]
    fn compile_rejects_unsupported_rune() {
        let err = compile(
            "<script>let a = $state(0);</script>\n<p>{a}</p>",
            &CompileOptions::default(),
        )
        .unwrap_err();
        assert!(
            matches!(&err, CompileError::Unsupported(what) if what.contains("$state")),
            "expected Unsupported($state), got {err:?}"
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
    fn compile_rejects_statement_position_rune() {
        assert_unsupported(
            "<script>\n\t$effect(() => {});\n</script>\n<p>text</p>",
            "$effect",
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
    fn compile_rejects_member_form_rune_init() {
        assert_unsupported(
            "<script>\n\tlet a = $state.raw([]);\n</script>\n<p>{a}</p>",
            "$state",
        );
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
    }

    #[test]
    fn compile_rejects_script_comments() {
        assert_unsupported(
            "<script>\n\t// note\n\tlet a = 1;\n</script>\n<p>text</p>",
            "comments in the instance script",
        );
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
        // A `$`-prefixed *name* (non-computed member property) is not a
        // reference — it must stay compilable.
        let out = compile(
            "<script>let { a } = $props();</script>\n<p>{a.$foo}</p>",
            &CompileOptions::default(),
        )
        .unwrap();
        assert!(out.js.contains("$.escape(a.$foo)"), "got: {}", out.js);
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
}
