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

/// The scoped CSS a component compiles to (panicking if it declines).
fn compile_css(source: &str) -> String {
    compile(source, &CompileOptions::default())
        .unwrap_or_else(|e| panic!("compile failed for {source:?}: {e:?}"))
        .css
        .unwrap_or_else(|| panic!("expected scoped css for {source:?}"))
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
    let js = compile_js("<script>let { p } = $props();</script>\n{#await p}<p>load</p>{/await}");
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
fn compile_carries_comments_with_blocks() {
    // A script comment carries through as a leading comment of its surviving
    // statement, unaffected by a template block: the block emits template-region
    // spans only, so no comment window sweeps the script comment.
    let js = compile_js("<script>\n\t// note\n\tlet a = 1;\n</script>\n{#if a}<p>x</p>{/if}");
    assert!(
        js.contains("// note"),
        "the script comment must carry through: {js}"
    );
}

#[test]
fn compile_comment_in_import_only_script() {
    // No surviving body statement (the import hoists to the comment-free module
    // program), so the carried comment leads the first synthetic statement instead.
    // The oracle trails it after that statement — a position difference the parity
    // bar tolerates, with the comment carried exactly once on both sides.
    let js =
        compile_js("<script>\n\t// note\n\timport Foo from './Foo.svelte';\n</script>\n<Foo />");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             import Foo from './Foo.svelte';\n\
             export default function Input($$renderer) {\n\
             \t// note\n\
             \tFoo($$renderer, {});\n\
             }\n"
    );
}

#[test]
fn compile_refuses_comment_after_last_statement_with_a_nested_block() {
    // A template that emits a synthetic (loc-less) block makes the oracle's printer
    // reset its monotonic comment index to the end, DROPPING every comment not yet
    // written — so an after-last comment vanishes from the oracle's output while
    // tsv keeps it. A drop is graded (unlike a position difference), so refuse.
    let what = "template that emits a nested block";
    for template in [
        "{#if x}<p>a</p>{/if}",
        "{#each [x] as n}<p>{n}</p>{/each}",
        "{#await x}<p>a</p>{:then v}<p>{v}</p>{/await}",
        "{#key x}<p>a</p>{/key}",
        "<div>{#if x}<p>a</p>{/if}</div>",
        "<svelte:head><title>t</title></svelte:head>",
    ] {
        assert_unsupported(
            &format!("<script>\n\tlet x = 1;\n\t// note\n</script>\n{template}"),
            what,
        );
    }
    // A component's children become a `children: ($$renderer) => { … }` block.
    assert_unsupported(
        "<script>\n\timport Foo from './Foo.svelte';\n\tlet x = 1;\n\t// note\n</script>\n<Foo>{x}</Foo>",
        what,
    );
}

#[test]
fn compile_carries_comment_after_last_statement_without_a_nested_block() {
    // The boundary of the refusal above: a component with no children (or only
    // whitespace) emits a bare call, not a block — probed against the oracle, which
    // keeps the comment in both forms. A `{@render}` is likewise a bare call.
    for template in ["<Foo />", "<Foo>\n</Foo>"] {
        let js = compile_js(&format!(
            "<script>\n\timport Foo from './Foo.svelte';\n\tlet x = 1;\n\t// note\n</script>\n{template}"
        ));
        assert!(js.contains("// note"), "comment must carry: {js}");
    }
    let js = compile_js(
        "<script>\n\tlet { children } = $props();\n\t// note\n</script>\n{@render children()}",
    );
    assert!(js.contains("// note"), "comment must carry: {js}");
}

#[test]
fn compile_comment_before_dropped_effect() {
    // The last SURVIVING statement is `let x = 1`; the `$effect` drops in SSR, so
    // the comment between them has no statement left to lead and falls to the
    // template emission that follows — inside the `needs_context` wrapper the
    // dropped effect forces.
    let js = compile_js(
        "<script>\n\tlet x = 1;\n\t// note\n\t$effect(() => {});\n</script>\n<p>{x}</p>",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \t$$renderer.component(($$renderer) => {\n\
             \t\tlet x = 1;\n\
             \t\t// note\n\
             \t\t$$renderer.push(`<p>1</p>`);\n\
             \t});\n\
             }\n"
    );
}

#[test]
fn compile_refuses_multiline_block_comment() {
    // The oracle re-indents a block comment's interior lines to the emit position;
    // tsv carries them verbatim, so they diverge. Refuse until the printer
    // re-indents block-comment interiors.
    assert_unsupported(
        "<script>\n\t/*\n\tmulti\n\tline\n\t*/\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        "multi-line block comment in script",
    );
}

#[test]
fn compile_refuses_comment_with_store() {
    // A store reference injects `var $$store_subs;` as a synthetic (appendix-span)
    // component-body statement whose leading comment window would sweep the carried
    // script comment (a double-print). Refuse. A script-position write mints
    // `$.store_set`/`$.store_get`, which sweep the same way.
    assert_unsupported(
        "<script>\n\timport { writable } from 'svelte/store';\n\tlet count = writable(0);\n\t// note\n\tfunction inc() {\n\t\t$count += 1;\n\t}\n</script>\n<button onclick={inc}>{$count}</button>",
        "references a store",
    );
    // A template-only `$name` read still injects the var, so it refuses too.
    assert_unsupported(
        "<script>\n\timport { writable } from 'svelte/store';\n\tlet count = writable(0);\n\t// note\n\tlet x = 1;\n</script>\n<p>{$count}{x}</p>",
        "references a store",
    );
}

#[test]
fn compile_hoistable_snippet_and_render() {
    // A top-level snippet whose only reference is its own parameter hoists to
    // module scope; `{@render foo(1)}` becomes `foo($$renderer, 1)`, standalone
    // (sole child, non-dynamic) so no trailing anchor.
    let js = compile_js("{#snippet foo(x)}<p>{x}</p>{/snippet}\n{@render foo(1)}");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             function foo($$renderer, x) {\n\
             \t$$renderer.push(`<p>${$.escape(x)}</p>`);\n\
             }\n\
             export default function Input($$renderer) {\n\
             \tfoo($$renderer, 1);\n\
             }\n"
    );
}

#[test]
fn compile_non_hoistable_snippet_stays_in_body() {
    // A snippet referencing a prop can't hoist — the `function` declaration
    // stays in the component body, after the props destructure.
    let js = compile_js(
        "<script>let { name } = $props();</script>\n{#snippet foo()}<p>{name}</p>{/snippet}\n{@render foo()}",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { name } = $$props;\n\
             \tfunction foo($$renderer) {\n\
             \t\t$$renderer.push(`<p>${$.escape(name)}</p>`);\n\
             \t}\n\
             \tfoo($$renderer);\n\
             }\n"
    );
}

#[test]
fn compile_snippet_component_spread_reference_blocks_hoist() {
    // The regression shape: a snippet whose ONLY instance-binding reference
    // sits in a component `{...spread}` must NOT module-hoist (a hoisted
    // `function s` referencing `n` declared inside Input is a runtime
    // ReferenceError — invisible to the reparse self-validation). The
    // shared attr_refs traversal makes the hoist collector see the spread.
    let js = compile_js(
        "<script>import Foo from './Foo.svelte';\n\tlet n = $state({ a: 1 });</script>\n{#snippet s()}<Foo {...n} />{/snippet}\n{@render s()}",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             import Foo from './Foo.svelte';\n\
             export default function Input($$renderer) {\n\
             \tlet n = { a: 1 };\n\
             \tfunction s($$renderer) {\n\
             \t\tFoo($$renderer, $.spread_props([n]));\n\
             \t}\n\
             \ts($$renderer);\n\
             }\n"
    );
    // The same discipline for a prop and a plain top-level const, and with
    // the component nested inside an element.
    for source in [
        "<script>let { p } = $props();</script>\n{#snippet s()}<Foo {...p} />{/snippet}\n{@render s()}",
        "<script>const c = { a: 1 };</script>\n{#snippet s()}<Foo {...c} />{/snippet}\n{@render s()}",
        "<script>let n = $state({ a: 1 });</script>\n{#snippet s()}<div><Foo {...n} /></div>{/snippet}\n{@render s()}",
    ] {
        let js = compile_js(source);
        assert!(
            js.contains("export default function Input")
                && js.find("function s($$renderer)").unwrap()
                    > js.find("export default function Input").unwrap(),
            "snippet must stay inside the component body for {source:?}:\n{js}"
        );
    }
}

#[test]
fn compile_snippet_component_spread_of_import_still_hoists() {
    // Imports (and globals) don't disqualify hoisting — a component spread of
    // an import keeps the snippet at module scope.
    let js = compile_js(
        "<script>import Foo from './Foo.svelte';\n\timport { cfg } from './cfg.js';</script>\n{#snippet s()}<Foo {...cfg} />{/snippet}\n{@render s()}",
    );
    assert!(
        js.find("function s($$renderer)").unwrap()
            < js.find("export default function Input").unwrap(),
        "import-spread snippet must module-hoist: {js}"
    );
    let js = compile_js(
        "<script>import Foo from './Foo.svelte';</script>\n{#snippet s()}<Foo {...globalThis.cfg} />{/snippet}\n{@render s()}",
    );
    assert!(
        js.find("function s($$renderer)").unwrap()
            < js.find("export default function Input").unwrap(),
        "global-spread snippet must module-hoist: {js}"
    );
}

#[test]
fn compile_render_prop_snippet_is_dynamic() {
    // `{@render children()}` where `children` is a prop is dynamic, so the
    // render tag keeps the trailing `<!---->` even as the sole child.
    let js = compile_js("<script>let { children } = $props();</script>\n{@render children()}");
    assert!(
        js.contains("children($$renderer);\n\t$$renderer.push(`<!---->`);"),
        "dynamic prop render must keep the anchor: {js}"
    );
}

#[test]
fn compile_render_optional_callee() {
    // `{@render foo?.()}` → `foo?.($$renderer)`.
    let js = compile_js("{#snippet foo()}<b>s</b>{/snippet}\n{@render foo?.()}");
    assert!(js.contains("foo?.($$renderer);"), "{js}");
}

#[test]
fn compile_typed_and_generic_snippet() {
    // A `: T` parameter annotation and a `<T>` clause are both ordinary
    // erasure: the oracle emits `function foo($$renderer, x)` either way, the
    // type-level syntax simply gone.
    let js = compile_js(
        "<script lang=\"ts\">\n\tlet { n }: { n: number } = $props();\n</script>\n\
             {#snippet foo(x: number)}<p>{x}</p>{/snippet}\n{@render foo(n)}",
    );
    assert!(
        js.contains("function foo($$renderer, x) {"),
        "annotated snippet param must erase: {js}"
    );
    let generic = compile_js(
        "<script lang=\"ts\">\n\tlet { n }: { n: number } = $props();\n</script>\n\
             {#snippet foo<T>(x: T)}<p>{x}</p>{/snippet}\n{@render foo(n)}",
    );
    assert!(
        generic.contains("function foo($$renderer, x) {"),
        "generic snippet must erase its <T>: {generic}"
    );
}

#[test]
fn compile_rejects_render_member_callee() {
    assert_unsupported(
        "<script>let { obj } = $props();</script>\n{@render obj.snip()}",
        "{@render} callee is not a resolvable local snippet or snippet prop",
    );
}

#[test]
fn compile_rejects_duplicate_snippet_name() {
    assert_unsupported(
        "{#snippet foo()}<b>1</b>{/snippet}\n{#snippet foo()}<b>2</b>{/snippet}\n{@render foo()}",
        "duplicate {#snippet} foo",
    );
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
fn compile_class_state_field_unwraps() {
    // A top-level class `$state` field unwraps exactly like a top-level `$state`
    // declarator: `count = $state(0)` → `count = 0`. The `new Counter()` forces
    // the component wrapper. (Durable coverage is `runes/class_state_*`.)
    let out = compile(
        "<script>\n\tclass Counter {\n\t\tcount = $state(0);\n\t}\n\tconst c = new Counter();\n</script>\n<p>{c.count}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("count = 0;"),
        "class $state field not unwrapped: {}",
        out.js
    );
    assert!(
        !out.js.contains("$state"),
        "no $state reference may survive: {}",
        out.js
    );
}

#[test]
fn compile_class_derived_field_refuses() {
    // A `$derived` class field is the SEPARATE v2 slice (the `#f = $.derived(…)` +
    // get/set accessor transform). The oracle ACCEPTS it, so this is a deliberate
    // over-refusal — it must refuse as `rune $derived`, not silently unwrap.
    assert_unsupported(
        "<script>\n\tclass C {\n\t\ta = $state(1);\n\t\tdouble = $derived(this.a * 2);\n\t}\n\tconst c = new C();\n</script>\n<p>{c.double}</p>",
        "$derived",
    );
}

#[test]
fn compile_class_static_state_field_refuses() {
    // A `static` rune field is oracle-rejected placement (`state_invalid_placement`)
    // — unwrapping it would be an over-acceptance. It must keep refusing.
    assert_unsupported(
        "<script>\n\tclass C {\n\t\tstatic a = $state(0);\n\t}\n\tconst c = new C();\n</script>\n<p>{C}</p>",
        "$state",
    );
}

#[test]
fn compile_class_computed_key_state_field_refuses() {
    // A computed-key rune field is oracle-rejected placement. It must keep refusing
    // (the `!computed` guard keeps it off the unwrap path).
    assert_unsupported(
        "<script>\n\tconst k = 'x';\n\tclass C {\n\t\t[k] = $state(0);\n\t}\n\tconst c = new C();\n</script>\n<p>{c.x}</p>",
        "$state",
    );
}

#[test]
fn compile_class_constructor_state_assignment_refuses() {
    // A constructor first-assignment `this.x = $state(0)` is a method-body
    // assignment (the oracle accepts it → `this.x = 0`), deferred this slice — the
    // method body takes the normal refusing guard walk.
    assert_unsupported(
        "<script>\n\tclass C {\n\t\tconstructor() {\n\t\t\tthis.x = $state(0);\n\t\t}\n\t}\n\tconst c = new C();\n</script>\n<p>{c.x}</p>",
        "$state",
    );
}

#[test]
fn compile_nested_class_state_field_refuses() {
    // A `$state` field in a NESTED class (inside a function body) is NOT reached by
    // the top-level-only transform, so the guard walk refuses it. This pins the
    // reach boundary: the exempt set (unwrapped fields) must equal the transform's
    // reach, or a guard exemption without a matching unwrap would emit an undefined
    // `$state` reference (a MISMATCH).
    assert_unsupported(
        "<script>\n\tfunction make() {\n\t\tclass Inner {\n\t\t\tx = $state(0);\n\t\t}\n\t\treturn new Inner();\n\t}\n\tconst c = make();\n</script>\n<p>{c.x}</p>",
        "$state",
    );
}

#[test]
fn compile_class_state_lone_store_arg_refuses() {
    // A class-field `$state($count)` / `$state.raw($count)` whose WHOLE argument is
    // a lone store read: the oracle keeps it BARE (`x = $count`), but tsv's store
    // rewrite descends into class bodies and would rewrite the kept argument to
    // `$.store_get(…)` — a corpus-invisible MISMATCH. Refuse (a narrow, safe
    // over-refusal; a compound `$state($count + 1)` still compiles at parity).
    let store = "import { writable } from 'svelte/store';\n\tconst count = writable(0);";
    assert_unsupported(
        &format!(
            "<script>\n\t{store}\n\tclass C {{\n\t\tx = $state($count);\n\t}}\n\tconst c = new C();\n</script>\n<p>{{c.x}}</p>"
        ),
        "lone store/$derived argument",
    );
    assert_unsupported(
        &format!(
            "<script>\n\t{store}\n\tclass C {{\n\t\tx = $state.raw($count);\n\t}}\n\tconst c = new C();\n</script>\n<p>{{c.x}}</p>"
        ),
        "lone store/$derived argument",
    );
}

#[test]
fn compile_class_state_lone_derived_arg_refuses() {
    // A class-field `$state(d)` / `$state.raw(d)` whose WHOLE argument is a lone
    // `$derived` read: the oracle keeps it bare (`x = d`), but tsv's store rewrite
    // would rewrite the kept argument to `d()` — a MISMATCH. Refuse (a compound
    // `$state(d + 1)` still compiles at parity).
    let derived = "let n = $state(1);\n\tconst d = $derived(n * 2);";
    assert_unsupported(
        &format!(
            "<script>\n\t{derived}\n\tclass C {{\n\t\tx = $state(d);\n\t}}\n\tconst c = new C();\n</script>\n<p>{{c.x}}</p>"
        ),
        "lone store/$derived argument",
    );
    assert_unsupported(
        &format!(
            "<script>\n\t{derived}\n\tclass C {{\n\t\tx = $state.raw(d);\n\t}}\n\tconst c = new C();\n</script>\n<p>{{c.x}}</p>"
        ),
        "lone store/$derived argument",
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

#[test]
fn compile_derived_read_refuses_deferred_positions() {
    // The template VALUE walk and the script-position rewrite (`store_rewrite`)
    // turn a derived read into `d()` (the fixtures `runes/derived_read_*` and
    // `runes/derived_read_script_*`). Positions NOT routed through either keep
    // refusing the derived read (`DerivedBindingRead`, "read of derived binding")
    // — never a MISMATCH.
    //
    // A `{#each}` context pattern default: the oracle emits a BARE `d` here
    // (`let { v = d }`), so tsv could match by borrowing verbatim — but patterns are
    // not rewritten this slice, so refusing is a deferred safe over-refusal.
    assert_unsupported(
        "<script>\n\tlet { a, xs } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{#each xs as { v = d }}{v}{/each}",
        "read of derived binding",
    );
    // A `{:then}` value pattern default: here the oracle emits `d()`
    // (`({ x = d() }) => …`), so borrowing the pattern verbatim WOULD emit a bare `d`
    // — a MISMATCH. Refusing is mandatory until patterns route through the walk.
    assert_unsupported(
        "<script>\n\tlet { a, p } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{#await p then { x = d }}{x}{/await}",
        "read of derived binding",
    );
    // A derived assignment target (`{d = 1}`) — the guard refuses the derived
    // WRITE (a template mutation would refuse too). A derived write is out of scope
    // on every path (the oracle lowers it to `d(v)`).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{d = 1}",
        "read of derived binding",
    );
    // A derived read under an ObjectExpression (`{f({ x: d })}`) — a wrapper kind the
    // value walk does not descend, so it never reaches the rewrite and the guard
    // refuses it (a safe over-refusal).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tfunction f(o) {\n\t\treturn o;\n\t}\n</script>\n{f({ x: d })}",
        "read of derived binding",
    );
    // An ESCAPED-identifier read of a `$derived` name: the six source bytes
    // `d` are the escaped spelling of the identifier `d`, which the oracle emits
    // as `d()`. The value-walk can't rewrite an escaped read (classification not
    // ported), so the rune guard refuses it rather than emit a bare `d` — a MISMATCH.
    // Both bare and nested.
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{\\u0064}",
        "read of derived binding",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n</script>\n{\\u0064 + 1}",
        "read of derived binding",
    );
}

#[test]
fn compile_derived_read_script_position_rewrites() {
    // A `$derived` read in a SCRIPT position (a top-level initializer, a function
    // body, a `$.derived(() => …)` thunk) rewrites to `d()`, the same lowering the
    // template value walk applies — extended to the script by `store_rewrite`.
    // Script positions never fold (only template text folds), so it is always
    // `d()`, never the derived's value.
    let out = compile(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet e = d + 1;\n\
         \tfunction total() {\n\t\treturn d + 1;\n\t}\n\tlet d2 = $derived(d + 1);\n\
         </script>\n<button onclick={total}>{a}</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    // The top-level initializer (no fold), the function-body read, and the read
    // inside the `$.derived` thunk all become `d()`; the derived declarations keep
    // their bare binding names.
    assert!(
        out.js.contains("let e = d() + 1;"),
        "top-level init: {}",
        out.js
    );
    assert!(out.js.contains("return d() + 1;"), "fn body: {}", out.js);
    assert!(
        out.js.contains("let d2 = $.derived(() => d() + 1);"),
        "nested-in-derived: {}",
        out.js
    );
    assert!(
        out.js.contains("let d = $.derived(() => a * 2);"),
        "binding id bare: {}",
        out.js
    );
}

#[test]
fn compile_derived_read_name_only_positions_stay_bare() {
    // Name-only positions are NOT reads: a non-computed member property (`o.d`) and
    // an object key (`{ d: 1 }`) stay verbatim, exactly like the store rewrite.
    let out = compile(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction g() {\n\t\tconst o = { d: 1 };\n\t\treturn o.d + d;\n\t}\n\
         </script>\n<button onclick={g}>{a}</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("const o = { d: 1 };"),
        "object key stays: {}",
        out.js
    );
    assert!(
        out.js.contains("return o.d + d();"),
        "member stays, read rewrites: {}",
        out.js
    );
}

#[test]
fn compile_derived_read_shadowed_refuses() {
    // A `$derived` name shadowed by a nested-scope binding (a param/local) is
    // ambiguous for the name-based rewrite (`return d` inside `f(d)` is the param,
    // not the derived). Refuse the whole compile — a safe over-refusal (shadowing a
    // derived is legal, so this is never a MISMATCH).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction f(d) {\n\t\treturn d;\n\t}\n</script>\n<button onclick={() => f(1)}>{a}</button>",
        "shadowed in a nested scope",
    );
    // A nested local (not a parameter) shadows too.
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction f() {\n\t\tlet d = 5;\n\t\treturn d;\n\t}\n</script>\n<button onclick={f}>{a}</button>",
        "shadowed in a nested scope",
    );
}

#[test]
fn compile_derived_write_refuses() {
    // A write to the derived binding ITSELF (`d = v` / `d++`) is out of scope — the
    // oracle lowers it to `d(v)` / `$.update_derived(d)`, which this slice does not
    // emit. The rune guard refuses the bare-identifier target (`DerivedBindingRead`).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction b() {\n\t\td = 5;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\
         \tfunction b() {\n\t\td++;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
    // A destructuring assignment whose leaf binds the derived (`[d] = …`,
    // `({ d } = …)`, `[z, d] = …`) is a derived write too — the oracle lowers it to
    // an `$.to_array` IIFE / `d(obj.d)`. The guard refuses the binding leaf.
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet arr = [1];\n\
         \tfunction b() {\n\t\t[d] = arr;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet obj = { d: 1 };\n\
         \tfunction b() {\n\t\t({ d } = obj);\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet z;\n\tlet arr = [1, 2];\n\
         \tfunction b() {\n\t\t[z, d] = arr;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        "read of derived binding",
    );
}

#[test]
fn compile_derived_member_write_compiles() {
    // A member/index target READS the derived (its object / computed index), never
    // binds it — `d.x = v` → `d().x = v` and `x[d] = v` → `x[d()] = v` compile via
    // the read rewrite (the narrower binding-leaf refusal stops at members).
    let out = compile(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived({ x: a });\n\
         \tfunction b() {\n\t\td.x = 5;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("d().x = 5;"),
        "member write reads derived: {}",
        out.js
    );

    let out = compile(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet arr = [1];\n\
         \tfunction b() {\n\t\tarr[d] = 5;\n\t}\n</script>\n<button onclick={b}>{a}</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("arr[d()] = 5;"),
        "index write reads derived: {}",
        out.js
    );
}

#[test]
fn compile_derived_by_bare_read_compiles() {
    // `$derived.by(d)` (a bare derived argument) compiles: `.by` passes `d` straight
    // through as the compute function (`$.derived(d)`) and the read rewrite lowers
    // it to `$.derived(d())`, the oracle's output. (Contrast `$derived(d)`, which the
    // oracle unthunk-collapses to `$.derived(d)` — refused as unreproducible.)
    let out = compile(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n\tlet e = $derived.by(d);\n\
         </script>\n<button onclick={() => e}>{a}</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("let e = $.derived(d());"),
        "$derived.by(d): {}",
        out.js
    );
}

#[test]
fn compile_escaped_local_read_still_compiles() {
    // An escaped identifier is NOT auto-refused — only one decoding to a `$derived`
    // name is. An escaped read of a plain (non-derived) local compiles, reading the
    // binding bare (`d`, never `d()`).
    let out = compile(
        "<script>\n\tlet { a } = $props();\n\tlet d = a * 2;\n</script>\n{\\u0064}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.escape(d)"),
        "escaped plain-local read must compile bare: {}",
        out.js
    );
}

#[test]
fn compile_derived_read_state_stays_bare() {
    // Only names in `derived_names` rewrite. A reassigned `$state` binding is NOT
    // derived, so a nested read of it stays bare (`s + 1`, never `s() + 1`).
    let out = compile(
        "<script>\n\tlet s = $state(1);\n\tfunction inc() {\n\t\ts += 1;\n\t}\n</script>\n{s + 1}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.escape(s + 1)"),
        "state read must stay bare: {}",
        out.js
    );
}

/// Assert `compile` refuses with an `Unsupported` message containing `what`.
fn assert_unsupported(source: &str, what: &str) {
    let err = compile(source, &CompileOptions::default()).unwrap_err();
    assert!(
        matches!(&err, CompileError::Unsupported(reason) if reason.to_string().contains(what)),
        "expected Unsupported({what}), got {err:?} for:\n{source}"
    );
}

/// Assert `compile` fails at the parse stage with a message containing `what`.
fn assert_parse_rejected(source: &str, what: &str) {
    let err = compile(source, &CompileOptions::default()).unwrap_err();
    assert!(
        matches!(&err, CompileError::Parse(e) if e.to_string().contains(what)),
        "expected Parse({what}), got {err:?} for:\n{source}"
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
fn compile_inspect_with_wrong_arity_refuses() {
    // `$inspect(a).with(cb)` drops only with EXACTLY one `.with` argument. A
    // wrong outer arity is a hard oracle error (`rune_invalid_arguments_length`:
    // "`$inspect().with` must be called with exactly one argument"), so the
    // recognizer must not drop it — it falls through to the rune guard.
    assert_unsupported(
        "<script>\n\tlet a = $state(0);\n\t$inspect(a).with();\n</script>\n<p>{a}</p>",
        "$inspect",
    );
    assert_unsupported(
        "<script>\n\tlet a = $state(0);\n\t$inspect(a).with((t, v) => v, 1);\n</script>\n<p>{a}</p>",
        "$inspect",
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
    // A bare `$props` reference (destructuring the rune itself) refuses.
    assert_unsupported(
        "<script>\n\tlet { id } = $props;\n</script>\n<p>text</p>",
        "$props",
    );
    // A non-sanctioned member-form rune call still refuses (`$props.id()` and
    // `$state.snapshot(x)` are the sanctioned member-form runes; `$state.foo()`
    // is not).
    assert_unsupported(
        "<script>\n\tlet b = $state.foo();\n</script>\n<p>{b}</p>",
        "$state",
    );
}

#[test]
fn compile_props_id_hoists_declaration() {
    // `const id = $props.id()` is skipped in place; a `const id =
    // $.props_id($$renderer)` is hoisted to the top of the component body, and a
    // `{id}` read stays dynamic (`$.escape(id)`, never a fold).
    let out = compile(
        "<script>\n\tconst id = $props.id();\n</script>\n<p>{id}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert_eq!(
        out.js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tconst id = $.props_id($$renderer);\n\
             \t$$renderer.push(`<p>${$.escape(id)}</p>`);\n\
             }\n"
    );
}

#[test]
fn compile_props_id_hoists_before_other_declarators() {
    // In `const a = 1, id = $props.id()` the hoisted `id` decl leads the body,
    // then the surviving `const a = 1` (the oracle's shape).
    let out = compile(
        "<script>\n\tconst a = 1,\n\t\tid = $props.id();\n</script>\n<p>{a}{id}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js
            .contains("const id = $.props_id($$renderer);\n\tconst a = 1;"),
        "props.id decl must lead: {}",
        out.js
    );
}

#[test]
fn compile_props_id_refuses_misuse() {
    // Arguments (`rune_invalid_arguments`) — not recognized as `$props.id()`, so
    // the guard refuses the stray `$props`-rooted call.
    assert_unsupported(
        "<script>\n\tconst id = $props.id(1);\n</script>\n<p>{id}</p>",
        "$props",
    );
    // A destructured target (`props_id_invalid_placement`).
    assert_unsupported(
        "<script>\n\tconst { x } = $props.id();\n</script>\n<p>{x}</p>",
        "$props.id()",
    );
    // A template position (`props_id_invalid_placement`) — the guard refuses it.
    assert_unsupported("<p>{$props.id()}</p>", "$props");
    // A second `$props.id()` (`props_duplicate`).
    assert_unsupported(
        "<script>\n\tconst a = $props.id();\n\tconst b = $props.id();\n</script>\n<p>{a}{b}</p>",
        "more than once",
    );
    // In a module script (`props_id_invalid_placement` — module scope). A plain
    // module now compiles, so the module guard refuses the stray `$props`-rooted
    // call (a module-scope rune) rather than declining the whole module up front.
    assert_unsupported(
        "<script module>\n\tconst id = $props.id();\n</script>\n<p>text</p>",
        "$props",
    );
}

#[test]
fn compile_state_snapshot_declarator_unwraps() {
    // `const s = $state.snapshot(obj)` unwraps to `const s = obj`; the `{s.a}`
    // read stays dynamic.
    let out = compile(
        "<script>\n\tlet obj = $state({ a: 1 });\n\tconst s = $state.snapshot(obj);\n</script>\n<p>{s.a}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(out.js.contains("const s = obj;"), "got: {}", out.js);
    assert!(
        out.js.contains("`<p>${$.escape(s.a)}</p>`"),
        "got: {}",
        out.js
    );
}

#[test]
fn compile_state_snapshot_template_rewrites_to_runtime_call() {
    // A `$state.snapshot(x)` in a template value becomes `$.snapshot(x)`, at the
    // root and nested inside a wrapper expression.
    let bare = compile(
        "<script>\n\tlet obj = $state({ a: 1 });\n</script>\n{$state.snapshot(obj)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        bare.js.contains("$.escape($.snapshot(obj))"),
        "bare snapshot: {}",
        bare.js
    );
    let nested = compile(
        "<script>\n\tlet state = $state({ a: 1 });\n</script>\n{2 in $state.snapshot(state)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        nested.js.contains("$.escape(2 in $.snapshot(state))"),
        "nested snapshot: {}",
        nested.js
    );
}

#[test]
fn compile_state_snapshot_derived_arg_becomes_call() {
    // A bare derived read as the snapshot argument becomes `d()` inside the
    // `$.snapshot(...)` call.
    let out = compile(
        "<script>\n\tlet a = $state(1);\n\tlet d = $derived(a * 2);\n</script>\n{$state.snapshot(d)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.escape($.snapshot(d()))"),
        "got: {}",
        out.js
    );
    // A NESTED derived read inside the snapshot argument (`d + 1`) also rewrites —
    // the snapshot walk and the derived-read walk compose on one node set.
    let nested = compile(
        "<script>\n\tlet a = $state(1);\n\tlet d = $derived(a * 2);\n</script>\n{$state.snapshot(d + 1)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        nested.js.contains("$.escape($.snapshot(d() + 1))"),
        "nested derived in snapshot arg: {}",
        nested.js
    );
}

#[test]
fn compile_state_snapshot_refuses_wrong_arity_and_deferred_positions() {
    // Arity ≠ 1 (`rune_invalid_arguments_length`) — not recognized as snapshot,
    // so the guard refuses the stray `$state`-rooted call.
    assert_unsupported(
        "<script>\n\tlet o = $state({ a: 1 });\n</script>\n{$state.snapshot()}",
        "$state",
    );
    assert_unsupported(
        "<script>\n\tlet o = $state({ a: 1 });\n</script>\n{$state.snapshot(o, 1)}",
        "$state",
    );
    // A destructured declarator (the oracle's temp-destructure lowering) — a safe
    // over-refusal.
    assert_unsupported(
        "<script>\n\tlet obj = $state({ a: 1 });\n\tconst { a } = $state.snapshot(obj);\n</script>\n<p>{a}</p>",
        "$state.snapshot",
    );
    // A script non-declarator position (deferred this slice) — the guard refuses it.
    assert_unsupported(
        "<script>\n\tlet x = $state(1);\n\tfunction f() {\n\t\treturn $state.snapshot(x);\n\t}\n</script>\n<p>text</p>",
        "$state",
    );
}

#[test]
fn compile_rune_optional_chain_declarator_refuses() {
    // An optional-chained rune init (`$state.snapshot?.(x)`, `$state?.snapshot(x)`,
    // `$props.id?.()`, `$state?.(1)`, …) is a ChainExpression the oracle's
    // `get_rune` does not see through, so its declarator-unwrap never applies. tsv
    // refuses to classify the optional form (a safe over-refusal) — closing a
    // net-new MISMATCH (`$state.snapshot?.()`, where the oracle emits
    // `$.snapshot(x)` and unwrapping to `x` diverged) and a pre-existing
    // optional-chain over-acceptance family for the placement-restricted runes
    // (the oracle rejects those, tsv used to compile them). Both the
    // optional-call and optional-member spellings, over every declarator-unwrap
    // rune.
    for src in [
        "<script>\n\tlet o = $state({ a: 1 });\n\tconst s = $state.snapshot?.(o);\n</script>\n<p>{s.a}</p>",
        "<script>\n\tlet o = $state({ a: 1 });\n\tconst s = $state?.snapshot(o);\n</script>\n<p>{s.a}</p>",
        "<script>\n\tconst id = $props.id?.();\n</script>\n<p>{id}</p>",
        "<script>\n\tconst x = $state?.(1);\n</script>\n<p>{x}</p>",
        "<script>\n\tconst p = $props?.();\n</script>\n<p>text</p>",
        "<script>\n\tconst d = $derived?.(1);\n</script>\n<p>{d}</p>",
    ] {
        assert!(
            compile(src, &CompileOptions::default()).is_err(),
            "optional-chained rune init must refuse: {src}"
        );
    }
}

#[test]
fn compile_state_snapshot_optional_chain_template_still_parity() {
    // In a TEMPLATE value position the optional form is fine: the oracle emits
    // `$.snapshot(x)` regardless of the `?.`, and `snapshot_call_arg` matches it,
    // so tsv emits the same — the declarator guard above does NOT reach here.
    let out = compile(
        "<script>\n\tlet o = $state({ a: 1 });\n</script>\n{$state.snapshot?.(o)}",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.escape($.snapshot(o))"),
        "got: {}",
        out.js
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
fn compile_carries_comment_after_last_statement() {
    // A comment past the last script statement leads the first synthetic statement
    // (the template flush). The oracle instead trails it after that statement —
    // position-tolerated, same single comment.
    let js = compile_js("<script>\n\tlet a = 1;\n\t// after last\n</script>\n<p>text</p>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tlet a = 1;\n\
             \t// after last\n\
             \t$$renderer.push(`<p>text</p>`);\n\
             }\n"
    );
}

#[test]
fn compile_rejects_template_expression_comments() {
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
fn compile_refuses_dollar_prefixed_bindings() {
    // The oracle's `dollar_prefix_invalid`
    // (`phases/2-analyze/visitors/shared/utils.js:278` —
    // `node.name.startsWith('$')` on a Binding) is a Svelte reserved-prefix rule
    // on a BINDING, not on a reference and not a JS early error. `$$slots` is
    // the sharp edge: a *reference* is the real runtime value the transform
    // injects (`$.sanitize_slots`), so the guard exempts it, but a
    // *declaration* of that name is a compile error and must not inherit the
    // reference's exemption. Every case below was probe-verified oracle-rejected
    // against the pinned compiler.
    //
    // Variable declarators — any declaration kind, any function depth
    // (the `VariableDeclarator` visitor validates with no `function_depth`
    // argument, so the depth gate never applies), destructured or not.
    assert_unsupported(
        "<script>\n\tlet $$slots = 1;\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script module>\n\tlet $$slots = 1;\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tconst $$slots = 1;\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tfunction f() {\n\t\tvar $$slots = 1;\n\t\treturn $$slots;\n\t}\n</script>\n<p>{f()}</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tfunction a() {\n\t\tfunction b() {\n\t\t\tlet $$slots = 1;\n\t\t\treturn $$slots;\n\t\t}\n\t\treturn b();\n\t}\n</script>\n<p>{a()}</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tlet o = {};\n\tconst { $$slots } = o;\n</script>\n<p>x</p>",
        "$$slots",
    );
    // A function / class declaration id, a function-expression id, and a catch
    // parameter are bindings too.
    assert_unsupported(
        "<script>\n\tfunction $$slots() {}\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tclass $$slots {}\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tconst f = function $$slots() {};\n</script>\n<p>{f.name}</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tlet r = 0;\n\ttry {\n\t\tr = 1;\n\t} catch ($$slots) {\n\t\tr = 2;\n\t}\n</script>\n<p>{r}</p>",
        "$$slots",
    );
    // An import's local — the oracle's message names imports explicitly.
    assert_unsupported(
        "<script>\n\timport { $$slots } from './x.js';\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\timport $$slots from './x.js';\n</script>\n<p>x</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\timport * as $$slots from './x.js';\n</script>\n<p>x</p>",
        "$$slots",
    );
    // The rule is the `$` prefix, not the name — the same positions refuse for
    // any `$`-prefixed binding. These cover the positions the pre-existing
    // reference-side refusal never reached (a class declaration id, an import
    // local), which over-accepted for EVERY name before the binding rule
    // existed, not only for `$$slots`.
    assert_unsupported("<script>\n\tclass $Foo {}\n</script>\n<p>x</p>", "$Foo");
    assert_unsupported(
        "<script>\n\timport { $foo } from './x.js';\n</script>\n<p>x</p>",
        "$foo",
    );
    assert_unsupported(
        "<script>\n\tfunction $$props() {}\n</script>\n<p>x</p>",
        "$$props",
    );
}

#[test]
fn compile_refuses_dollar_prefixed_binding_on_the_rewrite_path() {
    // `script_rewrite::rewrite_declaration` rewrites a top-level instance-script
    // declaration instead of guard-walking it, so it needs the binding rule at
    // its OWN chokepoint — the guard walk never applies it here. Two halves,
    // both probe-verified oracle-rejected (`dollar_prefix_invalid`), both
    // over-accepted before the check moved ahead of the rune dispatch:
    //
    // (a) a non-rune declarator sharing a statement with a rune one. Its id took
    // the STORE-READ exemption, which this path's `WalkCtx` enables — so it
    // needed the base name (`x`) to be a binding, and any plain import supplies
    // that. Kept per rune kind: the trigger is the shared-declaration rune path,
    // and each kind reaches it.
    for init in ["$state(1)", "$state.raw(1)", "$props()", "$derived(1 + 1)"] {
        assert_unsupported(
            &format!(
                "<script>\n\timport {{ x }} from './s.js';\n\tlet a = {init}, $x = 2;\n</script>\n<p>{{$x}}</p>"
            ),
            "$x",
        );
    }
    // (b) the rune declarator's OWN id, which was not walked at all — so unlike
    // (a) it never depended on a bound base name, and no template read is needed
    // to reach it.
    for init in [
        "$state(1)",
        "$state.raw(1)",
        "$derived(1 + 1)",
        "$props.id()",
    ] {
        assert_unsupported(
            &format!("<script>\n\tlet $x = {init};\n</script>\n<p>hi</p>"),
            "$x",
        );
    }
}

#[test]
fn compile_refuses_dollar_prefixed_class_expression_id() {
    // A class EXPRESSION id is the one `$`-prefixed binding name the oracle
    // ACCEPTS (it declares no binding, so `dollar_prefix_invalid` never fires),
    // and tsv over-refuses it deliberately. Reproducing the oracle here means
    // reproducing two defects: its reference analysis is name-based and counts
    // the id as a READ, so `class $$slots {}` injects `$.sanitize_slots`, and
    // `class $Foo {}` drives its store rewrite to emit `class $.store_get(…) {}`
    // — invalid JS. Pinned so the over-refusal is a decision, not a drift.
    assert_unsupported(
        "<script>\n\tconst C = class $$slots {};\n</script>\n<p>{C.name}</p>",
        "$$slots",
    );
    assert_unsupported(
        "<script>\n\tconst C = class $Foo {};\n</script>\n<p>{C.name}</p>",
        "$Foo",
    );
}

#[test]
fn compile_refuses_dollar_prefixed_binding_the_oracle_exempts_by_depth() {
    // Two of the guarded positions are oracle-rejected only at the TOP LEVEL: a
    // function-expression id and a catch-clause parameter both declare through
    // `scope.js:695`, the one `validate_identifier_name` call path that passes
    // `function_depth`, and the instance script's top-level scope sits at depth
    // 1. So the oracle ACCEPTS these three inside a function body
    // (probe-verified against the pinned compiler) and tsv refuses them anyway.
    //
    // The over-refusal is deliberate, not a missing depth check. tsv's
    // `WalkCtx::fn_depth` is NOT the oracle's `function_depth`: the oracle's
    // non-porous increment happens at a function's `BlockStatement`
    // (`scope.js:1174-1188`), so an expression-bodied arrow does not increment
    // it and does increment tsv's. Gating on `fn_depth == 0` would compile the
    // oracle-REJECTED `const h = () => function $$slots() {}` — an
    // over-acceptance, which is a refusal-contract bug, where this is only an
    // over-refusal. Pinned so the trade is a decision, not a drift.
    for (source, name) in [
        (
            "<script>\n\tfunction f() {\n\t\ttry {\n\t\t\tnull;\n\t\t} catch ($$slots) {\n\t\t\treturn $$slots;\n\t\t}\n\t}\n</script>\n<p>{f()}</p>",
            "$$slots",
        ),
        (
            "<script>\n\tfunction f() {\n\t\treturn function $$slots() {};\n\t}\n</script>\n<p>{f().name}</p>",
            "$$slots",
        ),
        (
            "<script>\n\timport { x } from './store.js';\n\tfunction f() {\n\t\ttry {\n\t\t\tnull;\n\t\t} catch ($x) {\n\t\t\treturn $x;\n\t\t}\n\t}\n</script>\n<p>{f()}{$x}</p>",
            "$x",
        ),
    ] {
        assert_unsupported(source, name);
    }
}

#[test]
fn compile_allows_dollar_prefixed_binding_positions_the_oracle_exempts() {
    // The complement of the rule above, and the real risk of adding it: the
    // oracle EXEMPTS a `$`-prefixed name in a parameter position
    // (`declaration_kind` `param`/`rest_param`) and in a template binding
    // (declared in scopes past its `function_depth <= 1` gate), so over-refusing
    // there would break components the oracle compiles. Probe-verified
    // oracle-accepted; the `$$slots` cases also carry a genuine `$$slots`
    // reference, the cross the name-keyed carve-out used to conflate.
    //
    // ⚠️ Read the coverage narrowly: every case below is named `$$slots`, and
    // that is not a stylistic choice — it is the ONLY `$`-prefixed name this
    // test CAN cover. For any other name in these same oracle-exempt positions
    // (`function f($p)`, `{#each … as $x}`, `{@const $c = 1}`) tsv already
    // over-refuses, via the unrelated pre-existing reference-side store
    // refusal (`Refusal::DollarPrefixedIdentifier` — a `$`-prefixed read whose
    // base is not a binding), which fires long before any binding rule. That
    // over-refusal is pre-existing and contract-safe (refusing is never a
    // refusal-contract bug), but it means these seven cases pin the exemption
    // only for the one name the reference carve-out already lets through. They
    // are NOT evidence that the binding rule leaves the oracle's exempt
    // positions generally intact — nothing here can be, until that store
    // refusal narrows.
    for source in [
        "<script>\n\tfunction f($$slots) {\n\t\treturn $$slots;\n\t}\n</script>\n{#if $$slots.a}<p>{f(1)}</p>{/if}",
        "<script>\n\tconst g = ($$slots) => $$slots;\n</script>\n<p>{g(1)}</p>",
        "<script>\n\tfunction f(...$$slots) {\n\t\treturn $$slots.length;\n\t}\n</script>\n<p>{f(1)}</p>",
        "{#each [1] as n}{@const $$slots = n}<p>{$$slots}</p>{/each}",
        "{#each [1] as $$slots}<p>{$$slots}</p>{/each}",
        "{#await Promise.resolve(1) then $$slots}<p>{$$slots}</p>{/await}",
        "{#snippet s($$slots)}<p>{$$slots}</p>{/snippet}",
    ] {
        assert!(
            compile(source, &CompileOptions::default()).is_ok(),
            "expected compile to succeed for:\n{source}"
        );
    }
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
fn compile_self_closing_component() {
    // A plain component invocation compiles to `Name($$renderer, {})`. As the
    // sole root child it is standalone — no trailing `<!---->` anchor.
    let js = compile_js("<Foo />");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tFoo($$renderer, {});\n\
             }\n"
    );
}

#[test]
fn compile_component_prop_value_shapes() {
    // string → 's'; expr(prop) → the reference; shorthand `{value}` collapses
    // to `value`; boolean → `true`. The component declares props, so `$$props`
    // is injected, but no `$$renderer.component` wrapper (a bare prop
    // reference is not `needs_context`-unsafe).
    let js = compile_js(
        "<script>let { x, value } = $props();</script>\n<Foo a=\"s\" b={x} {value} disabled />",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer, $$props) {\n\
             \tlet { x, value } = $$props;\n\
             \tFoo($$renderer, { a: 's', b: x, value, disabled: true });\n\
             }\n"
    );
}

#[test]
fn compile_component_shorthand_collapses_when_names_match() {
    // `b={b}` → `{ b }` (key === value identifier); `b={x}` → `{ b: x }`.
    let js = compile_js("<script>let { b } = $props();</script>\n<Foo b={b} />");
    assert!(js.contains("Foo($$renderer, { b });"), "{js}");
    let js = compile_js("<script>let { b } = $props();</script>\n<Foo a={b} />");
    assert!(js.contains("Foo($$renderer, { a: b });"), "{js}");
}

#[test]
fn compile_component_derived_prop_reads_as_call() {
    // A bare `$derived` read in a prop value becomes `d()` — so a `{d}`
    // shorthand is NOT collapsed (the value is a call, not the identifier).
    let js = compile_js(
        "<script>let n = $state(1);\n\tlet d = $derived(n * 2);\n\tfunction inc() {\n\t\tn++;\n\t}</script>\n<Foo a={d} {d} />",
    );
    assert!(js.contains("Foo($$renderer, { a: d(), d: d() });"), "{js}");
}

#[test]
fn compile_component_mixed_and_string_value_semantics() {
    // Mixed text+expr → a template literal with `$.stringify`; a single static
    // text value entity-decodes but is NOT HTML-escaped (a JS value, not
    // markup); an all-fold mixed value collapses to a string literal.
    let js = compile_js("<script>let { y } = $props();</script>\n<Foo a=\"x {y} z\" />");
    assert!(
        js.contains("Foo($$renderer, { a: `x ${$.stringify(y)} z` });"),
        "{js}"
    );
    let js = compile_js("<Foo a=\"&amp; &lt; &gt;\" />");
    assert!(js.contains("Foo($$renderer, { a: '& < >' });"), "{js}");
    let js = compile_js("<script>let a = 1;\n\tlet b = 2;</script>\n<Foo t=\"x{a}y{b}\" />");
    assert!(js.contains("Foo($$renderer, { t: 'x1y2' });"), "{js}");
}

#[test]
fn compile_component_non_identifier_key_quotes() {
    let js = compile_js("<Foo data-x=\"1\" aria-label=\"hi\" />");
    assert!(
        js.contains("Foo($$renderer, { 'data-x': '1', 'aria-label': 'hi' });"),
        "{js}"
    );
}

#[test]
fn compile_component_spread_props() {
    // Consecutive props group into object literals; spreads break the run,
    // wrapping the whole thing in `$.spread_props([...])`.
    let js = compile_js("<script>let { r } = $props();</script>\n<Foo a={1} {...r} b={2} />");
    assert!(
        js.contains("Foo($$renderer, $.spread_props([{ a: 1 }, r, { b: 2 }]));"),
        "{js}"
    );
    let js = compile_js("<script>let { r, s } = $props();</script>\n<Foo {...r} {...s} />");
    assert!(
        js.contains("Foo($$renderer, $.spread_props([r, s]));"),
        "{js}"
    );
}

#[test]
fn compile_component_event_handler_is_a_plain_prop() {
    // Unlike an element `on*` handler (dropped), a component `onclick={fn}` is
    // an ordinary prop.
    let js = compile_js("<script>function fn() {}</script>\n<Foo onclick={fn} />");
    assert!(js.contains("Foo($$renderer, { onclick: fn });"), "{js}");
}

#[test]
fn compile_element_spread_object() {
    // A regular element `{...spread}` routes the WHOLE attribute set through one
    // fused `$.attributes({ … })` call, source order: plain attrs become object
    // properties, spreads become `...expr` elements.
    let js = compile_js(
        "<script>let props = $state({});</script>\n<div class=\"foo\" id=\"a\" {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ class: 'foo', id: 'a', ...props })"),
        "{js}"
    );
    // A single-expression event handler drops from the object; a bare boolean and
    // a `data-*` key (quoted, lowercased) survive.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state(1);</script>\n<div DataFoo={x} disabled onclick={x} {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ datafoo: x, disabled: true, ...props })"),
        "{js}"
    );
}

#[test]
fn compile_element_spread_flags_and_elision() {
    // `<input>` → the `ELEMENT_IS_INPUT` flag (4) with interior `void 0` padding.
    let js = compile_js("<script>let props = $state({});</script>\n<input {...props}/>");
    assert!(
        js.contains("$.attributes({ ...props }, void 0, void 0, void 0, 4)"),
        "{js}"
    );
    // A custom element (hyphenated tag) → `ELEMENT_PRESERVE_ATTRIBUTE_CASE` (2).
    let js = compile_js("<script>let props = $state({});</script>\n<my-elem {...props}></my-elem>");
    assert!(
        js.contains("$.attributes({ ...props }, void 0, void 0, void 0, 2)"),
        "{js}"
    );
}

#[test]
fn compile_element_spread_scope_hash_rides_second_arg() {
    // In spread mode the scope hash is NOT concatenated into the class value — it
    // rides the `css_hash` (2nd) argument.
    let out = compile(
        "<script>let props = $state({});</script>\n<div class=\"foo\" {...props}></div><style>.foo{color:red}</style>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js
            .contains("$.attributes({ class: 'foo', ...props }, 'svelte-tsvhash')"),
        "{}",
        out.js
    );
}

#[test]
fn compile_element_spread_prop_root_forces_context_wrapper() {
    // A member access rooted at a prop inside a `{...spread}` must fire the
    // `$$renderer.component` wrapper (the reference feeds `needs_context`).
    let out = compile(
        "<script>let obj = $props();</script>\n<div {...obj.foo}></div>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$$renderer.component(($$renderer) =>"),
        "prop-rooted spread must wrap: {}",
        out.js
    );
}

#[test]
fn compile_element_spread_with_class_and_style_directives() {
    // A `class:`/`style:` directive co-present with a `{...spread}` folds into the
    // `classes` (3rd) / `styles` (4th) `$.attributes` arguments — an identifier-key
    // object with shorthand collapse for `classes`, a FLAT object (no `|important`
    // partition) for `styles`.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state(1); let v = $state('');</script>\n<div class:a={x} style:color={v} {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, void 0, { a: x }, { color: v })"),
        "{js}"
    );
    // A shorthand `class:active` collapses to `{ active }`.
    let js = compile_js(
        "<script>let props = $state({}); let active = $state(true);</script>\n<div class:active {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, void 0, { active })"),
        "{js}"
    );
    // `|important` is validated but does NOT partition in spread mode.
    let js = compile_js(
        "<script>let props = $state({}); let v = $state('');</script>\n<div style:c|important={v} {...props}></div>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, void 0, void 0, { c: v })"),
        "{js}"
    );
}

#[test]
fn compile_element_spread_bind_folds_into_object() {
    // A `bind:value` folds into the object at the bind's source slot (before the
    // spread); `<input>` still sets the flags argument.
    let js = compile_js(
        "<script>let props = $state({}); let w = $state('');</script>\n<input bind:value={w} {...props}/>",
    );
    assert!(
        js.contains("$.attributes({ value: w, ...props }, void 0, void 0, void 0, 4)"),
        "{js}"
    );
    // `bind:group` synthesizes a `checked` entry; the companion `value` still emits
    // as its own object property.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state('a');</script>\n<input type=\"radio\" bind:group={x} value=\"a\" {...props}/>",
    );
    assert!(
        js.contains(
            "$.attributes({ type: 'radio', checked: x === 'a', value: 'a', ...props }, void 0, void 0, void 0, 4)"
        ),
        "{js}"
    );
    // All together: bind entry in the object, class/style args, input flags.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state(1); let v = $state(''); let w = $state('');</script>\n<input class:a={x} style:color={v} bind:value={w} {...props}/>",
    );
    assert!(
        js.contains("$.attributes({ value: w, ...props }, void 0, { a: x }, { color: v }, 4)"),
        "{js}"
    );
}

#[test]
fn compile_element_spread_directive_scoping_and_drops() {
    // A `class:` directive NAME matching a scoped selector scopes the element — the
    // hash rides the `css_hash` (2nd) argument, the classes object the 3rd.
    let js = compile_js(
        "<script>let props = $state({}); let x = $state(1);</script>\n<div class:foo={x} {...props}></div>\n<style>.foo { color: red }</style>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, 'svelte-tsvhash', { foo: x })"),
        "{js}"
    );
    // The drop family (`use:`/`transition:`) contributes nothing — a bare
    // `$.attributes({ ...props })`.
    let js =
        compile_js("<script>let props = $state({});</script>\n<div use:action {...props}></div>");
    assert!(js.contains("$.attributes({ ...props })"), "{js}");
    let js = compile_js(
        "<script>let props = $state({});</script>\n<div transition:fade {...props}></div>",
    );
    assert!(js.contains("$.attributes({ ...props })"), "{js}");
}

#[test]
fn compile_element_spread_refuses_invalid_directives() {
    // A `bind:value` on a non-`<input>` element is `bind_invalid_target` (an oracle
    // error) — the slice-3 gate still applies with a spread.
    assert_unsupported(
        "<script>let props = $state({}); let v = $state('');</script>\n<div bind:value={v} {...props}></div>",
        "bind: directive value",
    );
    // A `style:` directive with an invalid modifier still refuses.
    assert_unsupported(
        "<script>let props = $state({}); let v = $state('');</script>\n<div style:color|foo={v} {...props}></div>",
        "style: directive with an invalid modifier",
    );
    // A deferred (content-editable) bind still refuses.
    assert_unsupported(
        "<script>let props = $state({}); let h = $state('');</script>\n<div contenteditable=\"true\" bind:innerHTML={h} {...props}></div>",
        "bind: directive innerHTML",
    );
    // A legacy `on:` directive and `let:` alongside a spread stay refused (the
    // oracle drops them, but tsv declines to reproduce that).
    assert_unsupported(
        "<script>let props = $state({});</script>\n<div on:click={() => {}} {...props}></div>",
        "legacy on: directive (runes-only fence)",
    );
    assert_unsupported(
        "<script>let props = $state({});</script>\n<div let:x {...props}></div>",
        "legacy let: directive (runes-only fence)",
    );
}

#[test]
fn compile_element_spread_refuses_omit_in_ssr_binds() {
    // An `omit_in_ssr` bind (media/dimension/window binding) co-present with a
    // `{...spread}` refuses on the spread path too — consistent with the inline
    // path, and the SAFE side (the oracle rejects these shapes; tsv declines rather
    // than silently drop them). Well-formed `omit_in_ssr`+spread parity is deferred.
    let prefix =
        "<script>let props = $state({}); let w = $state(''); let x = $state(1);</script>\n";
    // `bind:files` needs `type=\"file\"` (an oracle `bind_invalid_target`).
    assert_unsupported(
        &format!("{prefix}<input bind:files={{w}} {{...props}}/>"),
        "bind: directive files",
    );
    // A dimension binding on a non-matching element (oracle `bind_invalid_target`).
    assert_unsupported(
        &format!("{prefix}<div bind:clientWidth={{x}} {{...props}}></div>"),
        "bind: directive clientWidth",
    );
    // A window binding on a non-window element (oracle `bind_invalid_target`).
    assert_unsupported(
        &format!("{prefix}<div bind:scrollX={{w}} {{...props}}></div>"),
        "bind: directive scrollX",
    );
    // A non-lvalue target on an `omit_in_ssr` bind (oracle `bind_invalid_expression`).
    assert_unsupported(
        &format!("{prefix}<div bind:clientWidth={{f()}} {{...props}}></div>"),
        "bind: directive clientWidth",
    );
}

#[test]
fn compile_component_anchor_when_not_standalone() {
    // Inside an element the component is not standalone → trailing `<!---->`.
    let js = compile_js("<div><Foo /></div>");
    assert!(
        js.contains("$$renderer.push(`<div>`);")
            && js.contains("Foo($$renderer, {});")
            && js.contains("$$renderer.push(`<!----></div>`);"),
        "{js}"
    );
    // Two sibling components each get an anchor (not a sole child).
    let js = compile_js("<Foo /><Bar />");
    assert!(
        js.contains("Foo($$renderer, {});")
            && js.contains("$$renderer.push(`<!---->`);")
            && js.contains("Bar($$renderer, {});"),
        "{js}"
    );
}

#[test]
fn compile_component_sole_block_child_is_standalone() {
    // `{#if a}<Foo/>{/if}` — the component is the branch's sole child, so it
    // reuses the branch anchor and emits no trailing `<!---->`.
    let js = compile_js("{#if a}<Foo />{/if}");
    assert!(js.contains("Foo($$renderer, {});"), "{js}");
    assert!(
        !js.contains("$$renderer.push(`<!---->`)"),
        "sole block-child component must not add an anchor: {js}"
    );
}

#[test]
fn compile_refuses_dynamic_components() {
    // A member component and a component named after a reactive binding
    // (prop / $state / $derived / each-local) all compile to the oracle's
    // truthiness guard — refused in this slice.
    assert_unsupported("<Foo.Bar />", "dynamic <Foo.Bar> component");
    assert_unsupported(
        "<script>let { Foo } = $props();</script>\n<Foo />",
        "dynamic <Foo> component",
    );
    assert_unsupported(
        "<script>let Foo = $state(null);</script>\n<Foo />",
        "dynamic <Foo> component",
    );
    assert_unsupported(
        "<script>let n = $state(1);\n\tlet Foo = $derived(n);\n\tfunction f() {\n\t\tn++;\n\t}</script>\n<Foo />",
        "dynamic <Foo> component",
    );
    // A plain local / import is NOT dynamic — it compiles.
    compile(
        "<script>const Foo = null;</script>\n<Foo />",
        &CompileOptions::default(),
    )
    .expect("plain-local component compiles");
}

#[test]
fn compile_component_children_snippet_prop() {
    // Default-slot children compile to a `children: ($$renderer) => {…}`
    // snippet prop plus `$$slots: { default: true }`. A text-first body gets
    // the `<!---->` marker.
    let js = compile_js("<Foo><p>hi</p></Foo>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \tFoo($$renderer, {\n\
             \t\tchildren: ($$renderer) => {\n\
             \t\t\t$$renderer.push(`<p>hi</p>`);\n\
             \t\t},\n\
             \t\t$$slots: { default: true }\n\
             \t});\n\
             }\n"
    );
    // Text-first children get the `<!---->` anchor inside the arrow.
    let js = compile_js("<Foo>hi <b>x</b></Foo>");
    assert!(
        js.contains("$$renderer.push(`<!---->hi <b>x</b>`);"),
        "{js}"
    );
    // An empty / whitespace-only body is NOT children (no `children` prop).
    let js = compile_js("<Foo></Foo>");
    assert_eq!(js.matches("children").count(), 0, "{js}");
    let js = compile_js("<Foo>   </Foo>");
    assert_eq!(js.matches("children").count(), 0, "{js}");
}

#[test]
fn compile_component_children_after_attrs_and_spread() {
    // The `children` prop appends after attribute props.
    let js = compile_js("<Foo a=\"x\"><p>hi</p></Foo>");
    assert!(
        js.contains("a: 'x'") && js.contains("children: ($$renderer) =>"),
        "{js}"
    );
    // With a trailing spread the children go to their own object element.
    let js = compile_js("<script>let { r } = $props();</script>\n<Foo {...r}><p>hi</p></Foo>");
    assert!(js.contains("$.spread_props(["), "{js}");
    assert!(js.contains("children: ($$renderer) =>"), "{js}");
    assert!(js.contains("$$slots: { default: true }"), "{js}");
}

#[test]
fn compile_component_named_snippet_props() {
    // A `{#snippet}` child compiles to a `function` in a wrapping block plus a
    // `{ name }` shorthand prop and a `$$slots: { name: true }` entry.
    let js = compile_js("<Foo>{#snippet header()}<h1>t</h1>{/snippet}</Foo>");
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {\n\
             \t{\n\
             \t\tfunction header($$renderer) {\n\
             \t\t\t$$renderer.push(`<h1>t</h1>`);\n\
             \t\t}\n\
             \t\tFoo($$renderer, { header, $$slots: { header: true } });\n\
             \t}\n\
             }\n"
    );
    // Multiple snippets: functions and slot entries in source order.
    let js =
        compile_js("<Foo>{#snippet a()}<b>1</b>{/snippet}{#snippet b()}<i>2</i>{/snippet}</Foo>");
    assert!(
        js.contains("Foo($$renderer, { a, b, $$slots: { a: true, b: true } });"),
        "{js}"
    );
    // A snippet named `children` keeps the `children` prop but a `default`
    // slot key.
    let js = compile_js("<Foo>{#snippet children()}<p>c</p>{/snippet}</Foo>");
    assert!(
        js.contains("Foo($$renderer, { children, $$slots: { default: true } });"),
        "{js}"
    );
}

#[test]
fn compile_component_snippet_and_default_children() {
    // Mixed named snippet + default children: the `children` arrow holds only
    // the default children (the snippet is in the wrapping block), and
    // `$$slots` carries both keys.
    let js = compile_js("<Foo>text{#snippet header()}<h1>t</h1>{/snippet}</Foo>");
    assert!(js.contains("function header($$renderer) {"), "{js}");
    assert!(js.contains("header,"), "{js}");
    assert!(js.contains("children: ($$renderer) =>"), "{js}");
    assert!(js.contains("$$renderer.push(`<!---->text`);"), "{js}");
    assert!(
        js.contains("$$slots: { header: true, default: true }"),
        "{js}"
    );
}

#[test]
fn compile_refuses_deferred_component_children() {
    // A `slot="…"` child (named slot) is a later slice; an explicit `children`
    // prop + default children is the oracle's `$$slots.default` divergence.
    assert_unsupported(
        "<Foo><p slot=\"header\">hi</p></Foo>",
        "named slot on <Foo> component",
    );
    assert_unsupported(
        "<script>let { c } = $props();</script>\n<Foo children={c}><p>hi</p></Foo>",
        "both a children prop and default children",
    );
}

#[test]
fn compile_refuses_component_directives_and_css_vars() {
    // `--custom-property` → `$.css_props`; `bind:` → a settle loop; other
    // directives are (mostly) oracle-rejected — all refused here.
    assert_unsupported(
        "<Foo --my-color=\"red\" />",
        "--custom-property attribute on <Foo> component",
    );
    assert_unsupported(
        "<script>let { v } = $props();</script>\n<Foo bind:value={v} />",
        "bind: directive on <Foo> component",
    );
}

#[test]
fn compile_carries_comments_with_component() {
    // Carried script comments alongside a component invocation carry through: the
    // component call's prop values are template-region borrows, so the comment
    // stays a leading comment of its script statement.
    let js = compile_js("<script>\n\t// note\n\tlet x = 1;\n</script>\n<Foo a={x} />");
    assert!(
        js.contains("// note"),
        "the script comment must carry through: {js}"
    );
}

#[test]
fn compile_component_prop_new_expression_wraps() {
    // A `new` in a prop value drives `needs_context` (walked in
    // needs_context.rs), wrapping the body and injecting `$$props`.
    let js = compile_js("<Foo a={new Date()} />");
    assert!(
        js.contains("$$renderer.component(($$renderer) =>")
            && js.contains("Foo($$renderer, { a: new Date() });"),
        "{js}"
    );
}

#[test]
fn compile_component_spread_member_on_prop_wraps() {
    // A member access inside a component spread must feed needs_context.
    let js = compile_js("<script>let { p } = $props();</script>\n<Foo {...p.x} />");
    assert!(
        js.contains("$$renderer.component(($$renderer) =>"),
        "spread member-on-prop must wrap: {js}"
    );
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
fn compile_refuses_unrecognized_lang() {
    // The oracle's TypeScript flag tests `lang === 'ts'` EXACTLY, so
    // `lang="typescript"` is plain JS to it — rather than compile it as JS
    // on a guess, refuse.
    assert_unsupported(
        "<script lang=\"typescript\">let x = 5;</script>\n<p>text</p>",
        "lang=\"typescript\" script",
    );
    // `generics` is an open type-parameter binding, not annotation erasure.
    assert_unsupported(
        "<script generics=\"T\">let x = 5;</script>\n<p>text</p>",
        "generics attribute",
    );
    // `lang="js"` / `lang=""` / no attribute all compile as plain JS.
    for source in [
        "<script>let x = 5;</script>\n<p>text</p>",
        "<script lang=\"js\">let x = 5;</script>\n<p>text</p>",
        "<script lang=\"\">let x = 5;</script>\n<p>text</p>",
    ] {
        compile(source, &CompileOptions::default()).expect("plain script compiles");
    }
}

#[test]
fn compile_erases_typescript() {
    // The headline Svelte-5 TypeScript idiom: a `Props` interface plus an
    // annotated `$props()` destructure.
    assert_eq!(
        compile_js(
            "<script lang=\"ts\">\n\tinterface Props {\n\t\ta: string;\n\t}\n\tlet { a }: Props = $props();\n</script>\n<p>{a}</p>"
        ),
        "import * as $ from 'svelte/internal/server';\nexport default function Input($$renderer, $$props) {\n\tlet { a } = $$props;\n\t$$renderer.push(`<p>${$.escape(a)}</p>`);\n}\n"
    );
}

#[test]
fn compile_refuses_typescript_without_lang_ts() {
    // tsv's parser is TypeScript-permissive, so it happily parses an
    // annotation in a plain `<script>`; the ORACLE hits a JS parse error
    // there. Compiling it would be an over-acceptance.
    for source in [
        "<script>let x: number = 5;</script>\n<p>text</p>",
        "<script lang=\"js\">let x = 5 as number;</script>\n<p>text</p>",
        "<script>interface P { a: string }\n\tlet x = 1;</script>\n<p>{x}</p>",
    ] {
        assert_unsupported(source, "TypeScript syntax without lang=\"ts\"");
    }
}

#[test]
fn compile_erases_typescript_in_template() {
    // A template expression is erased at its borrow point, and the erased node
    // is what the printer sees: `(x as { n: number }).n` → `x.n`, with the
    // redundant parens re-derived away by precedence (as the oracle does).
    let js = compile_js(
        "<script lang=\"ts\">\n\tlet x: any = { n: 1 };\n</script>\n<p>{(x as { n: number }).n}</p>",
    );
    assert!(
        js.contains("$.escape(x.n)"),
        "template `as` must erase, parens included: {js}"
    );
    assert!(
        !js.contains("as { n: number }"),
        "no TypeScript may survive: {js}"
    );
}

#[test]
fn compile_erases_typescript_in_template_patterns() {
    // The four pattern borrow points: `{#each}`'s context, `{#await}`'s
    // `{:then}` value, `{@const}`'s binding, and a `{#snippet}`'s parameters
    // (covered by `compile_typed_and_generic_snippet`).
    let each = compile_js(
        "<script lang=\"ts\">\n\tlet { items }: { items: number[] } = $props();\n</script>\n\
             {#each items as item: number}<li>{item}</li>{/each}",
    );
    assert!(
        each.contains("let item = each_array[$$index];"),
        "{{#each}} context annotation must erase: {each}"
    );
    let await_block = compile_js(
        "<script lang=\"ts\">\n\tlet { p }: { p: Promise<number> } = $props();\n</script>\n\
             {#await p then v: number}<p>{v}</p>{/await}",
    );
    assert!(
        await_block.contains("(v) => {"),
        "{{:then}} annotation must erase: {await_block}"
    );
    let const_tag = compile_js(
        "<script lang=\"ts\">\n\tlet { a }: { a: number } = $props();\n</script>\n\
             {#if a}{@const b: number = a}<p>{b}</p>{/if}",
    );
    assert!(
        const_tag.contains("const b = a;"),
        "{{@const}} annotation must erase: {const_tag}"
    );
}

#[test]
fn compile_template_erasure_feeds_the_fold_gate() {
    // The designed-in trap: erasing for the guard walk while the static-fold
    // gate beside it still reads the raw node yields a SILENT under-fold —
    // `1 as number` evaluating to UNKNOWN where the oracle folds `1` — a parity
    // divergence no refusal catches. The borrow point erases once, and the fold
    // gate reads the erased node.
    let js = compile_js(
        "<script lang=\"ts\">\n\tconst n: number = 1;\n</script>\n<p>{(n as number) + 1}</p>",
    );
    assert!(
        js.contains("`<p>2</p>`"),
        "a TypeScript-wrapped constant must still fold: {js}"
    );
    assert!(
        !js.contains("$.escape"),
        "a folded value must not emit an escape call: {js}"
    );
}

#[test]
fn compile_template_erasure_feeds_the_shape_predicates() {
    // The other half of the borrow-point contract: a predicate that switches on
    // an expression's VARIANT must read the erased node, or it classifies the
    // TypeScript wrapper instead of the expression the oracle prints.
    //
    // `is_standalone` (the `{@render}` anchor-elision rule) asks "is the callee
    // a plain identifier naming a local snippet?" — reading the raw
    // `(s as any)(a)` calls it dynamic and emits a `<!---->` anchor the oracle
    // elides.
    let js = compile_js(
        "<script lang=\"ts\">\n\tlet { a }: any = $props();\n</script>\n\
             {#snippet s(x)}<p>{x}</p>{/snippet}\n{@render (s as any)(a)}",
    );
    assert!(
        !js.contains("$$renderer.push(`<!---->`)"),
        "a sole local-snippet render must elide the anchor through a wrapper: {js}"
    );
    // A bare `$derived` read must still become `d()` through a wrapper.
    let derived = compile_js(
        "<script lang=\"ts\">\n\tlet { n }: any = $props();\n\tlet d = $derived(n * 2);\n</script>\n\
             <p>{d as number}</p>",
    );
    assert!(
        derived.contains("$.escape(d())"),
        "a wrapped derived read must still be called: {derived}"
    );
    // A component prop keeps the `{ n }` shorthand through a wrapper.
    let shorthand = compile_js(
        "<script lang=\"ts\">\n\timport Foo from './F.svelte';\n\n\tlet { n }: any = $props();\n</script>\n\
             <Foo n={n as number} />",
    );
    assert!(
        shorthand.contains("Foo($$renderer, { n })"),
        "a wrapped prop value must keep the shorthand: {shorthand}"
    );
}

#[test]
fn compile_render_call_shape_is_decided_before_erasure() {
    // "A `{@render}` holds a call expression" is a PARSE-time rule in the oracle
    // (`render_tag_invalid_expression`), so it is decided on the raw node — and
    // tsv's Svelte parser enforces it there too, matching the oracle exactly. A
    // wrapper around the CALL is rejected even though erasure would reveal a call
    // underneath (a `as`-cast or a `!` non-null assertion both leave the outer
    // node a non-call), so the rejection is a parse error, not a compiler
    // refusal; a wrapper around the CALLEE would leave a call and compile.
    assert_parse_rejected(
        "<script lang=\"ts\">\n\tlet { a }: any = $props();\n</script>\n\
             {#snippet s(x)}<p>{x}</p>{/snippet}\n{@render (s(a) as any)}",
        "call expressions",
    );
    assert_parse_rejected(
        "<script lang=\"ts\">\n\tlet { a }: any = $props();\n</script>\n\
             {#snippet s(x)}<p>{x}</p>{/snippet}\n{@render s(a)!}",
        "call expressions",
    );
}

#[test]
fn compile_typescript_wrapper_does_not_force_the_context_wrapper() {
    // `needs_context` walks the RAW template — the Svelte AST is never rebuilt,
    // so template erasure happens per-expression at the emitter's borrow points
    // and this analysis still sees the TypeScript wrappers. Its `is_safe_identifier`
    // port must peel them, or a member/call rooted at a SAFE binding (a plain
    // local, `$state`, a block local, a global) reads as a non-identifier root and
    // spuriously fires — wrapping the whole body in `$$renderer.component(…)` plus
    // a `$$props` parameter the oracle never emits. A silent MISMATCH, not a
    // refusal.
    for source in [
        "<script lang=\"ts\">\n\tlet local: any = { field: 1 };\n</script>\n<p>{(local!).field}</p>",
        "<script lang=\"ts\">\n\tlet local: any = { field: 1 };\n</script>\n<p>{(local as any).field}</p>",
        "<script lang=\"ts\">\n\tlet fns: any = { go: () => 1 };\n</script>\n<p>{(fns!).go()}</p>",
        "<script lang=\"ts\">\n\tlet obj = $state({ a: 1 });\n</script>\n<p>{(obj as any).a}</p>",
    ] {
        let js = compile_js(source);
        assert!(
            !js.contains("$$renderer.component("),
            "a safe root behind a TypeScript wrapper must not force the wrapper:\n{source}\n{js}"
        );
    }
}

#[test]
fn compile_rejects_snippet_rest_parameter() {
    // A **top-level** rest parameter is `snippet_invalid_rest_parameter` in the
    // oracle's analysis phase…
    assert_unsupported(
        "{#snippet foo(...xs)}<p>{xs}</p>{/snippet}\n{@render foo(1)}",
        "{#snippet} rest parameter",
    );
    // …but the oracle scans `node.parameters` itself and never descends, so a rest
    // element NESTED in a destructuring parameter is legal and compiles.
    let nested =
        compile_js("{#snippet foo({ ...rest })}<p>{rest}</p>{/snippet}\n{@render foo({})}");
    assert!(
        nested.contains("function foo($$renderer, { ...rest })"),
        "a nested rest must not be refused: {nested}"
    );
    let array =
        compile_js("{#snippet foo(a, [b, ...t])}<p>{a}{b}{t}</p>{/snippet}\n{@render foo(1, [2])}");
    assert!(
        array.contains("function foo($$renderer, a, [b, ...t])"),
        "an array-nested rest must not be refused: {array}"
    );
}

#[test]
fn compile_dropped_derived_read_is_not_refused() {
    // The derived-read rule is an emission REWRITE (`d` → `d()`), not a validity
    // rule — the oracle accepts a derived read it never emits. So a dropped region
    // must not enforce it: `{#key}`'s expression and the `{#each}` key are as
    // dropped as a `{:catch}` branch, and refusing there costs parity on shapes
    // the oracle compiles.
    let key = compile_js(
        "<script>\n\tlet { a } = $props();\n\tlet d = $derived(a * 2);\n</script>\n\
             {#key d}<p>k</p>{/key}",
    );
    assert!(key.contains("<!---->"), "{{#key}} must compile: {key}");
    compile_js(
        "<script>\n\tlet { xs, a } = $props();\n\tlet d = $derived(a);\n</script>\n\
             {#each xs as x (d)}<p>{x}</p>{/each}",
    );
    // An EMITTED pattern is not a dropped region: this emitter borrows a binding
    // pattern through untouched, so a derived read in a default value would print a
    // bare `d` where the oracle prints `d()`. That one still refuses.
    assert_unsupported(
        "<script>\n\tlet { xs, a } = $props();\n\tlet d = $derived(a);\n</script>\n\
             {#each xs as { v = d }}<p>{v}</p>{/each}",
        "read of derived binding",
    );
}

#[test]
fn compile_refuses_template_typescript_without_lang_ts() {
    // The oracle's `ts` flag is document-wide: without `lang="ts"` its parser
    // rejects TypeScript in the template too, so accepting it would be an
    // over-acceptance. Both an EMITTED borrow point…
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n</script>\n<p>{a as string}</p>",
        "TypeScript syntax without lang=\"ts\"",
    );
    // …and the SSR-DROPPED positions the erase self-check can never see (the
    // `{#key}` expression, the `{#each}` key, an event handler, and the whole
    // `{:catch}` branch).
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n</script>\n{#key a as string}<p>k</p>{/key}",
        "TypeScript syntax without lang=\"ts\"",
    );
    assert_unsupported(
        "<script>\n\tlet { items } = $props();\n</script>\n\
             {#each items as x (x.id as string)}<li>{x}</li>{/each}",
        "TypeScript syntax without lang=\"ts\"",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n</script>\n\
             <button onclick={() => (a as any)}>b</button>",
        "TypeScript syntax without lang=\"ts\"",
    );
    assert_unsupported(
        "<script>\n\tlet { p } = $props();\n</script>\n\
             {#await p then v}<p>{v}</p>{:catch e}<p>{e as string}</p>{/await}",
        "TypeScript syntax without lang=\"ts\"",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n</script>\n\
             {#snippet foo<T>(x)}<p>{x}</p>{/snippet}\n{@render foo(a)}",
        "TypeScript syntax without lang=\"ts\"",
    );
    // The destructured block-pattern forms. These were INVISIBLE to this sweep
    // until the parser stopped silently discarding a destructuring pattern's
    // annotation — a dropped node is a node no tree-walking gate can refuse.
    assert_unsupported(
        "<script>\n\tlet { p } = $props();\n</script>\n\
             {#await p then { a }: { a: number }}<p>{a}</p>{/await}",
        "TypeScript syntax without lang=\"ts\"",
    );
    assert_unsupported(
        "<script>\n\tlet { p } = $props();\n</script>\n\
             {#await p then v}<p>{v}</p>{:catch { message }: Error}<p>{message}</p>{/await}",
        "TypeScript syntax without lang=\"ts\"",
    );
    assert_unsupported(
        "<script>\n\tlet { xs } = $props();\n</script>\n\
             {#each xs as { a }: { a: number }}<p>{a}</p>{/each}",
        "TypeScript syntax without lang=\"ts\"",
    );
}

#[test]
fn dropped_fragments_are_walked() {
    // The M4 class, pinned. A fragment the emitter DISCARDS without visiting —
    // today only `{:catch}`, plus an event handler's expression — must still be
    // walked for the things the oracle decides BEFORE it chooses what to emit.
    // Dropping the region cannot make the component valid.
    //
    // This test covers the two facts carried by an EXPRESSION inside the region:
    // its reference counting (`needs_context`) and its analysis-phase errors (a
    // misplaced rune). The third fact — one the oracle reads from a node's mere
    // PRESENCE, independent of any expression — is a different walk over node
    // KINDS, pinned separately by `dropped_fragment_refuses_presence_read_nodes`.
    //
    // A new emission-dropped fragment that skips that walk fails here (and, for
    // TypeScript, in `compile_refuses_template_typescript_without_lang_ts`).

    // 1. References inside a dropped `{:catch}` still reach `needs_context`: a
    //    prop-rooted member access there forces the `$$renderer.component`
    //    wrapper, exactly as the oracle counts it.
    let js = compile_js(
        "<script>\n\tlet { p, obj } = $props();\n</script>\n\
             {#await p then v}<p>{v}</p>{:catch e}<p>{obj.field}</p>{/await}",
    );
    assert!(
        js.contains("$$renderer.component(($$renderer) => {"),
        "a prop-rooted access in a dropped {{:catch}} must still fire needs_context: {js}"
    );

    // 2. An analysis-phase error inside a dropped region still refuses — the
    //    oracle rejects `{:catch e}{$state(1)}` with `state_invalid_placement`.
    assert_unsupported(
        "<script>\n\tlet { p } = $props();\n</script>\n\
             {#await p then v}<p>{v}</p>{:catch e}<p>{$state(1)}</p>{/await}",
        "$state",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n</script>\n\
             <button onclick={() => $state(1)}>b</button>",
        "$state",
    );

    // 3. …but a shape the oracle merely DROPS must still compile: a derived read
    //    inside a dropped `{:catch}` is emitted nowhere, so the oracle accepts
    //    it. Guarding a dropped region must not over-refuse.
    let derived = compile_js(
        "<script>\n\tlet { p } = $props();\n\tlet d = $derived(1);\n</script>\n\
             {#await p then v}<p>{v}</p>{:catch e}<p>{d}</p>{/await}",
    );
    assert!(
        !derived.contains("catch"),
        "the {{:catch}} branch is dropped from SSR: {derived}"
    );
}

/// The dropped-region walk over node KINDS, and the rule that scopes it.
///
/// `guard_dropped_fragment`'s expression walk asks what a dropped region
/// *references*. It never asks what a dropped node *is* — so a fact the oracle
/// reads out of a node's mere presence reaches no guard at all. `<slot>` is that
/// fact: the oracle's phase-2 analysis records every `<slot>` in `slot_names`
/// (`2-analyze/visitors/SlotElement.js`) and phase 3 folds `slot_names.size > 0`
/// into `should_inject_props`, so a `<slot>` in a `{:catch}` widens the component
/// signature to `($$renderer, $$props)` even though SSR emits nothing from the
/// branch. Without the node walk this pins, tsv fires no refusal there and emits
/// the bare signature — a MISMATCH.
///
/// The rule is NOT "every fenced construct refuses everywhere". It is: a construct
/// refuses everywhere it can **affect the result**. A dropped region suppresses a
/// construct's emission but not a phase-2 fact keyed on its presence — and there
/// are TWO such kinds of fact, which is the part that is easy to get wrong:
///
/// - **emission** — the fact rides into the generated code (`<slot>` → the widened
///   signature, above). Measurable one construct at a time.
/// - **validation** — the fact feeds a whole-component check that can turn an
///   otherwise-valid component into a compile error (a dropped `on:` + an emitted
///   `onclick` → `mixed_event_handler_syntaxes`). Invisible in isolation: it needs
///   a SECOND construct elsewhere in the component to fire, so a per-construct
///   probe reports the dropped one as inert and is simply asking the wrong
///   question.
///
/// The third loop pins the validation axis; the second pins that everything on
/// neither axis keeps compiling, which is what makes the distinction load-bearing
/// rather than a slogan.
#[test]
fn dropped_fragment_refuses_presence_read_nodes() {
    // A `$state` promise, deliberately NOT `$props()`: the fact under test is a
    // widened component signature, so the baseline must be the bare
    // `($$renderer)` one that a `$props()` destructure would mask.
    let await_ = |catch: &str| {
        format!(
            "<script>\n\tlet p = $state(Promise.resolve(1));\n</script>\n\
             {{#await p}}a{{:then v}}b{{:catch e}}{catch}{{/await}}"
        )
    };

    // ── refuses: `<slot>`, the one presence-read construct ────────────────
    //
    // Same bucket label as the emitted path (`template node special element
    // <slot>`), so the refusal is the fence firing in a second position rather
    // than a new reason — and the corpus census keeps one bucket for the tag.
    for catch in [
        "<slot />",
        // Nested under an element: the walk must recurse into child fragments.
        "<div><slot /></div>",
        // Named — `slot_names` keys by name, but any key makes the map non-empty.
        "<slot name=\"x\" />",
        // Nested under a BLOCK inside the dropped branch: recursion has to ride
        // the structural seam, not just element children.
        "{#if e}<slot />{/if}",
        "{#each [e] as x}<slot />{/each}",
        // The other container kinds `each_child_fragment` enumerates — a
        // component's children, a `{#snippet}` body, a nested `{#await}`, and a
        // special element's own fragment. Each is a distinct arm of that match.
        "<Foo><slot /></Foo>",
        "{#snippet s()}<slot />{/snippet}",
        "{#await p}<slot />{:then q}x{/await}",
        "<svelte:boundary><slot /></svelte:boundary>",
    ] {
        assert_unsupported(&await_(catch), "template node special element <slot>");
    }

    // ── refuses: the legacy directives, on the VALIDATION axis ────────────
    //
    // A dropped `on:` is not inert. `2-analyze/visitors/OnDirective.js` sets
    // `analysis.event_directive_node` wherever the directive sits, an
    // `onclick`-style attribute on an EMITTED element sets
    // `analysis.uses_event_attributes` (`visitors/Attribute.js`), and
    // `2-analyze/index.js` errors `mixed_event_handler_syntaxes` when both are
    // set. So `{:catch}<button on:click=…>` plus a sibling `<div onclick=…>`
    // makes the oracle REJECT the component while tsv compiled it — an
    // over-acceptance, which this pins closed. It refuses unconditionally rather
    // than only in the mixed configuration: the sibling is arbitrarily far away
    // and the construct is a permanent fence, so there is nothing to buy by
    // being clever about it.
    //
    // `let:` rides along on the fence argument, NOT this one — its only oracle
    // check is the local `let_directive_invalid_placement`, which reads its
    // parent and writes no whole-component field. It is genuinely inert here and
    // refuses anyway, to keep the fenced pair in one census bucket.
    for (catch, refusal) in [
        ("<button on:click={() => e}>x</button>", "on:"),
        ("<div let:x>y</div>", "let:"),
        // Nested, and on a component / special element — the directive hangs off
        // an attribute list, so the walk has to reach every host that has one.
        ("<div><button on:click={() => e}>x</button></div>", "on:"),
        ("<Foo on:click={() => e} />", "on:"),
        ("<Foo let:x>y</Foo>", "let:"),
        ("{#if e}<button on:click={() => e}>x</button>{/if}", "on:"),
        (
            "<svelte:boundary><i on:click={() => e}>x</i></svelte:boundary>",
            "on:",
        ),
    ] {
        assert_unsupported(
            &await_(catch),
            &format!("legacy {refusal} directive (runes-only fence)"),
        );
    }

    // The full mixed configuration, verbatim — the shape the isolated probes
    // report as inert and the oracle rejects. `await_` cannot express it (the
    // sibling sits OUTSIDE the block), so it is spelled out.
    assert_unsupported(
        "<script>\n\tlet p = $state(Promise.resolve(1));\n</script>\n\
         {#await p}a{:then v}b{:catch e}<button on:click={() => e}>x</button>{/await}\n\
         <div onclick={() => p}>y</div>",
        "legacy on: directive (runes-only fence)",
    );

    // ── must NOT over-refuse: everything else at the same position ────────
    //
    // Each of these is emission-refused in an EMITTED position, and each reaches
    // parity with the oracle inside `{:catch}` today. Refusing them to satisfy a
    // uniform "fenced refuses everywhere" rule would trade correct output for
    // nothing.
    //
    // This set is clean on BOTH axes, and the validation half is verified by
    // reading the writers rather than by probing: the whole-component fields a
    // phase-2 validation reads are `slot_names`, `uses_slots`, `uses_render_tags`,
    // `event_directive_node`, `uses_event_attributes`, and `snippets` — written
    // ONLY by `SlotElement` / an `$$slots` `Identifier` / `RenderTag` /
    // `OnDirective` / an event `Attribute` / `SnippetBlock`. No construct below
    // writes any of them, so no combination can make one of them matter.
    // (`<svelte:boundary>` is not even fenced — a first-class Svelte 5 feature and
    // the next implementation target — so refusing it would actively obstruct
    // that work.)
    for catch in [
        "<svelte:boundary><i>x</i></svelte:boundary>",
        "<svelte:component this={e} />",
        "<svelte:element this=\"div\" />",
        // `<svelte:self>` only in a nesting the oracle permits — bare inside
        // `{:catch}` it is `svelte_self_invalid_placement`, so unreachable.
        "{#if e}<svelte:self />{/if}",
        // `<svelte:fragment>` and a `slot=\"…\"` child likewise want a component
        // parent; the named-slot fence is the CONSUMER half of the slot system
        // whose declaring half (`<slot>`) refuses two loops above.
        "<Foo><svelte:fragment slot=\"x\">y</svelte:fragment></Foo>",
        "<Foo><p slot=\"x\">y</p></Foo>",
    ] {
        let js = compile_js(&await_(catch));
        assert!(
            js.contains("export default function Input($$renderer) {"),
            "a dropped `{{:catch}}` {catch} must compile with the bare signature: {js}"
        );
    }
}

#[test]
fn compile_ssr_inert_special_elements() {
    // `<svelte:window>`/`<svelte:body>`/`<svelte:document>` are SSR-inert: their
    // events/binds are client-only, so the oracle emits NOTHING for them. A bare
    // one leaves only the empty exported function.
    assert_eq!(
        compile_js("<svelte:window />"),
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {}\n"
    );
    // Beside real content: the content still emits, the window drops (its only
    // template output is the sibling's push — no window marker).
    let beside = compile_js("<svelte:window />\n<p>real</p>");
    assert!(
        beside.contains("$$renderer.push(`<p>real</p>`)") && !beside.contains("svelte:window"),
        "window drops, sibling content emits: {beside}"
    );
    // The attribute expressions are still WALKED by needs_context: a prop-rooted
    // member in a window handler fires the `$$renderer.component` wrapper, exactly
    // as the oracle counts it.
    let wrapped = compile_js(
        "<script>\n\tlet { p } = $props();\n</script>\n<svelte:window onkeydown={p.method} />",
    );
    assert!(
        wrapped.contains("$$renderer.component(($$renderer) => {"),
        "a prop-rooted access in a window handler must fire needs_context: {wrapped}"
    );
    // A `bind:` marks its target reassigned, so a later `{y}` read stays dynamic
    // (not folded to its initial value).
    let bound = compile_js(
        "<script>\n\tlet y = $state(0);\n</script>\n<svelte:window bind:scrollY={y} />{y}",
    );
    assert!(
        bound.contains("$.escape(y)"),
        "a window bind must keep a later read dynamic (not folded): {bound}"
    );
    // A stray rune inside a window attribute still refuses (the oracle rejects it
    // as `state_invalid_placement` at analysis).
    assert_unsupported("<svelte:window onkeydown={$state(0)} />", "$state");

    // A valid mix of MODERN-runes attributes compiles to nothing: a modern event
    // attribute (guard-dropped), two whitelisted binds (`focused`/`innerWidth`),
    // and a `class:` directive — all oracle-accepted, all dropped from SSR output,
    // so the body is just the (rewritten) script declarations, no window markup.
    let combined = compile_js(
        "<script>\n\tlet f = $state(0);\n\tlet w = $state(0);\n\tlet x = $state(false);\n</script>\n\
         <svelte:window onclick={() => {}} bind:focused={f} bind:innerWidth={w} class:c={x} />",
    );
    assert!(
        !combined.contains("$$renderer.push") && !combined.contains("svelte:window"),
        "a valid inert element with modern attrs compiles to nothing: {combined}"
    );

    // A whitelisted bind with a VALID target compiles (dropped): a `$state`-rooted
    // lvalue for a normal bind (`innerWidth`), and ANY lvalue for `bind:this` (no
    // `$state` gate — even an uninitialized `let el`), matching the regular-element
    // fork.
    assert!(
        compile(
            "<script>\n\tlet s = $state(0);\n\tlet el;\n</script>\n\
             <svelte:window bind:innerWidth={s} bind:this={el} />",
            &CompileOptions::default(),
        )
        .is_ok(),
        "a whitelisted bind with a valid $state / bind:this target must compile"
    );

    // The no-op drop family is oracle-accepted on these elements and guard-dropped
    // (`class:`/`style:`/`use:`/`transition:`/`in:`/`out:`/`animate:`/`{@attach}`).
    for attr in [
        "class:c={ok}",
        "style:color={ok}",
        "use:ok",
        "transition:ok",
        "in:ok",
        "out:ok",
        "animate:ok",
        "{@attach ok}",
    ] {
        let src = format!("<script>\n\tlet ok = 0;\n</script>\n<svelte:window {attr} />");
        assert!(
            compile(&src, &CompileOptions::default()).is_ok(),
            "drop-family directive must compile on an inert element: {attr}"
        );
    }
}

#[test]
fn compile_refuses_invalid_ssr_inert_special_elements() {
    // Invalid-input shapes the oracle rejects at analysis; tsv's parser accepts
    // them, so the compiler must refuse (never emit nothing for oracle-rejected
    // input, which would surface as a corpus OVER-ACCEPTANCE).
    //
    // PLACEMENT: legal only at the component root — nested inside an element/block/
    // snippet is `svelte_meta_invalid_placement`.
    assert_unsupported(
        "<div><svelte:window onkeydown={() => {}} /></div>",
        "must be a top-level element",
    );
    assert_unsupported(
        "{#if true}<svelte:body use:act />{/if}",
        "must be a top-level element",
    );
    // DUPLICATE: at most one of each kind (`svelte_meta_duplicate`).
    assert_unsupported(
        "<svelte:window /><svelte:window />",
        "duplicate <svelte:window> element",
    );
    // Different kinds side-by-side are fine (not a duplicate).
    assert_compiles("<svelte:window /><svelte:body />");

    // CHILDREN: `disallow_children` — these cannot have children
    // (`svelte_meta_invalid_content`). tsv's parser DOES parse them into the
    // fragment, so refuse.
    assert_unsupported("<svelte:window>hi</svelte:window>", "cannot have children");
    assert_unsupported(
        "<svelte:body><p>x</p></svelte:body>",
        "cannot have children",
    );

    // ILLEGAL ATTRIBUTE: only a modern event attribute (`on*={expr}`) is legal; a
    // non-event plain attribute, a string-valued handler, a bare handler, and a
    // spread refuse (`illegal_element_attribute` / `svelte_body_illegal_attribute`).
    assert_unsupported("<svelte:window class=\"x\" />", "invalid attribute");
    assert_unsupported("<svelte:window id={x} />", "invalid attribute");
    assert_unsupported("<svelte:window onkeydown=\"str\" />", "invalid attribute");
    assert_unsupported("<svelte:window onclick />", "invalid attribute");
    assert_unsupported("<svelte:window {...o} />", "invalid attribute");

    // INVALID BIND: a name outside the per-kind whitelist refuses. `bind:scrollY`
    // is window-only (`bind_invalid_target` on body); `bind:nonexistent` is not a
    // binding (`bind_invalid_name`); `bind:clientWidth` is invalid on window
    // (`bind_invalid_name`). Valid $state target isolates the NAME check.
    assert_unsupported(
        "<script>\n\tlet y = $state(0);\n</script>\n<svelte:body bind:scrollY={y} />",
        "bind: directive scrollY",
    );
    assert_unsupported(
        "<script>\n\tlet a = $state(0);\n</script>\n<svelte:window bind:nonexistent={a} />",
        "bind: directive nonexistent",
    );
    assert_unsupported(
        "<script>\n\tlet a = $state(0);\n</script>\n<svelte:window bind:clientWidth={a} />",
        "bind: directive clientWidth",
    );

    // INVALID BIND TARGET: a whitelisted NAME with a target that is not a
    // `$state`-rooted lvalue refuses — the SAME reassignable-lvalue rule regular
    // elements enforce (`validate_inert_bind_target`). A non-lvalue (call / literal),
    // a `const`, and an undefined identifier the oracle also rejects
    // (`bind_invalid_expression` / `constant_binding` / `bind_invalid_value`) — this
    // closes the target over-acceptance the blanket guard-drop had left open.
    assert_unsupported(
        "<script>\n\tlet s = $state(0);\n</script>\n<svelte:window bind:innerWidth={foo()} />",
        "bind: directive innerWidth",
    );
    assert_unsupported(
        "<svelte:window bind:innerWidth={5} />",
        "bind: directive innerWidth",
    );
    // A `const` target now refuses one step EARLIER, in the whole-component
    // `validate_assignment` port (which reaches every `bind:` the oracle's own
    // validator does, dropped regions included) — so it carries the sharper
    // `constant_assignment` bucket rather than the bind-shaped one. The
    // reassignable-lvalue rule in the bind path is unchanged and still stands
    // behind it.
    assert_unsupported(
        "<script>\n\tconst c = 1;\n</script>\n<svelte:window bind:innerWidth={c} />",
        "a constant",
    );
    assert_unsupported(
        "<svelte:window bind:innerWidth={undefinedVar} />",
        "bind: directive innerWidth",
    );

    // LEGACY DIRECTIVES: a legacy `on:` event directive and `let:` refuse
    // (`RunesOnlyFence`) — the runes-only fence, matching the regular-element
    // path. The oracle ACCEPTS `on:` here, so this is a deliberate safe
    // over-refusal, not an oracle-parity claim.
    assert_unsupported(
        "<svelte:window on:click={() => {}} />",
        "legacy on: directive (runes-only fence)",
    );
    assert_unsupported(
        "<svelte:body let:x />",
        "legacy let: directive (runes-only fence)",
    );
}

/// Pin every refused special-element kind to its OWN bucket label.
///
/// `special_element_kind_table!` expands one row set into the mapping and
/// `SPECIAL_ELEMENT_REFUSAL_KINDS` together, so a NEW kind cannot reach either
/// without the other, and pairing each pattern with its label in the row rules out
/// an index that a reorder could re-point. What none of that catches is a row
/// written with the WRONG label — which would silently relabel one tag as another.
/// That matters beyond cosmetics: `Refusal::is_deliberate_fence` keys the runes-only
/// fence set on these labels, so a swap would move a tag in or out of the
/// achievable-parity denominator.
#[test]
fn refused_special_elements_carry_their_own_bucket_label() {
    for (source, tag) in [
        (
            "<script>\n\tconst Foo = null;\n</script>\n<svelte:component this={Foo} />",
            "<svelte:component>",
        ),
        ("<svelte:self />", "<svelte:self>"),
        ("<slot />", "<slot>"),
        ("<svelte:fragment />", "<svelte:fragment>"),
        // `<svelte:boundary>` is deliberately absent — it COMPILES, so it carries
        // no `TemplateNode` label at all.
    ] {
        assert_unsupported(source, &format!("template node special element {tag}"));
    }
}

#[test]
fn compile_refuses_runtime_typescript_features() {
    // Constructs with runtime semantics an erasure would silently delete —
    // and the ones the oracle itself mis-compiles into invalid JS.
    let cases: [(&str, &str); 10] = [
        ("enum E {\n\t\tA\n\t}", "TS enum"),
        ("declare enum E {\n\t\tA\n\t}", "TS enum"),
        (
            "namespace N {\n\t\texport const v = 1;\n\t}",
            "TS namespace/module with a value member",
        ),
        (
            "class C {\n\t\tconstructor(public x: number) {}\n\t}",
            "TS parameter property",
        ),
        ("import X = require('m');", "import x = require"),
        ("const v = 1;\n\texport = v;", "export = "),
        ("export as namespace Foo;", "export as namespace"),
        (
            "abstract class A {\n\t\tabstract x: number;\n\t}",
            "abstract class property",
        ),
        (
            "class C {\n\t\taccessor x = 1;\n\t}",
            "accessor class field",
        ),
        (
            "class C {\n\t\t[key: string]: unknown;\n\t}",
            "index signature in a class body",
        ),
    ];
    for (script, what) in cases {
        assert_unsupported(
            &format!("<script lang=\"ts\">\n\t{script}\n</script>\n<p>text</p>"),
            what,
        );
    }
    // A decorator is a hard error in the oracle, TypeScript or not.
    assert_unsupported(
        "<script lang=\"ts\">\n\tfunction dec(v: any, c: any) {\n\t\treturn v;\n\t}\n\tclass C {\n\t\t@dec\n\t\tm() {}\n\t}\n</script>\n<p>text</p>",
        "decorator",
    );
    // A bodiless, non-abstract class method (an overload signature).
    assert_unsupported(
        "<script lang=\"ts\">\n\tclass C {\n\t\tm(x: number): void;\n\t\tm(x: any) {}\n\t}\n</script>\n<p>text</p>",
        "bodiless class method",
    );
}

#[test]
fn compile_drops_type_only_namespace() {
    // A namespace whose whole body erases away vanishes silently — the
    // oracle's all-type→drop / any-value→reject fork.
    assert_eq!(
        compile_js(
            "<script lang=\"ts\">\n\tnamespace N {\n\t\texport type Foo = number;\n\t}\n\tlet a = 1;\n</script>\n<p>{a}</p>"
        ),
        "import * as $ from 'svelte/internal/server';\nexport default function Input($$renderer) {\n\tlet a = 1;\n\t$$renderer.push(`<p>1</p>`);\n}\n"
    );
}

#[test]
fn compile_refuses_comment_in_erased_type_region() {
    // The refusal WINDOW runs past the erased span to the next surviving
    // token, so a comment after an erased annotation — which the oracle
    // re-anchors onto the initializer (`let x = /* c */ 1`) — is caught.
    assert_unsupported(
        "<script lang=\"ts\">\n\tlet x: number /* c */ = 1;\n</script>\n<p>{x}</p>",
        "comment inside an erased TypeScript region",
    );
    // …and so is one strictly inside an erased declaration's body.
    assert_unsupported(
        "<script lang=\"ts\">\n\tinterface Props {\n\t\t/* c */\n\t\ta: string;\n\t}\n\tlet { a }: Props = $props();\n</script>\n<p>{a}</p>",
        "comment inside an erased TypeScript region",
    );
    // A LEADING comment sits before the erased region's start — outside the
    // window — and survives, landing on the next surviving statement exactly
    // as the oracle places it.
    assert_eq!(
        compile_js(
            "<script lang=\"ts\">\n\t/** doc */\n\tinterface Props {\n\t\ta: string;\n\t}\n\tlet { a }: Props = $props();\n</script>\n<p>{a}</p>"
        ),
        "import * as $ from 'svelte/internal/server';\nexport default function Input($$renderer, $$props) {\n\t/** doc */\n\tlet { a } = $$props;\n\t$$renderer.push(`<p>${$.escape(a)}</p>`);\n}\n"
    );
}

#[test]
fn compile_refuses_comment_before_a_detached_erased_region() {
    // The window reaches BACKWARD too, for a region whose start is detached
    // from its preceding token. Without that, the printer never queries the
    // erased node's byte range (it is gone) but the ENCLOSING node's gap
    // window still spans it — so the comment prints anyway, and for
    // `implements` two windows find it and it prints TWICE.
    for source in [
        // `implements` — the keyword itself carries no span.
        "<script lang=\"ts\">\n\tinterface I {\n\t\tx: number;\n\t}\n\tclass C implements /* c */ I {\n\t\tx = 1;\n\t}\n\tlet v = new C().x;\n</script>\n<p>{v}</p>",
        // A return type, preceded by `)`.
        "<script lang=\"ts\">\n\tfunction f(a: number) /* c */ : number {\n\t\treturn a;\n\t}\n\tlet v = f(1);\n</script>\n<p>{v}</p>",
        // A `<T>` type-parameter list, preceded by the function name.
        "<script lang=\"ts\">\n\tfunction f /* c */ <T>(a: T) {\n\t\treturn a;\n\t}\n\tlet v = f(1);\n</script>\n<p>{v}</p>",
        // A `<T>` type-argument list, preceded by the callee.
        "<script lang=\"ts\">\n\tfunction f<T>(a: T) {\n\t\treturn a;\n\t}\n\tlet v = f /* c */ <number>(1);\n</script>\n<p>{v}</p>",
    ] {
        assert_unsupported(source, "comment inside an erased TypeScript region");
    }
}

#[test]
fn compile_carries_comments_through_the_context_wrapper() {
    // A comment plus `needs_context` used to print TWICE: the wrapper
    // statement's appendix span left the function body's leading-comment
    // window spanning the whole script, and the arrow's own block — anchored
    // on the same script start — swept it again. The wrapper's fictional span
    // makes the arrow's block the sole owner, which is the oracle's placement.
    assert_eq!(
        compile_js(
            "<script>\n\t/** doc */\n\tclass A {\n\t\ty = 1;\n\t}\n\tlet v = new A().y;\n</script>\n<p>{v}</p>"
        ),
        "import * as $ from 'svelte/internal/server';\nexport default function Input($$renderer, $$props) {\n\t$$renderer.component(($$renderer) => {\n\t\t/** doc */\n\t\tclass A {\n\t\t\ty = 1;\n\t\t}\n\t\tlet v = new A().y;\n\t\t$$renderer.push(`<p>${$.escape(v)}</p>`);\n\t});\n}\n"
    );
}

#[test]
fn compile_unthunks_derived_of_an_argumentless_call() {
    // The oracle's `b.thunk` runs `unthunk`, which collapses `() => f()` to
    // `f` when the callee is a bare identifier and the (empty) parameter list
    // matches the arguments — so an argument-less call passes straight
    // through. An argument, or a member callee, keeps the arrow.
    let js = compile_js(
        "<script>\n\timport { get_library, f, o } from './m.ts';\n\tconst a = $derived(get_library());\n\tconst b = $derived(f(1));\n\tconst c = $derived(o.m());\n</script>\n<p>{a}{b}{c}</p>",
    );
    assert!(js.contains("const a = $.derived(get_library);"), "{js}");
    assert!(js.contains("const b = $.derived(() => f(1));"), "{js}");
    assert!(js.contains("const c = $.derived(() => o.m());"), "{js}");
}

#[test]
fn compile_unwraps_a_jsdoc_cast() {
    // `/** @type {T} */ (expr)` is an internal-only wrapper for the cast's
    // parens. The oracle has no such node — it prints the JSDoc as a detached
    // leading comment, drops the parens, and FOLDS the inner value. Valid
    // JavaScript, so it must not trip the `lang="ts"` gate either.
    assert_eq!(
        compile_js("<script>\n\tconst x = /** @type {number} */ (1);\n</script>\n<p>{x}</p>"),
        "import * as $ from 'svelte/internal/server';\nexport default function Input($$renderer) {\n\tconst x = /** @type {number} */ 1;\n\t$$renderer.push(`<p>1</p>`);\n}\n"
    );
}

#[test]
fn compile_narrows_the_parameter_property_refusal() {
    // The oracle rejects a parameter property ONLY when it carries
    // `readonly`/an accessibility modifier AND sits in a constructor — those
    // synthesize `this.x = x`. A lone `override` is unwrapped and compiles.
    assert_unsupported(
        "<script lang=\"ts\">\n\tclass C {\n\t\tconstructor(readonly x: number) {}\n\t}\n\tlet v = new C(1).x;\n</script>\n<p>{v}</p>",
        "TS parameter property with readonly/accessibility",
    );
    let js = compile_js(
        "<script lang=\"ts\">\n\tclass B {\n\t\tx = 0;\n\t}\n\tclass C extends B {\n\t\tconstructor(override x: number) {\n\t\t\tsuper();\n\t\t}\n\t}\n\tlet v = new C(1).x;\n</script>\n<p>{v}</p>",
    );
    assert!(js.contains("constructor(x) {"), "{js}");
}

#[test]
fn compile_refuses_a_dotted_namespace() {
    // `namespace A.B { … }` nests a module declaration where the oracle's
    // strip visitor assumes a block — it throws outright, at any body content.
    assert_unsupported(
        "<script lang=\"ts\">\n\tnamespace A.B {\n\t\texport type T = number;\n\t}\n\tlet v = 1;\n</script>\n<p>{v}</p>",
        "dotted TS namespace",
    );
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
fn compile_refuses_top_level_legacy_reactive_statement() {
    // A top-level `$:` label is a legacy reactive statement — the oracle
    // rejects it in runes mode (legacy_reactive_statement_invalid), so
    // cloning it through as a dead JS label would be a silent mis-compile.
    for source in [
        "<script>let c = 1;\n\t$: doubled = c * 2;</script>\n<p>{c}</p>",
        "<script>let c = 1;\n\t$: { console.log(c); }</script>\n<p>{c}</p>",
        "<script>let c = 1;\n\t$: if (c) c = 0;</script>\n<p>{c}</p>",
    ] {
        assert_unsupported(source, "legacy reactive statement");
    }
}

#[test]
fn compile_refuses_runes_invalid_imports() {
    // The oracle's runes-mode import rules: `svelte/internal*` sources are
    // forbidden outright; `beforeUpdate`/`afterUpdate` cannot be imported
    // from `svelte`. Other `svelte` imports stay valid.
    for source in [
        "<script>import { get } from 'svelte/internal/client';</script>\n<p>x</p>",
        "<script>import * as i from 'svelte/internal';</script>\n<p>x</p>",
    ] {
        assert_unsupported(source, "import from svelte/internal");
    }
    for source in [
        "<script>import { beforeUpdate } from 'svelte';</script>\n<p>x</p>",
        "<script>import { afterUpdate as au } from 'svelte';</script>\n<p>x</p>",
    ] {
        assert_unsupported(source, "runes-invalid import");
    }
    let js = compile_js("<script>import { mount } from 'svelte';</script>\n<p>x</p>");
    assert!(
        js.contains("import { mount } from 'svelte';"),
        "valid svelte import must hoist through: {js}"
    );
}

#[test]
fn compile_clones_plain_and_nested_dollar_labels() {
    // A plain label anywhere, and a `$` label INSIDE a function, are
    // ordinary JS the oracle accepts and clones through verbatim — only
    // the top-level `$:` form is the legacy reactive statement.
    let js = compile_js(
        "<script>\n\touter: for (let i = 0; i < 1; i += 1) {\n\t\tbreak outer;\n\t}\n</script>\n<p>x</p>",
    );
    assert!(
        js.contains("outer: for"),
        "plain label must clone through: {js}"
    );
    let js = compile_js(
        "<script>\n\tlet c = 1;\n\tfunction f() {\n\t\t$: y = c;\n\t\treturn y;\n\t}\n</script>\n<p>{c}</p>",
    );
    assert!(
        js.contains("$: y = c;"),
        "nested $ label must clone through: {js}"
    );
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
fn compile_class_clsx_rule() {
    // The oracle's needs_clsx rule (oracle-probed): only a BARE
    // `class={expr}` wraps in $.clsx, and only when the expression is not
    // a Literal, TemplateLiteral, or ESTree BinaryExpression — logical
    // operators are LogicalExpression there and DO wrap. The quoted form
    // `class="{expr}"` is a one-chunk array in the oracle's AST and NEVER
    // wraps. (Quoted shapes live here, not in a fixture — prettier strips
    // the redundant quotes from fixture inputs.)
    let wraps = |src: &str| compile_js(src).contains("$.clsx(");
    // Bare: identifier / conditional / logical / object / array wrap.
    assert!(wraps(
        "<script>let a = `f`;</script>\n<div class={a}></div>"
    ));
    assert!(wraps(
        "<script>let { x } = $props();</script>\n<div class={x ? `a` : `b`}></div>"
    ));
    assert!(wraps(
        "<script>let { x } = $props();</script>\n<div class={x ?? `a`}></div>"
    ));
    assert!(wraps(
        "<script>let { x } = $props();</script>\n<div class={{ active: x }}></div>"
    ));
    assert!(wraps(
        "<script>let { x } = $props();</script>\n<div class={[x, `b`]}></div>"
    ));
    // Bare exclusions: template literal / arithmetic binary / number literal.
    assert!(!wraps(
        "<script>let { x } = $props();</script>\n<div class={`a ${x}`}></div>"
    ));
    assert!(!wraps(
        "<script>let { x } = $props();</script>\n<div class={x + ` y`}></div>"
    ));
    assert!(!wraps("<div class={5}></div>"));
    // Quoted: never wraps, regardless of expression shape.
    assert!(!wraps(
        "<script>let a = `f`;</script>\n<div class=\"{a}\"></div>"
    ));
    assert!(!wraps(
        "<script>let { x } = $props();</script>\n<div class=\"{{ active: x }}\"></div>"
    ));
    // Non-class dynamic attributes never wrap.
    assert!(!wraps(
        "<script>let a = `f`;</script>\n<div title={a}></div>"
    ));
}

#[test]
fn compile_class_directive_basic() {
    // A `class:` directive on a regular element fuses with the authored `class`
    // attribute into `$.attr_class(base, void 0, { name: expr })` (the oracle's
    // `build_attr_class`). The directive key is a (canonicalized) identifier and
    // the value is the borrowed expression.
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div class=\"foo\" class:active={x}>text</div>",
    );
    assert!(
        js.contains("`<div${$.attr_class('foo', void 0, { active: x })}>text</div>`"),
        "{js}"
    );
}

#[test]
fn compile_class_directive_synthetic_and_shorthand() {
    // No authored `class`: the synthetic empty `''` base, and the fused call
    // emits after all plain attributes. A shorthand `class:active` carries the
    // auto-generated identifier as its value (`{ active: active }`, not collapsed).
    let js = compile_js("<script>let active = $state(true);</script>\n<div class:active>x</div>");
    assert!(
        js.contains("`<div${$.attr_class('', void 0, { active: active })}>x</div>`"),
        "{js}"
    );
}

#[test]
fn compile_class_directive_ordering() {
    // Plain attributes emit inline in source order; the synthetic-`class` fused
    // call emits at the END (after `id` and `title`).
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div id=\"a\" class:x={x} title=\"b\">t</div>",
    );
    assert!(
        js.contains("`<div id=\"a\" title=\"b\"${$.attr_class('', void 0, { x: x })}>t</div>`"),
        "{js}"
    );
    // An authored `class` after the directive: the fused call takes the `class`
    // slot (before the later `id`).
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div class:x={x} class=\"c\" id=\"a\">t</div>",
    );
    assert!(
        js.contains("`<div${$.attr_class('c', void 0, { x: x })} id=\"a\">t</div>`"),
        "{js}"
    );
}

#[test]
fn compile_class_directive_scoping() {
    // Scoped via a static-class token: the hash concatenates into the string base.
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div class=\"foo\" class:active={x}>t</div>\n<style>.foo { color: red }</style>",
    );
    assert!(
        js.contains("$.attr_class('foo svelte-tsvhash', void 0, { active: x })"),
        "static-token scope concat: {js}"
    );
    // Scoped via the directive NAME: the empty base concatenates to just the hash.
    let js = compile_js(
        "<script>let x = $state(true);</script>\n<div class:active={x}>t</div>\n<style>.active { color: red }</style>",
    );
    assert!(
        js.contains("$.attr_class('svelte-tsvhash', void 0, { active: x })"),
        "directive-name scope: {js}"
    );
    // Scoped with a dynamic base: the hash rides the 2nd argument.
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div class={w} class:foo={x}>t</div>\n<style>.foo { color: red }</style>",
    );
    assert!(
        js.contains("$.attr_class($.clsx(w), 'svelte-tsvhash', { foo: x })"),
        "dynamic-base scope: {js}"
    );
}

#[test]
fn compile_class_directive_mixed_class_refuses() {
    // A `class:` directive alongside a mixed-value `class="a {b}"` attribute is
    // deferred — the oracle passes the mixed value to `build_attr_class` as the
    // base, a shape this slice does not build.
    assert_unsupported(
        "<script>let a = $state(1); let x = $state(true);</script>\n<div class=\"a {a}\" class:active={x}>t</div>",
        "class: directive alongside a mixed-value class attribute",
    );
}

#[test]
fn compile_css_type_selector_synthesizes_class() {
    // A bare `<div>` scoped by a type selector gains a synthetic
    // `class="svelte-tsvhash"` (no class markup of its own), and the type selector
    // splices the hash after the tag name.
    let out = compile(
        "<div>x</div>\n<style>div{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("type selector compiles");
    assert!(
        out.js.contains(r#"<div class="svelte-tsvhash">x</div>"#),
        "synthetic scoped class: {}",
        out.js
    );
    assert_eq!(out.css.as_deref(), Some("div.svelte-tsvhash{ color: red }"));
}

#[test]
fn compile_css_type_selector_extends_existing_class() {
    // A type-scoped element with an authored static `class` appends the hash to
    // the existing value (the element is scoped by the type, not the class token).
    let js = compile_js("<div class=\"a\">x</div>\n<style>div{ color: red }</style>");
    assert!(
        js.contains(r#"<div class="a svelte-tsvhash">x</div>"#),
        "{js}"
    );
}

#[test]
fn compile_css_id_and_attribute_selectors() {
    // Id selector: synthetic class after the authored `id` attribute.
    let js = compile_js("<div id=\"foo\">y</div>\n<style>#foo{ color: red }</style>");
    assert!(
        js.contains(r#"<div id="foo" class="svelte-tsvhash">y</div>"#),
        "{js}"
    );
    // Attribute presence selector matches any value (here static).
    let js = compile_js("<p data-x=\"1\">y</p>\n<style>[data-x]{ color: red }</style>");
    assert!(
        js.contains(r#"<p data-x="1" class="svelte-tsvhash">y</p>"#),
        "{js}"
    );
    // Attribute value + explicit `i` flag matches case-insensitively.
    let js = compile_js("<p data-x=\"BAR\">y</p>\n<style>[data-x=\"bar\" i]{ color: red }</style>");
    assert!(
        js.contains(r#"<p data-x="BAR" class="svelte-tsvhash">y</p>"#),
        "{js}"
    );
}

#[test]
fn compile_css_universal_replaces_span() {
    // A bare `*` is REPLACED by the hash class (not appended).
    let out = compile(
        "<div>x</div>\n<style>*{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("universal compiles");
    assert_eq!(out.css.as_deref(), Some(".svelte-tsvhash{ color: red }"));
    // `*.c` appends on `.c` (only a bare trailing `*` replaces).
    let out = compile(
        "<div class=\"c\">x</div>\n<style>*.c{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("universal compound compiles");
    assert_eq!(out.css.as_deref(), Some("*.c.svelte-tsvhash{ color: red }"));
}

#[test]
fn compile_css_compound_needs_same_element() {
    // `.a.b` matches an element carrying BOTH classes.
    let out = compile(
        "<div class=\"a b\">x</div>\n<style>.a.b{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("same-element compound compiles");
    assert!(
        out.js.contains(r#"class="a b svelte-tsvhash""#),
        "{}",
        out.js
    );
    // `.a` and `.b` on DIFFERENT elements — no element carries both, so the
    // compound matches nothing and refuses (the oracle would comment-wrap it).
    assert_unsupported(
        "<div class=\"a\"><span class=\"b\">x</span></div>\n<style>.a.b{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_open_whitelist_on_details() {
    // `[open]` on `<details>` matches unconditionally (no `open` attribute needed).
    let js = compile_js("<details>x</details>\n<style>[open]{ color: red }</style>");
    assert!(
        js.contains(r#"<details class="svelte-tsvhash">x</details>"#),
        "{js}"
    );
}

#[test]
fn compile_css_type_matching_no_element_refuses() {
    // A type selector for an element that isn't present matches nothing → refuse.
    assert_unsupported(
        "<div>x</div>\n<style>span{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_combinator_selectors() {
    // Descendant: both compounds scope (each matched element gains the hash), the
    // first bump is a plain class, the second a zero-specificity `:where(...)`.
    let out = compile(
        "<div><p>hi</p></div>\n<style>div p{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("descendant compiles");
    assert!(
        out.js
            .contains(r#"<div class="svelte-tsvhash"><p class="svelte-tsvhash">hi</p></div>"#),
        "{}",
        out.js
    );
    assert_eq!(
        out.css.as_deref(),
        Some("div.svelte-tsvhash p:where(.svelte-tsvhash){ color: red }")
    );
    // Child `>`, next-sibling `+`, subsequent-sibling `~` all splice the same way.
    assert_eq!(
        compile_css("<div><p>hi</p></div>\n<style>div > p{ color: red }</style>"),
        "div.svelte-tsvhash > p:where(.svelte-tsvhash){ color: red }"
    );
    assert_eq!(
        compile_css("<a></a><b></b>\n<style>a + b{ color: red }</style>"),
        "a.svelte-tsvhash + b:where(.svelte-tsvhash){ color: red }"
    );
    assert_eq!(
        compile_css("<a></a><b></b>\n<style>a ~ b{ color: red }</style>"),
        "a.svelte-tsvhash ~ b:where(.svelte-tsvhash){ color: red }"
    );
}

#[test]
fn compile_css_combinator_block_descent_and_each_wrap() {
    // A preceding sibling reached through a `{#if}` block (block descent) still
    // matches `a + b`.
    assert_eq!(
        compile_css("{#if x}<a></a>{/if}<b></b>\n<style>a + b{ color: red }</style>"),
        "a.svelte-tsvhash + b:where(.svelte-tsvhash){ color: red }"
    );
    // The `{#each}` self-adjacency wrap-around: a later-in-source sibling is a
    // possible runtime preceding sibling.
    assert_eq!(
        compile_css("{#each xs as x}<b></b><a></a>{/each}\n<style>a ~ b{ color: red }</style>"),
        "a.svelte-tsvhash ~ b:where(.svelte-tsvhash){ color: red }"
    );
}

#[test]
fn compile_css_combinator_no_match_refuses() {
    // A combinator chain that matches no element is pruned by the oracle — tsv
    // refuses (no `<span>` for `span + b`).
    assert_unsupported(
        "<a></a><b></b>\n<style>span + b{ color: red }</style>",
        "matches no element",
    );
}

#[test]
fn compile_css_global_leading_trailing_and_bare() {
    // Leading `:global(.x) .y`: `.x` is global (no hash, wrapper stripped), `.y`
    // scopes (the first bump, plain class). The `.y` element gains the class.
    let out = compile(
        "<div class=\"y\">hi</div>\n<style>:global(.x) .y{ color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("leading :global compiles");
    assert!(out.js.contains(r#"class="y svelte-tsvhash""#), "{}", out.js);
    assert_eq!(
        out.css.as_deref(),
        Some(".x .y.svelte-tsvhash{ color: red }")
    );
    // Trailing `.a :global(.x)`: truncate drops `:global(.x)` from matching, but its
    // wrapper still strips in output; `.a` scopes.
    assert_eq!(
        compile_css("<div class=\"a\">hi</div>\n<style>.a :global(.x){ color: red }</style>"),
        ".a.svelte-tsvhash .x{ color: red }"
    );
    // A fully-global `:global(.x)` is never pruned and scopes no element.
    assert_eq!(
        compile_css("<div>hi</div>\n<style>:global(.x){ color: red }</style>"),
        ".x{ color: red }"
    );
    // A bare `:global` combinator: `:global` (and the preceding space) strips.
    assert_eq!(
        compile_css(
            "<div><span class=\"x\">hi</span></div>\n<style>div :global.x{ color: red }</style>"
        ),
        "div.svelte-tsvhash.x{ color: red }"
    );
}

#[test]
fn compile_css_specificity_bump_resets_per_comma() {
    // Bump state resets per comma `ComplexSelector`: the `.a` after the comma gets a
    // plain class again, not `:where(...)`.
    assert_eq!(
        compile_css("<div><p class=\"a\">hi</p></div>\n<style>div p, .a{ color: red }</style>"),
        "div.svelte-tsvhash p:where(.svelte-tsvhash), .a.svelte-tsvhash{ color: red }"
    );
}

#[test]
fn compile_css_refused_selector_shapes() {
    // The refuse-list held after slice 5: the `||` column combinator, the logical/
    // relational pseudos (`:is`/`:where`/`:has`/`:not`), `:root`, and bare
    // pseudo-only compounds. (The four real combinators and basic `:global` now
    // compile — see `compile_css_combinator_selectors` / `compile_css_global_*`.)
    assert_unsupported(
        "<div>x</div>\n<style>div || p{ color: red }</style>",
        "combinator",
    );
    for selector in [
        ":is(.a, .b)",
        ":where(.a)",
        ":has(.a)",
        ":not(.a)",
        ":root",
        ":hover",
    ] {
        assert_unsupported(
            &format!("<div>x</div>\n<style>{selector}{{ color: red }}</style>"),
            "unsupported css selector",
        );
    }
    // A `:global{}` global block stays refused — it is a nested rule, so it lands on
    // the nested-rule guard (global blocks are a deferred slice either way).
    assert_unsupported(
        "<div>x</div>\n<style>:global { .x { color: red } }</style>",
        "nested css rule",
    );
}

#[test]
fn compile_css_dynamic_attribute_value_match_refuses() {
    // A VALUED attribute selector matched against a same-named dynamic attribute
    // value the oracle would `get_possible_values`-enumerate (here an all-literal
    // ternary) is not ported — refuse rather than risk a false match.
    assert_unsupported(
        "<script>let c = $state(true);</script>\n<p data-x={c ? 'a' : 'b'}>y</p>\n<style>[data-x=\"z\"]{ color: red }</style>",
        "dynamic attribute value",
    );
}

#[test]
fn compile_css_class_split_matches_js_whitespace() {
    // The `~=` class token split must match JS `/\s/` exactly, not Rust's
    // `char::is_whitespace`. BOM (U+FEFF) is JS whitespace (not Rust's), so it
    // splits the value → `.foo` matches and the element scopes.
    let js =
        compile_js("<div class=\"foo\u{feff}bar\">x</div>\n<style>.foo { color: red }</style>");
    assert!(
        js.contains("class=\"foo\u{feff}bar svelte-tsvhash\""),
        "BOM must split the class token (JS \\s): {js:?}"
    );
    // NEL (U+0085) is Rust whitespace but NOT JS's, so it must NOT split →
    // `foo\u{85}bar` is one token, `.foo` does not match it (only the plain
    // `<div class="foo">` matches), so the NEL element is left unscoped.
    let js = compile_js(
        "<div class=\"foo\">a</div>\n<div class=\"foo\u{85}bar\">b</div>\n<style>.foo { color: red }</style>",
    );
    assert!(
        js.contains("class=\"foo svelte-tsvhash\">a</div>"),
        "plain foo scopes: {js:?}"
    );
    assert!(
        js.contains("class=\"foo\u{85}bar\">b</div>"),
        "NEL token must NOT match .foo (no hash): {js:?}"
    );
}

#[test]
fn compile_css_non_ascii_case_insensitive_refuses() {
    // A case-insensitive attribute match with a non-ASCII operand refuses (the
    // oracle folds case with full Unicode; tsv folds ASCII-only — a safe
    // over-refusal). Selector value, element value, and the `i` flag all reach it.
    assert_unsupported(
        "<p data-x=\"caf\u{e9}\">y</p>\n<style>[data-x=\"caf\u{e9}\" i] { color: red }</style>",
        "non-ASCII operand",
    );
    // An HTML case-insensitive attribute (`type`) with a non-ASCII value refuses
    // even without an explicit flag.
    assert_unsupported(
        "<p type=\"caf\u{e9}\">y</p>\n<style>[type=\"caf\u{e9}\"] { color: red }</style>",
        "non-ASCII operand",
    );
    // A case-SENSITIVE compare (no flag, not an HTML ci attr) is a byte test and
    // stays supported with a non-ASCII value.
    let out = compile(
        "<p data-x=\"caf\u{e9}\">y</p>\n<style>[data-x=\"caf\u{e9}\"] { color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("case-sensitive non-ASCII attribute value compiles");
    assert!(out.js.contains("class=\"svelte-tsvhash\""), "{}", out.js);
}

#[test]
fn compile_css_spread_element_scoped_by_type() {
    // A spread element scoped by a type selector carries the hash in the
    // `css_hash` (2nd) `$.attributes` argument (assume-match on the spread too).
    let js = compile_js(
        "<script>let props = $state({});</script>\n<div {...props}>x</div>\n<style>div{ color: red }</style>",
    );
    assert!(
        js.contains("$.attributes({ ...props }, 'svelte-tsvhash')"),
        "{js}"
    );
}

#[test]
fn compile_class_and_style_directive_coexist() {
    // `class:` and `style:` on one element both emit their own fused call — the
    // synthetic-`class` `$.attr_class` before the synthetic-`style` `$.attr_style`
    // (the oracle appends the synthetic empty `class` then the synthetic `style`).
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div class:a={x} style:color={w}>t</div>",
    );
    assert!(
        js.contains(
            "`<div${$.attr_class('', void 0, { a: x })}${$.attr_style('', { color: w })}>t</div>`"
        ),
        "{js}"
    );
}

#[test]
fn compile_style_directive_basic() {
    // A `style:` directive on a regular element fuses with the authored `style`
    // attribute into `$.attr_style(base, { name: value })` — TWO args, no css-hash
    // (style is never scoped).
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div style=\"x\" style:color={w}>text</div>",
    );
    assert!(
        js.contains("`<div${$.attr_style('x', { color: w })}>text</div>`"),
        "{js}"
    );
}

#[test]
fn compile_style_directive_synthetic_and_shorthand() {
    // No authored `style`: the synthetic empty `''` base, emitted after all plain
    // attributes. A shorthand `style:color` prints as object-shorthand `{ color }`
    // (the oracle's `b.id(name)` value coincides with the lowercased key).
    let js = compile_js("<script>let color = $state(1);</script>\n<div style:color>x</div>");
    assert!(
        js.contains("`<div${$.attr_style('', { color })}>x</div>`"),
        "{js}"
    );
}

#[test]
fn compile_style_directive_important_partition() {
    // Any `|important` directive → the 2-element `[ {normal}, {important} ]` array,
    // source order preserved within each group.
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div style:a={w} style:b|important={x} style:c={w}>t</div>",
    );
    assert!(
        js.contains("$.attr_style('', [{ a: w, c: w }, { b: x }])"),
        "{js}"
    );
    // All important → the normal object is empty `{}`.
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div style:a|important={w}>t</div>",
    );
    assert!(js.contains("$.attr_style('', [{}, { a: w }])"), "{js}");
}

#[test]
fn compile_style_directive_key_lowercasing_and_quoting() {
    // A hyphenated / custom property is a quoted string key; a `--custom` key keeps
    // its case, a plain name lowercases.
    let js = compile_js(
        "<script>let w = $state(1);</script>\n<div style:font-weight={w} style:--MyVar={w}>t</div>",
    );
    assert!(
        js.contains("$.attr_style('', { 'font-weight': w, '--MyVar': w })"),
        "{js}"
    );
}

#[test]
fn compile_style_directive_dynamic_base_no_clsx() {
    // A dynamic `style={expr}` base is the BARE expression — no `$.clsx` (unlike
    // `class`).
    let js = compile_js(
        "<script>let x = $state(true); let w = $state(1);</script>\n<div style={w} style:color={x}>t</div>",
    );
    assert!(js.contains("$.attr_style(w, { color: x })"), "{js}");
}

#[test]
fn compile_style_directive_invalid_modifier_refuses() {
    // Only a single `|important` is a legal modifier — any other modifier, or two
    // or more, is `style_directive_invalid_modifier` (an oracle error).
    assert_unsupported(
        "<script>let x = $state(true);</script>\n<div style:color|foo={x}>t</div>",
        "style: directive with an invalid modifier",
    );
    assert_unsupported(
        "<script>let x = $state(true);</script>\n<div style:color|important|bar={x}>t</div>",
        "style: directive with an invalid modifier",
    );
}

#[test]
fn compile_style_directive_mixed_value_refuses() {
    // A mixed-value `style:color="a {b}"` value (text + expression) is deferred.
    assert_unsupported(
        "<script>let b = $state(1);</script>\n<div style:color=\"a {b}\">t</div>",
        "style: directive with a mixed-value (text + expression) value",
    );
}

#[test]
fn compile_style_directive_mixed_base_refuses() {
    // A `style:` directive alongside a mixed-value `style="a {b}"` base is deferred.
    assert_unsupported(
        "<script>let a = $state(1); let x = $state(true);</script>\n<div style=\"a {a}\" style:color={x}>t</div>",
        "style: directive alongside a mixed-value style attribute",
    );
}

#[test]
fn compile_bind_this_omits() {
    // `bind:this` is omitted on any regular element (the oracle's early
    // `continue`) and works for any variable — no `$state` gate, nothing emitted.
    let js = compile_js("<script>let el = $state();</script>\n<div bind:this={el}>t</div>");
    assert!(js.contains("`<div>t</div>`"), "{js}");
}

#[test]
fn compile_bind_value_and_member_emit_attr() {
    // `bind:value` on `<input>` → `$.attr('value', expr)`; a member target rides
    // through (`obj.x`), a dynamic `type={x}` is fine for `value`.
    let js = compile_js("<script>let v = $state('');</script>\n<input bind:value={v}>");
    assert!(js.contains("$.attr('value', v)"), "{js}");
    let js = compile_js("<script>let obj = $state({ x: 1 });</script>\n<input bind:value={obj.x}>");
    assert!(js.contains("$.attr('value', obj.x)"), "{js}");
    let js = compile_js(
        "<script>let v = $state(''); let t = $state('text');</script>\n<input type={t} bind:value={v}>",
    );
    assert!(js.contains("$.attr('value', v)"), "{js}");
}

#[test]
fn compile_bind_checked_checkbox_emits_boolean_attr() {
    // `bind:checked` on a static `type="checkbox"` → `$.attr('checked', c, true)`.
    let js = compile_js(
        "<script>let c = $state(false);</script>\n<input type=\"checkbox\" bind:checked={c}>",
    );
    assert!(js.contains("$.attr('checked', c, true)"), "{js}");
}

#[test]
fn compile_bind_group_synthesizes_checked() {
    // `bind:group` synthesizes a `checked`: `group === value` (radio/other static
    // type) or `group.includes(value)` (checkbox). The companion `value` attribute
    // still emits at its own slot.
    let js = compile_js(
        "<script>let g = $state('a');</script>\n<input type=\"radio\" bind:group={g} value=\"a\">",
    );
    assert!(js.contains("$.attr('checked', g === 'a', true)"), "{js}");
    assert!(
        js.contains(" value=\"a\""),
        "companion value still emits: {js}"
    );
    let js = compile_js(
        "<script>let g = $state('a');</script>\n<input type=\"checkbox\" bind:group={g} value=\"a\">",
    );
    assert!(
        js.contains("$.attr('checked', g.includes('a'), true)"),
        "{js}"
    );
    // A dynamic companion `value={x}`: the synthesis reads `x` AND `value={x}`
    // still emits its own `$.attr('value', x)`.
    let js = compile_js(
        "<script>let g = $state('a'); let x = $state(1);</script>\n<input type=\"checkbox\" bind:group={g} value={x}>",
    );
    assert!(
        js.contains("$.attr('checked', g.includes(x), true)"),
        "{js}"
    );
    assert!(js.contains("$.attr('value', x)"), "{js}");
}

#[test]
fn compile_bind_group_no_companion_value_drops() {
    // No companion `value` attribute → the oracle silently drops the group bind.
    let js =
        compile_js("<script>let g = $state('a');</script>\n<input type=\"radio\" bind:group={g}>");
    assert!(js.contains("`<input type=\"radio\"/>`"), "{js}");
}

#[test]
fn compile_bind_coexists_with_class_directive() {
    // `bind:value` (inline) and `class:x` (pre-scanned, fused, synthetic slot) both
    // emit — the value attr first, then the fused class call after all plain attrs.
    let js = compile_js(
        "<script>let v = $state(''); let c = $state(false);</script>\n<input bind:value={v} class:x={c}>",
    );
    assert!(
        js.contains("$.attr('value', v)}${$.attr_class('', void 0, { x: c })}"),
        "{js}"
    );
}

#[test]
fn compile_bind_invalid_target_refuses() {
    // A `value`/`checked` bind on a non-`<input>` element, or `value` on
    // `<textarea>` — the oracle rejects the target (or the shape is unimplemented).
    assert_unsupported(
        "<script>let v = $state('');</script>\n<div bind:value={v}></div>",
        "bind: directive value",
    );
    assert_unsupported(
        "<script>let v = $state('');</script>\n<textarea bind:value={v}></textarea>",
        "bind: directive value",
    );
}

#[test]
fn compile_bind_checked_requires_static_checkbox_type() {
    // `bind:checked` requires a static `type="checkbox"` — a missing / non-checkbox
    // type is `bind_invalid_target` (an oracle error).
    assert_unsupported(
        "<script>let c = $state(false);</script>\n<input bind:checked={c}>",
        "bind: directive checked",
    );
    assert_unsupported(
        "<script>let c = $state(false);</script>\n<input type=\"radio\" bind:checked={c}>",
        "bind: directive checked",
    );
}

#[test]
fn compile_bind_group_dynamic_type_refuses() {
    // A dynamic `type={x}` with `bind:group` is `attribute_invalid_type` (an oracle
    // error) — refuse rather than over-accept.
    assert_unsupported(
        "<script>let g = $state('a'); let t = $state('radio');</script>\n<input type={t} bind:group={g} value=\"a\">",
        "bind: directive group",
    );
}

#[test]
fn compile_bind_value_bare_type_and_file_refuse() {
    // A BARE `type` with `bind:value` is `attribute_invalid_type` (an oracle error);
    // a static `type="file"` is the files trap the oracle silently drops the bind
    // for — refuse rather than emit a divergent `$.attr('value', …)`.
    assert_unsupported(
        "<script>let v = $state('');</script>\n<input type bind:value={v}>",
        "bind: directive value",
    );
    assert_unsupported(
        "<script>let v = $state('');</script>\n<input type=\"file\" bind:value={v}>",
        "bind: directive value",
    );
}

#[test]
fn compile_bind_omit_in_ssr_and_special_targets_refuse() {
    // The `omit_in_ssr` media/dimension binds, `bind:open` on `<details>`, and the
    // content-editable trio are all deferred → the collapsing `bind:` bucket.
    assert_unsupported(
        "<script>let w = $state(0);</script>\n<div bind:clientWidth={w}></div>",
        "bind: directive clientWidth",
    );
    assert_unsupported(
        "<script>let o = $state(false);</script>\n<details bind:open={o}></details>",
        "bind: directive open",
    );
    assert_unsupported(
        "<script>let h = $state('');</script>\n<div contenteditable bind:innerHTML={h}></div>",
        "bind: directive innerHTML",
    );
}

#[test]
fn compile_bind_non_state_expression_refuses() {
    // The expression-validity gate: a non-lvalue target (a call) and a bind rooted
    // at a non-`$state` binding (a `$derived`) both refuse — tsv emits only a
    // `$state`-rooted lvalue (the SAFE side of the oracle's assignable rule).
    assert_unsupported(
        "<script>let f = () => '';</script>\n<input bind:value={f()}>",
        "bind: directive value",
    );
    assert_unsupported(
        "<script>let n = $state(1); let d = $derived(n + 1);</script>\n<input bind:value={d}>",
        "bind: directive value",
    );
}

#[test]
fn compile_bind_this_non_lvalue_refuses() {
    // `bind:this` binds any variable (no `$state` gate), but the target must still
    // be an assignable lvalue — an Identifier or member chain. A non-lvalue target
    // (call, literal, logical) is the oracle's `bind_invalid_expression`; refuse
    // rather than silently omit the bind.
    assert_unsupported(
        "<script>let f = () => '';</script>\n<div bind:this={f()}></div>",
        "bind: directive this",
    );
    assert_unsupported("<div bind:this={42}></div>", "bind: directive this");
    assert_unsupported("<div bind:this={a && b}></div>", "bind: directive this");
    // A plain `let` (no `$state`) is a valid `bind:this` target and still omits, as
    // does a member-chain lvalue — both compile with no `this` attribute.
    let js = compile_js("<script>let el;</script>\n<div bind:this={el}>t</div>");
    assert!(js.contains("`<div>t</div>`"), "{js}");
    let js = compile_js("<script>let obj = {};</script>\n<div bind:this={obj.x}>t</div>");
    assert!(js.contains("`<div>t</div>`"), "{js}");
    // A `{get, set}` pair (the oracle's third valid bind form) also omits in SSR —
    // it is not an lvalue but is a legal bind target, so refuse-don't-omit would
    // over-refuse a valid component (the corpus's `bind-getter-setter-loop`).
    let js =
        compile_js("<script>let el;</script>\n<div bind:this={() => el, (v) => (el = v)}>t</div>");
    assert!(js.contains("`<div>t</div>`"), "{js}");
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
fn compile_drops_event_handler_attribute() {
    // An `on*` single-expression handler is omitted from SSR output.
    let out = compile(
        "<script>function go() {}</script><button onclick={go}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("`<button>x</button>`") && !out.js.contains("onclick"),
        "event handler not dropped: {}",
        out.js
    );
}

#[test]
fn compile_event_handler_new_forces_wrapper() {
    // A `new` inside a dropped handler still triggers the component wrapper
    // (needs_context walks the handler even though its markup is dropped).
    let out = compile(
        "<button onclick={() => new Date()}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$$renderer.component("),
        "needs_context wrapper missing: {}",
        out.js
    );
}

#[test]
fn compile_event_handler_decision_uses_raw_name() {
    // The oracle's `is_event_attribute` tests the RAW authored name
    // (case-sensitive `startsWith('on')`); lowercasing happens at emission
    // only. So `onClick` drops but `ONCLICK` emits `$.attr('onclick', …)`.
    let out = compile(
        "<script>let { h } = $props();</script><button ONCLICK={h}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.attr('onclick', h)"),
        "ONCLICK must emit, not drop: {}",
        out.js
    );
    let out = compile(
        "<script>let { h } = $props();</script><button onClick={h}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("`<button>x</button>`") && !out.js.contains("onclick"),
        "onClick must drop: {}",
        out.js
    );
    // Raw `onLoad` on a load-error element is a plain drop (the capture
    // exception matches the raw name exactly).
    let out = compile(
        "<script>let { h } = $props();</script><img onLoad={h} src=\"a\" />",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("`<img src=\"a\"/>`"),
        "onLoad on img must plain-drop: {}",
        out.js
    );
    // A mixed-value `ONCLICK` is not an event (raw test) and emits through
    // the normal interpolated-attribute path.
    let out = compile(
        "<script>let { h } = $props();</script><button ONCLICK=\"a {h}\">x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.attr('onclick'"),
        "mixed ONCLICK must emit: {}",
        out.js
    );
}

#[test]
fn compile_handler_shadow_never_masks_the_outer_fold_wrongly() {
    // A handler-local binding (param, destructured/default param, let-decl,
    // function-expr param, nested-arrow param) may own the mutation target,
    // so the outer binding goes Opaque: reads REFUSE (the script side's
    // shadow envelope) rather than fold or escape on a guess.
    for source in [
        "<script>let a = 1;</script><p>{a}</p><button onclick={(a) => a++}>x</button>",
        "<script>let a = 1;</script><p>{a}</p><button onclick={({ a }) => (a = 2)}>x</button>",
        "<script>let a = 1;</script><p>{a}</p><button onclick={(a = 1) => a++}>x</button>",
        "<script>let a = 1;</script><p>{a}</p><button onclick={() => { let a = 0; a++; }}>x</button>",
        "<script>let a = 1;</script><p>{a}</p><button onclick={() => { const f = (a) => a++; f(0); }}>x</button>",
    ] {
        assert_unsupported(source, "binding a is not statically modeled");
    }
    // The non-shadow direction still masks: `(x) => a++` reassigns the
    // OUTER `a`, so its read escapes instead of folding.
    let out = compile(
        "<script>let a = 1;</script><p>{a}</p><button onclick={(x) => a++}>x</button>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$.escape(a)"),
        "outer mutation must escape: {}",
        out.js
    );
    // Partial shadow: the shadowed name refuses only when read; the
    // non-shadowed co-mutated name still masks.
    let out = compile(
            "<script>let a = 1;\n\tlet b = 2;</script><p>{b}</p><button onclick={(a) => { a++; b++; }}>x</button>",
            &CompileOptions::default(),
        )
        .unwrap();
    assert!(
        out.js.contains("$.escape(b)"),
        "co-mutated b must escape: {}",
        out.js
    );
}

#[test]
fn compile_rejects_load_error_event_capture() {
    // `onload`/`onerror` on a load-error element needs `this.__e=event`
    // capture markup, not a clean drop.
    assert_unsupported("<img onload={h} src=\"a\" />", "load-error element");
    assert_unsupported(
        "<iframe onerror={h} src=\"a\"></iframe>",
        "load-error element",
    );
}

/// Assert `compile` refuses with any `Unsupported` reason (bucket-agnostic — a
/// durable pin that survives a later slice re-bucketing the same shape).
fn assert_refuses(source: &str) {
    let err = compile(source, &CompileOptions::default()).unwrap_err();
    assert!(
        matches!(&err, CompileError::Unsupported(_)),
        "expected Unsupported, got {err:?} for:\n{source}"
    );
}

#[test]
fn compile_const_bind_target_refuses() {
    // The oracle's `constant_binding` rejects a bind whose target IDENTIFIER is
    // `const`-declared or an import — keyed on the declaration keyword, so a
    // reactive `const c = $state(0)` is refused too. Corpus-invisible (idiomatic
    // bindables are `let`), so these tests are the only guard.
    assert_refuses("<script>const c = $state('');</script><input bind:value={c} />");
    assert_refuses("<script>const c = $state.raw('');</script><input bind:value={c} />");
    assert_refuses(
        "<script>const c = $state(false);</script><input type=\"checkbox\" bind:checked={c} />",
    );
    // `bind:this` takes any lvalue with no `$state` gate, so it needs the same
    // guard — including for a plain (non-rune) const and an import.
    assert_refuses("<script>const el = $state(null);</script><div bind:this={el}></div>");
    assert_refuses("<script>const el = null;</script><div bind:this={el}></div>");
    assert_refuses("<script>import {thing} from './m.js';</script><div bind:this={thing}></div>");
    // The SSR-inert special-element path shares the primitive.
    assert_refuses("<script>const w = $state(0);</script><svelte:window bind:innerWidth={w} />");
    // A MODULE-script const or import is unassignable on exactly the same terms —
    // the rule reads the declaration keyword, not which script declared it. Only
    // `bind:this` exposes this (the `$state`-gated binds refuse a module binding
    // anyway, since it never reaches `state_names`), and it exposes it on all
    // three bind paths.
    assert_refuses("<script module>const el = null;</script><div bind:this={el}></div>");
    assert_refuses(
        "<script module>import {thing} from './m.js';</script><div bind:this={thing}></div>",
    );
    assert_refuses(
        "<script module>const el = null;</script>\
         <svelte:element this=\"div\" bind:this={el}></svelte:element>",
    );
    assert_refuses("<script module>const el = null;</script><svelte:window bind:this={el} />");
}

#[test]
fn compile_const_bind_target_through_member_is_allowed() {
    // The oracle refuses only REBINDING a const name. Writing THROUGH one mutates
    // the object, so a member-chain target rooted at a const is accepted — its
    // `validate_no_const_assignment` tests `Identifier` and lets a
    // `MemberExpression` fall through. Walking the chain to its root here would
    // over-refuse a common shape, so this pins the boundary.
    for source in [
        "<script>const obj = $state({v: ''});</script><input bind:value={obj.v} />",
        "<script>const obj = $state({el: null});</script><div bind:this={obj.el}></div>",
        // A plain `let` target is untouched by the const gate.
        "<script>let c = $state('');</script><input bind:value={c} />",
        "<script>let el = $state(null);</script><div bind:this={el}></div>",
        // Same boundary for a MODULE-script const — the member chain writes
        // through it, so the oracle accepts and the widened set must not refuse.
        "<script module>const obj = {el: null};</script><div bind:this={obj.el}></div>",
        // A top-level class or function is `declaration_kind` 'class'/'function',
        // never 'const', so the oracle's test does not fire — in either script.
        "<script module>class C {}</script><div bind:this={C}></div>",
        "<script module>function f() {}</script><div bind:this={f}></div>",
        "<script>class C {}</script><div bind:this={C}></div>",
        "<script>function f() {}</script><div bind:this={f}></div>",
    ] {
        compile(source, &CompileOptions::default())
            .unwrap_or_else(|err| panic!("must still compile: {err:?} for:\n{source}"));
    }
}

#[test]
fn compile_optional_chain_bind_target_refuses() {
    // acorn wraps a chain containing an optional link in a `ChainExpression`, and
    // the oracle's bind-expression test admits only `Identifier` /
    // `MemberExpression` / a `{get, set}` pair — so `bind:this={o?.el}` is
    // `bind_invalid_expression`. tsv has no chain wrapper, so `bind_target_root`
    // refuses any optional link and the recursion propagates a deeper one up.
    assert_refuses("<script>const o = $state({el: null});</script><div bind:this={o?.el}></div>");
    assert_refuses(
        "<script>let o = $state({a: {b: null}});</script><div bind:this={o?.a.b}></div>",
    );
    assert_refuses(
        "<script>let o = $state({a: {b: null}});</script><div bind:this={o.a?.b}></div>",
    );
    assert_refuses("<script>let o = $state({v: ''});</script><input bind:value={o?.v} />");
    assert_refuses(
        "<script>let o = $state({w: 0});</script><svelte:window bind:innerWidth={o?.w} />",
    );
    // The non-optional chain is untouched.
    compile(
        "<script>let o = $state({el: null});</script><div bind:this={o.el}></div>",
        &CompileOptions::default(),
    )
    .expect("a plain member chain must still compile");
}

#[test]
fn compile_template_scoped_const_bind_target_over_accepts() {
    // TRACKED over-acceptance, pre-existing and shared with the regular-element
    // path: a `{@const}` name and a `{:then}`/`{:catch}` value are declared
    // `declaration_kind: 'const'` (kind `'template'`, not `'each'`), so the oracle
    // raises `constant_binding` — while `unassignable_names` sees top-level script
    // statements only and tsv compiles. Pinned as a CURRENT-BEHAVIOR test so the
    // gap is visible and flips loudly when template scopes are modeled.
    for source in [
        "<script>let p = $state(0);</script>{#await p then v}<div bind:this={v}></div>{/await}",
        "<script>let c = $state(0);</script>{#if c}{@const o = {}}<div bind:this={o}></div>{/if}",
    ] {
        compile(source, &CompileOptions::default()).unwrap_or_else(|err| {
            panic!("over-acceptance closed — update the doc residuals: {err:?} for:\n{source}")
        });
    }
}

#[test]
fn compile_duplicate_props_rune_refuses() {
    // The oracle's `props_duplicate` rejects a second `$props()`. Corpus-invisible
    // (no real component writes it), so this test is the only guard.
    assert_refuses("<script>let {a} = $props(); let {b} = $props();</script>{a}{b}");
    assert_refuses("<script>let {a} = $props(); let {b} = $props(); let {c} = $props();</script>");
    // Both declarators in ONE statement is the same duplicate.
    assert_refuses("<script>let {a} = $props(), {b} = $props();</script>{a}{b}");
    // A non-destructured second `$props()` duplicates just the same.
    assert_refuses("<script>let {a} = $props(); let rest = $props();</script>{a}");
    // A single `$props()` still compiles — the guard must not fire on one.
    compile(
        "<script>let {a} = $props();</script>{a}",
        &CompileOptions::default(),
    )
    .expect("a single $props() must still compile");
    // `$props()` and `$props.id()` are tracked separately (the oracle keeps two
    // flags), so one of each is NOT a duplicate.
    compile(
        "<script>let {a} = $props(); let i = $props.id();</script>{a}{i}",
        &CompileOptions::default(),
    )
    .expect("$props() alongside $props.id() must still compile");
}

#[test]
fn compile_use_directive_on_load_error_element_refuses() {
    // `use:` on a load-error element makes the oracle add onload/onerror capture
    // attributes (its `events_to_capture` set) — not implemented, so refuse.
    // Only `use:` (and a spread) triggers this; the other drop-family kinds drop.
    assert_unsupported("<img use:action />", "load-error element");
    assert_unsupported("<iframe use:action></iframe>", "load-error element");
    // `transition:`/`{@attach}` on the same element are a plain drop.
    let out = compile("<img transition:fade />", &CompileOptions::default()).unwrap();
    assert!(
        out.js.contains("`<img/>`"),
        "transition on img must plain-drop: {}",
        out.js
    );
    let out = compile("<img {@attach a} />", &CompileOptions::default()).unwrap();
    assert!(
        out.js.contains("`<img/>`"),
        "attach on img must plain-drop: {}",
        out.js
    );
}

#[test]
fn compile_await_in_dropped_directive_expression_refuses() {
    // The oracle rejects `await` inside a directive expression
    // (`illegal_await_expression` / the async gate); tsv's dropped-expression
    // guard refuses the top-level await, the correct analog.
    assert_unsupported("<div use:action={await f()}></div>", "top-level await");
    assert_unsupported("<div {@attach await mk()}></div>", "top-level await");
}

#[test]
fn compile_rune_in_dropped_directive_expression_refuses() {
    // A dropped directive expression is still validated: a misplaced rune is an
    // oracle analysis-phase error (`state_invalid_placement`), so tsv refuses.
    assert_unsupported("<div use:action={$state(1)}></div>", "rune $state");
    assert_unsupported("<div {@attach $derived(1)}></div>", "rune $derived");
}

#[test]
fn compile_select_family_spread_and_bind_refuse() {
    // The `<select>` trap: an empty `<select {...props}>` / `<select bind:value>`
    // routes through `$$renderer.select(...)` in the oracle, NOT `$.attributes`.
    // Spread and `bind:` are refused in this slice, so both refuse today — pin it
    // so the later spread/bind slices can't silently mis-route the select family.
    // See docs/checklist_svelte_compiler.md §select-family.
    assert_refuses("<select {...props}></select>");
    assert_refuses("<script>let v = $state('');</script><select bind:value={v}></select>");
}

#[test]
fn compile_svelte_element_const_tag_direct_child_refuses() {
    // The oracle rejects a `{@const}` as a direct `<svelte:element>` child
    // (`const_tag_invalid_placement`; a `<svelte:element>` is not among its valid
    // `{@const}` parents). Without a guard tsv would over-accept: the children
    // closure pushes a block-scope overlay (load-bearing for snippet hoisting) that
    // `emit_const_tag` reads as "inside a block". Pin the refusal.
    assert_refuses("<svelte:element this={tag}>{@const y = 1}{y}</svelte:element>");
    // A `{#snippet}` direct child stays valid (proves the guard didn't drop the
    // overlay the hoist analysis needs).
    compile(
        "<svelte:element this={tag}>{#snippet s()}x{/snippet}{@render s()}</svelte:element>",
        &CompileOptions::default(),
    )
    .expect("a {#snippet} child of <svelte:element> still compiles");
}

#[test]
fn compile_svelte_element_specific_refusals() {
    // A `bind:` other than `bind:this` refuses — the dynamic tag has no static
    // `<input>` identity, so the oracle rejects `bind:value`/etc.
    // (`bind_invalid_target`).
    assert_refuses(
        "<script>let x = $state(0);</script><svelte:element this={tag} bind:value={x} />",
    );
    // Legacy `on:`/`let:` refuse (the runes-only fence).
    assert_refuses("<svelte:element this={tag} on:click={h} />");
    // A scoping `<style>` scopes the element: a type selector matches a
    // `<svelte:element>` unconditionally, so it synthesizes the hash class and the
    // selector is used (not pruned → no `CssSelectorNoMatch`).
    let out = compile(
        "<svelte:element this={tag} /><style>div { color: red }</style>",
        &CompileOptions::default(),
    )
    .expect("scoped <svelte:element> compiles");
    assert!(
        out.js.contains(r#" class="svelte-tsvhash""#),
        "expected synthesized hash class, got: {}",
        out.js
    );
    assert!(
        out.css
            .as_deref()
            .is_some_and(|css| css.contains("div.svelte-tsvhash")),
        "expected scoped selector, got: {:?}",
        out.css
    );
    // A `bind:this` omits and the element compiles.
    compile(
        "<script>let el;</script><svelte:element this={tag} bind:this={el} />",
        &CompileOptions::default(),
    )
    .expect("bind:this on <svelte:element> compiles");
}

#[test]
fn compile_slots_reference_injects_sanitize() {
    // A `$$slots` reference injects the binding and takes `$$props`.
    let out = compile("<p>{$$slots}</p>", &CompileOptions::default()).unwrap();
    assert!(
        out.js.contains("const $$slots = $.sanitize_slots($$props)")
            && out.js.contains("function Input($$renderer, $$props)"),
        "sanitize_slots injection missing: {}",
        out.js
    );
}

#[test]
fn compile_rejects_slots_with_comments() {
    // Script comments plus the injected first statement would sweep the
    // comment windows — refused for now.
    assert_unsupported(
        "<script>\n\t// note\n\tlet x = 1;\n</script>\n<p>{x}{$$slots}</p>",
        "$$slots reference",
    );
}

#[test]
fn compile_slots_with_props_rest_renames_destructured_slots() {
    // The injected sanitize_slots const owns `$$slots`, so the rest-props
    // injection deconflicts by renaming: `$$slots: $$slots_` (a shorthand
    // `$$slots` would be a duplicate lexical declaration — invalid JS).
    let out = compile(
        "<script>let {...r} = $props();</script><p>{$$slots}{r}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("const $$slots = $.sanitize_slots($$props)")
            && out.js.contains("{ $$slots: $$slots_, $$events, ...r }"),
        "rest-props $$slots rename wrong: {}",
        out.js
    );
    // Non-destructured `let props = $props()` deconflicts the same way.
    let out = compile(
        "<script>let props = $props();</script><p>{$$slots}{props}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("{ $$slots: $$slots_, $$events, ...props }"),
        "non-destructured $$slots rename wrong: {}",
        out.js
    );
    // Without a `$$slots` reference the injection stays shorthand.
    let out = compile(
        "<script>let {...r} = $props();</script><p>{r}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("{ $$slots, $$events, ...r }"),
        "shorthand injection regressed: {}",
        out.js
    );
}

#[test]
fn compile_svelte_head_emits_head_call() {
    // `<svelte:head>` → `$.head('<hash>', $$renderer, closure)`. The hash is
    // the ported `hash("input.svelte")`.
    let out = compile(
        "<svelte:head><meta charset=\"utf-8\" /></svelte:head>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js
            .contains("$.head('4hbqx4', $$renderer, ($$renderer) =>"),
        "head call wrong: {}",
        out.js
    );
}

#[test]
fn compile_svelte_head_title_emits_title_call() {
    // `<title>` inside `<svelte:head>` → `$$renderer.title(($$renderer) => …)`,
    // hoisted before any sibling head content.
    let out = compile(
        "<svelte:head><title>Hi</title></svelte:head>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$$renderer.title(($$renderer) =>")
            && out.js.contains("$$renderer.push(`<title>Hi</title>`)"),
        "title call wrong: {}",
        out.js
    );
}

#[test]
fn compile_rejects_title_attribute() {
    // Any attribute on `<title>` is `title_illegal_attribute` in the oracle; tsv's
    // parser accepts it, so the compiler refuses.
    assert_unsupported(
        "<svelte:head><title foo=\"x\">Hi</title></svelte:head>",
        "attribute on <title>",
    );
}

#[test]
fn compile_rejects_title_invalid_content() {
    // A `<title>` child that is neither text nor `{expression}` is
    // `title_invalid_content` in the oracle; tsv's parser accepts it, so refuse.
    assert_unsupported(
        "<svelte:head><title>{#if x}a{/if}</title></svelte:head>",
        "invalid <title> content",
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

/// Assert `compile` produces server output (a durable "compiles" pin — the exact
/// bytes are covered by the compile fixtures, so only presence is asserted here).
fn assert_compiles(source: &str) {
    let out = compile(source, &CompileOptions::default())
        .unwrap_or_else(|err| panic!("expected compile, got {err:?} for:\n{source}"));
    assert!(
        out.js.contains("$$renderer"),
        "expected server output for:\n{source}\ngot:\n{}",
        out.js
    );
}

#[test]
fn compile_rejects_conflicting_transition_directives() {
    // The oracle's phase-2 placement check (shared/element.js:115-132): a
    // `transition:` claims BOTH intro and outro, `in:` only intro, `out:` only
    // outro; a channel claimed twice is `transition_duplicate`/`transition_conflict`.
    // tsv folds the whole union into one refusal — modifiers are irrelevant.
    assert_unsupported("<div transition:fade transition:fly></div>", "transition");
    assert_unsupported("<div in:fly transition:fade></div>", "transition");
    // Reverse order refuses the same.
    assert_unsupported("<div transition:fade in:fly></div>", "transition");
    assert_unsupported("<div in:a in:b></div>", "transition");
    assert_unsupported("<div out:a out:b></div>", "transition");
    assert_unsupported("<div transition:fade out:fly></div>", "transition");
    // Three directives claiming intro twice.
    assert_unsupported("<div in:a out:b in:c></div>", "transition");
    // A modifier does not change the direction, so it does not rescue the pair.
    assert_unsupported(
        "<div transition:fade|local transition:fly></div>",
        "transition",
    );
}

#[test]
fn compile_allows_legal_transition_directives() {
    // A single transition/in/out, or an in:+out: pair with no `transition:`, claims
    // each channel at most once — legal; the directive drops and the element compiles.
    assert_compiles("<div transition:fade></div>");
    assert_compiles("<div in:fly></div>");
    assert_compiles("<div out:fade></div>");
    assert_compiles("<div in:fly out:fade></div>");
}

#[test]
fn compile_rejects_invalid_animate_placement() {
    // `animate:` is legal only on the sole non-trivial child of a keyed `{#each}`
    // (shared/element.js:92-114). Every other placement refuses.
    // Duplicate `animate:` even in the sanctioned spot still refuses.
    assert_unsupported(
        "{#each xs as x (x)}<div animate:a animate:b></div>{/each}",
        "animate",
    );
    // Root (not inside any `{#each}`).
    assert_unsupported("<div animate:flip></div>", "animate");
    // Unkeyed each (missing key).
    assert_unsupported("{#each xs as x}<div animate:flip></div>{/each}", "animate");
    // A non-trivial sibling.
    assert_unsupported(
        "{#each xs as x (x)}<div animate:flip></div><span></span>{/each}",
        "animate",
    );
    // Not an immediate child — wrapped in `{#if}`.
    assert_unsupported(
        "{#each xs as x (x)}{#if x}<div animate:flip></div>{/if}{/each}",
        "animate",
    );
    // `{@html}` sibling counts as a non-trivial child.
    assert_unsupported(
        "{#each xs as x (x)}{@html s}<div animate:flip></div>{/each}",
        "animate",
    );
}

#[test]
fn compile_allows_valid_animate_placement() {
    // The sole non-trivial child of a keyed `{#each}` — comment/`{@const}`/
    // whitespace siblings are tolerated, an index-only key counts as keyed, and a
    // single `transition:` may coexist.
    assert_compiles("{#each xs as x (x)}<div animate:flip></div>{/each}");
    assert_compiles("{#each xs as x, i (i)}<div animate:flip></div>{/each}");
    assert_compiles("{#each xs as x (x)}<div animate:flip transition:fade></div>{/each}");
    assert_compiles("{#each xs as x (x)}<!--c-->\n<div animate:flip></div>{/each}");
    assert_compiles("{#each xs as x (x)}{@const y = 1}<div animate:flip></div>{/each}");
}

#[test]
fn compile_store_read_subscribes() {
    // A template `$name` read where `name` is a binding is a store
    // auto-subscription: `$.store_get(($$store_subs ??= {}), '$name', name)`, plus
    // the `var $$store_subs;` header and the `$.unsubscribe_stores` cleanup,
    // injected at the component-body level (no wrapper forced on its own).
    let out = compile(
        "<script>\n\timport { count } from './s';\n</script>\n<p>{$count}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js
            .contains("$.store_get(($$store_subs ??= {}), '$count', count)"),
        "store read: {}",
        out.js
    );
    assert!(out.js.contains("var $$store_subs;"), "subs var: {}", out.js);
    assert!(
        out.js
            .contains("if ($$store_subs) $.unsubscribe_stores($$store_subs);"),
        "unsubscribe: {}",
        out.js
    );
    assert!(
        !out.js.contains("$$renderer.component"),
        "a bare store read must not force the wrapper: {}",
        out.js
    );
}

#[test]
fn compile_store_script_reads_and_writes() {
    // Script-position reads, writes, and updates all compile now: a read →
    // `$.store_get`, an assignment → `$.store_set`, an update → `$.update_store`.
    let read = compile_js(
        "<script>\n\timport { count } from './s';\n\tconst d = $count * 2;\n</script>\n<p>{d}</p>",
    );
    assert!(
        read.contains("const d = $.store_get(($$store_subs ??= {}), '$count', count) * 2"),
        "script read: {read}"
    );
    let write = compile_js(
        "<script>\n\timport { count } from './s';\n\tfunction f() { $count = 5; }\n</script>\n<button onclick={f}>{$count}</button>",
    );
    assert!(
        write.contains("$.store_set(count, 5)"),
        "store write: {write}"
    );
    let update = compile_js(
        "<script>\n\timport { count } from './s';\n\tfunction f() { $count++; }\n</script>\n<button onclick={f}>{$count}</button>",
    );
    assert!(
        update.contains("$.update_store(($$store_subs ??= {}), '$count', count)"),
        "store update: {update}"
    );
    // A store read in CALLEE / new position (`$fn()`, `new $C()`) is rewritten too
    // (it forces the needs_context wrapper — a call rooted at the import).
    let callee = compile_js(
        "<script>\n\timport { fn } from './s';\n\tfunction f() { return $fn(); }\n</script>\n{f()}",
    );
    assert!(
        callee.contains("$.store_get(($$store_subs ??= {}), '$fn', fn)()"),
        "callee store read: {callee}"
    );
    let new_call = compile_js(
        "<script>\n\timport { C } from './s';\n\tfunction f() { return new $C(); }\n</script>\n{f()}",
    );
    assert!(
        new_call.contains("new ($.store_get(($$store_subs ??= {}), '$C', C))()"),
        "new store read: {new_call}"
    );
}

#[test]
fn compile_store_write_refuses_member_and_destructuring() {
    // A member write (`$obj.foo = 5` → `$.store_mutate`) and a destructuring write
    // (`[$count] = arr` → an IIFE) are out of scope for this slice — refuse rather
    // than emit the un-ported lowering.
    assert_unsupported(
        "<script>\n\timport { obj } from './s';\n\tfunction f() { $obj.foo = 5; }\n</script>\n<button onclick={f}>x</button>",
        "store member write",
    );
    assert_unsupported(
        "<script>\n\timport { count } from './s';\n\tfunction f(arr) { [$count] = arr; }\n</script>\n<button onclick={f}>x</button>",
        "store destructuring write",
    );
    // A member UPDATE (`$obj.foo++`) refuses the same way.
    assert_unsupported(
        "<script>\n\timport { obj } from './s';\n\tfunction f() { $obj.foo++; }\n</script>\n<button onclick={f}>x</button>",
        "store member write",
    );
}

#[test]
fn compile_store_refuses_scoped_subscription() {
    // `$count` where the base `count` is bound in a nested scope is the oracle's
    // `store_invalid_scoped_subscription` error. Refuse (name-based shadow check).
    assert_unsupported(
        "<script>\n\timport { writable } from 'svelte/store';\n\tlet count = writable(0);\n\tfunction f(count) { return $count; }\n</script>\n<p>{f}</p>",
        "not a top-level component binding",
    );
    // A base that is not a component binding at all (`$missing`) stays refused as a
    // bare `$`-prefixed identifier (the oracle's `global_reference_invalid`).
    assert_unsupported(
        "<script>\n\tfunction f(count) { return $count; }\n</script>\n<p>{f}</p>",
        "$count",
    );
    // A store read in CALLEE position with a shadowed LOCAL base refuses via the
    // same `StoreScopedSubscription` path as a bare read (the callee-exemption in
    // the guard mirrors the bare-read shadow handling). A local base keeps
    // `needs_context` out of the way — a `$fn()` rooted at an IMPORT instead
    // refuses earlier as `MemberCallAmbiguousRoot` (also a refusal, no
    // over-acceptance).
    assert_unsupported(
        "<script>\n\timport { writable } from 'svelte/store';\n\tlet fn = writable(0);\n\tfunction f(fn) { return $fn(); }\n</script>\n<p>{f}</p>",
        "not a top-level component binding",
    );
    // The coordinator's literal example — a callee whose base is a bare param
    // (not a top-level binding) — refuses as a rune call ($fn is not a store base).
    assert_unsupported(
        "<script>\n\tfunction f(fn) { return $fn(); }\n</script>\n<p>{f}</p>",
        "rune $fn",
    );
    // A genuine rune call in callee position stays refused (never exempted).
    assert_unsupported(
        "<script>\n\tlet x = $state(0);\n\tfunction f() { return $state(1); }\n</script>\n<p>{f}</p>",
        "rune",
    );
}

#[test]
fn compile_dollar_rune_is_not_a_store_read() {
    // A rune callee (`$props()`) is NOT a store read even when its base name
    // coincides with a binding (`const props = $props()`): stripping `$props` to
    // `props` and treating it as a store on the props object would spuriously force
    // the `$$renderer.component` wrapper. Regression guard for `store_read_base`.
    let out = compile(
        "<script>\n\tconst props = $props();\n</script>\n<p>text</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        !out.js.contains("$$store_subs"),
        "$props() must not mint store subscriptions: {}",
        out.js
    );
}

#[test]
fn compile_store_read_in_snippet_stays_nested() {
    // A top-level `{#snippet}` whose only hoist-blocking reference is a store read
    // must NOT hoist to module scope — its body reads the component-local
    // `$$store_subs`. (Regression: the free-var collector recorded `$count`, which
    // is not a binding name, so the store read failed to block hoisting.)
    let out = compile(
        "<script>\n\timport { count } from './s';\n</script>\n{#snippet foo()}{$count}{/snippet}{@render foo()}",
        &CompileOptions::default(),
    )
    .unwrap();
    // The snippet function nests inside the component (after `var $$store_subs;`),
    // never as a module-scope sibling of the import.
    let subs = out.js.find("var $$store_subs;").expect("subs var");
    let foo = out.js.find("function foo").expect("snippet fn");
    assert!(
        foo > subs,
        "snippet must stay nested (after $$store_subs), got:\n{}",
        out.js
    );
}

#[test]
fn compile_derived_store_reads_call() {
    // A store whose base is a `$derived` binding reads `d()` as the store value —
    // a `$derived` read is a call at every position.
    let out = compile(
        "<script>\n\tlet d = $derived(0);\n</script>\n<p>{$d}</p>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js
            .contains("$.store_get(($$store_subs ??= {}), '$d', d())"),
        "derived-base store must read d(): {}",
        out.js
    );
}

#[test]
fn compile_shadowed_store_base_refuses() {
    // A store base shadowed by a block-local (`{#each}`/`{#await}`/snippet param) is
    // not a top-level store — the oracle errors `store_invalid_scoped_subscription`,
    // so tsv refuses rather than subscribe to the block-local.
    assert_unsupported(
        "<script>\n\timport { count } from './s';\n</script>\n{#each xs as count}{$count}{/each}",
        "$count",
    );
    assert_unsupported(
        "<script>\n\timport { count } from './s';\n</script>\n{#snippet foo(count)}{$count}{/snippet}{@render foo(1)}",
        "$count",
    );
}

// ── Module scripts (`<script module>`) ─────────────────────────────────────

#[test]
fn compile_module_refuses_default_export() {
    // `export default` in a module is the oracle's `module_illegal_default_export`
    // error — refuse rather than emit.
    assert_unsupported(
        "<script module>\n\texport default 5;\n</script>\n<p>hi</p>",
        "default export in <script module>",
    );
}

#[test]
fn compile_module_refuses_state_rune() {
    // v1 defers the oracle's module `$state`→v rewrite (the corpus is module-rune-
    // free), so a module-scope rune refuses via the guard — a safe over-refusal.
    assert_unsupported(
        "<script module>\n\tlet count = $state(0);\n</script>\n<p>hi</p>",
        "rune $state",
    );
}

#[test]
fn compile_module_refuses_store_read() {
    // A module-scope `$name` store read is the oracle's `store_invalid_subscription`
    // error — the guard refuses it (no store exemption in a module).
    assert_unsupported(
        "<script module>\n\timport { writable } from 'svelte/store';\n\tconst c = writable(0);\n\tconst v = $c;\n</script>\n<p>hi</p>",
        "$-prefixed identifier $c",
    );
}

#[test]
fn compile_module_refuses_top_level_await() {
    // Top-level `await` forces the oracle's async-component shapes (not implemented),
    // so a module top-level await refuses — a safe over-refusal (the oracle compiles it).
    assert_unsupported(
        "<script module>\n\tconst x = await fetch('/');\n</script>\n<p>hi</p>",
        "top-level await",
    );
}

#[test]
fn compile_module_body_follows_hoisted_snippet() {
    // Emission order (probe-verified): the module block prints AFTER the hoisted
    // snippets, NOT merged into the instance import group — imports, hoisted
    // snippet, module body, then the component function.
    let js = compile_js(
        "<script module>\n\tconst SHARED = 5;\n</script>\n{#snippet foo()}<p>{SHARED}</p>{/snippet}\n{@render foo()}",
    );
    assert_eq!(
        js,
        "import * as $ from 'svelte/internal/server';\n\
             function foo($$renderer) {\n\
             \t$$renderer.push(`<p>5</p>`);\n\
             }\n\
             const SHARED = 5;\n\
             export default function Input($$renderer) {\n\
             \tfoo($$renderer);\n\
             }\n"
    );
}

#[test]
fn compile_module_sets_document_ts_flag() {
    // A `lang="ts"` module sets the document-wide TypeScript flag, so the instance
    // script's TypeScript erases even though it carries no `lang` of its own.
    let js = compile_js(
        "<script module lang=\"ts\">\n\tconst K: number = 5;\n</script>\n<script>\n\tlet a: number = 1;\n</script>\n<p>{a}{K}</p>",
    );
    assert!(
        !js.contains(": number"),
        "instance TypeScript must erase under the module's lang=\"ts\": {js}"
    );
}

#[test]
fn compile_module_refuses_name_collision_with_instance() {
    // A name declared in BOTH scripts: the oracle resolves `{K}` to the instance
    // (inner-scope) binding (`$.escape(K)`), but the name-based table would fold
    // the module `const K = 5` — a real MISMATCH, so refuse.
    assert_unsupported(
        "<script module>\n\tconst K = 5;\n</script>\n<script>\n\tlet { K } = $props();\n</script>\n<p>{K}</p>",
        "declared in both the module and instance scripts",
    );
}

#[test]
fn compile_module_before_instance_comment_carries() {
    // A whitespace-only text run between the module `</script>` and the instance
    // `<script>` must NOT trip the template-before-script comment guard — the
    // instance comment carries through (parity with the oracle).
    let js = compile_js(
        "<script module>\n\tconst K = 5;\n</script>\n\n<script>\n\t// instance comment\n\tlet a = 1;\n</script>\n<p>{a}{K}</p>",
    );
    assert!(
        js.contains("// instance comment"),
        "the instance comment must carry through past the module script: {js}"
    );
}

#[test]
fn compile_refuses_module_comment_after_instance_script() {
    // A module script placed AFTER the instance script puts its comments at
    // offsets the oracle's printer re-seeks BACKWARD over (the component body
    // block carries the instance script's `loc`), so esrap re-attaches them into
    // whatever loc-bearing node it reaches next — a template expression it has
    // nothing to do with. tsv drops the comment, which is a comment PRESENCE
    // difference the parity bar grades as a MISMATCH. Refuse.
    for source in [
        // The minimal shape: instance script, module script, template expression.
        "<script>function w(x){return x;}</script><script module>\n// MYC\nconst K = 5;\n</script>{w(1)}",
        // A block comment lands the same way.
        "<script>function w(x){return x;}</script><script module>\n/* MYC */\nconst K = 5;\n</script>{w(1)}",
        // The comment past the module body's last statement lands the same way.
        "<script>function w(x){return x;}</script><script module>\nconst K = 5;\n// MYC\n</script>{w(1)}",
        // An import-only instance script still supplies the `loc` that seeks back.
        "<script>import {a} from './a.js';</script><script module>\n// MYC\nconst K = 5;\n</script>{a}",
    ] {
        assert_unsupported(source, "module script placed after the instance script");
    }
}

#[test]
fn compile_module_comment_before_instance_script_still_drops() {
    // The mirror of the refusal above: with the module script FIRST, the body
    // block's seek moves FORWARD past the module comment, so the oracle drops it
    // too — tsv's drop is parity and must keep compiling.
    let js = compile_js(
        "<script module>\n// MYC\nconst K = 5;\n</script><script>function w(x){return x;}</script>{w(1)}",
    );
    assert!(
        !js.contains("MYC"),
        "a module comment before the instance script must drop: {js}"
    );
}

// ── <svelte:boundary> ──────────────────────────────────────────────────────

#[test]
fn compile_boundary_invalid_attributes_refuse() {
    // The oracle's phase-2 `SvelteBoundary` visitor accepts a CLOSED list of three
    // names, each carrying exactly one `{expression}`. tsv's parser accepts all of
    // the shapes below, so without the guard each would be an over-acceptance of
    // input the oracle rejects — the refusal contract's hard bar.
    for source in [
        // `svelte_boundary_invalid_attribute`: an unknown plain attribute, a
        // spread, and every directive kind (a modern event attribute included).
        "<svelte:boundary foo={1}><p>a</p></svelte:boundary>",
        "<svelte:boundary {...x}><p>a</p></svelte:boundary>",
        "<svelte:boundary onclick={f}><p>a</p></svelte:boundary>",
        "<svelte:boundary on:click={f}><p>a</p></svelte:boundary>",
        "<svelte:boundary class:x={y}><p>a</p></svelte:boundary>",
        "<svelte:boundary bind:this={el}><p>a</p></svelte:boundary>",
        "<svelte:boundary use:action><p>a</p></svelte:boundary>",
        // `svelte_boundary_invalid_attribute_value`: a valid NAME whose value is
        // not exactly one expression tag.
        "<svelte:boundary onerror><p>a</p></svelte:boundary>",
        "<svelte:boundary onerror=\"x\"><p>a</p></svelte:boundary>",
        "<svelte:boundary onerror=\"a{b}c\"><p>a</p></svelte:boundary>",
    ] {
        assert_refuses(source);
    }
}

#[test]
fn compile_boundary_snippet_attribute_forms_refuse() {
    // The `failed={expr}` / `pending={expr}` ATTRIBUTE forms compile in the oracle
    // but are a deliberate v1 gap (asymmetric precedence against a same-named
    // snippet, plus the statically-nullish `pending` if/else fork).
    assert_unsupported(
        "<script>const f = () => {};</script><svelte:boundary failed={f}><p>a</p></svelte:boundary>",
        "attribute form",
    );
    assert_unsupported(
        "<script>const g = () => {};</script><svelte:boundary pending={g}><p>a</p></svelte:boundary>",
        "attribute form",
    );
}

#[test]
fn compile_boundary_onerror_is_dropped_but_guarded() {
    // A valid `onerror={handler}` never reaches SSR output, but the oracle still
    // analyzes it — so it is guard-walked like an event-handler attribute.
    let out = compile(
        "<svelte:boundary onerror={(e) => e}><p>a</p></svelte:boundary>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(!out.js.contains("onerror"), "onerror must drop: {}", out.js);
    assert_refuses("<svelte:boundary onerror={() => $state(1)}><p>a</p></svelte:boundary>");
}

#[test]
fn compile_boundary_anchors_are_isolated_pushes() {
    // Unlike `{#key}`'s `<!---->` marker, a boundary's anchors do NOT merge into an
    // adjacent sibling's template — the oracle's `build_template` starts a fresh
    // push for each. A merge would be invisible to the fixtures if no fixture put a
    // sibling beside a boundary, so pin it here too.
    let out = compile(
        "<b>a</b><svelte:boundary><p>hi</p></svelte:boundary><i>z</i>",
        &CompileOptions::default(),
    )
    .unwrap();
    assert!(
        out.js.contains("$$renderer.push(`<b>a</b>`)")
            && out.js.contains("$$renderer.push(`<!--[-->`)"),
        "anchors must not merge with siblings: {}",
        out.js
    );
}

#[test]
fn compile_refuses_rune_name_bound_as_store() {
    // The oracle's `analyze_component` reclassifies `$state` as a STORE
    // subscription on the imported `state` binding — `$.store_get(($$store_subs
    // ??= {}), '$state', state)()` — because the binding's initializer is not a
    // rune call. tsv would compile it as the rune (`const x = void 0`), so refuse.
    assert_unsupported(
        "<script>\n\timport { state } from './store';\n\tconst x = $state();\n</script>\n<p>{x}</p>",
        "rune $state whose base is also an instance binding",
    );
    // No store import is needed — ANY instance binding of the stem collides.
    assert_unsupported(
        "<script>\n\tlet state = 5;\n\tconst x = $state(1);\n</script>\n<p>{x}</p>",
        "rune $state whose base is also an instance binding",
    );
    // Every rune keyword, not just `$state`.
    assert_unsupported(
        "<script>\n\timport { derived } from './store';\n\tlet d = $derived(1);\n</script>\n<p>{d}</p>",
        "rune $derived whose base is also an instance binding",
    );
    assert_unsupported(
        "<script>\n\timport { props } from './store';\n\tlet { a } = $props();\n</script>\n<p>{a}</p>",
        "rune $props whose base is also an instance binding",
    );
    // A function or class declaration binds the stem just as an import does.
    assert_unsupported(
        "<script>\n\tfunction state() {}\n\tconst x = $state(1);\n</script>\n<p>{x}{state}</p>",
        "rune $state whose base is also an instance binding",
    );
}

#[test]
fn compile_refuses_rune_name_bound_in_module_scope() {
    // `instance.scope.get(stem)` walks UP (`phases/scope.js:748`) and the instance
    // scope's parent IS the module scope (`2-analyze/index.js:337`), so a MODULE
    // binding reclassifies an INSTANCE `$state` too. Oracle (verified):
    // `const x = $.store_get($$store_subs ??= {}, '$state', state)(1)`.
    assert_unsupported(
        "<script module>\n\timport { state } from './store.js';\n</script>\n<script>\n\tconst x = $state(1);\n</script>\n<p>{x}</p>",
        "rune $state whose base is also an instance binding",
    );
    // The module's own exempting initializer counts too — a module binding whose
    // init is a rune call is not reclassified. (A module `$state` refuses on its
    // own path, so the reachable exempt shape is a module-scope declaration
    // consumed only by the instance script.)
    assert_unsupported(
        "<script module>\n\tlet state = 5;\n</script>\n<script>\n\tconst x = $state(1);\n</script>\n<p>{x}</p>",
        "rune $state whose base is also an instance binding",
    );
}

#[test]
fn compile_refuses_rune_name_bound_by_hoisted_var() {
    // `var` is FUNCTION-scoped, so a `var state` in any block or for-head of the
    // instance script lands in `instance.scope` exactly like a top-level one.
    // Oracle (verified): both reclassify to `$.store_get(…)`.
    assert_unsupported(
        "<script>\n\tif (true) {\n\t\tvar state = 5;\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}{state}</p>",
        "rune $state whose base is also an instance binding",
    );
    assert_unsupported(
        "<script>\n\tfor (var props of []) {\n\t}\n\tlet { a } = $props();\n</script>\n<p>{a}{props}</p>",
        "rune $props whose base is also an instance binding",
    );
}

#[test]
fn compile_refuses_rune_name_bound_in_class_static_block() {
    // ⚠️ A class STATIC BLOCK is NOT a scope in the oracle: `phases/scope.js` has
    // no `StaticBlock` visitor (and none for `ClassBody`/`MethodDefinition`), so a
    // `var` there declares directly in the ENCLOSING script scope. ECMAScript
    // disagrees — a static block is its own VariableEnvironment — but the oracle
    // is the parity target. Oracle (verified, all three): reclassified to
    // `$.store_get(($$store_subs ??= {}), '$state', state)(1)` / `'$props'`.
    assert_unsupported(
        "<script>\n\tclass C {\n\t\tstatic {\n\t\t\tvar state = 5;\n\t\t\tconsole.log(state);\n\t\t}\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}</p>",
        "rune $state whose base is also an instance binding",
    );
    // The same class as an EXPRESSION in a declarator initializer.
    assert_unsupported(
        "<script>\n\tconst C = class {\n\t\tstatic {\n\t\t\tvar state = 5;\n\t\t\tconsole.log(state);\n\t\t}\n\t};\n\tconst x = $state(1);\n</script>\n<p>{x}{C}</p>",
        "rune $state whose base is also an instance binding",
    );
    // A class expression nested deeper than a declarator init — here an
    // assignment RHS. Oracle (verified): reclassifies. The fence reaches it
    // because it never asks WHERE the class sits.
    assert_unsupported(
        "<script>\n\tlet y;\n\ty = class {\n\t\tstatic {\n\t\t\tvar state = 5;\n\t\t\tconsole.log(state);\n\t\t}\n\t};\n\tconst x = $state(1);\n</script>\n<p>{x}{y}</p>",
        "rune $state whose base is also an instance binding",
    );
    // The `$props` stem, to show the rule is not `$state`-specific.
    assert_unsupported(
        "<script>\n\tclass C {\n\t\tstatic {\n\t\t\tvar props = 5;\n\t\t\tconsole.log(props);\n\t\t}\n\t}\n\tlet { a } = $props();\n</script>\n<p>{a}</p>",
        "rune $props whose base is also an instance binding",
    );
}

#[test]
fn compile_refuses_rune_name_bound_by_porous_hoisted_var_with_rune_init() {
    // ⚠️ A `var` hoisting through a POROUS scope arrives with NO initializer:
    // `phases/scope.js:673-681` re-declares it on the parent via the 3-argument
    // call, leaving `initial` at its `null` default. So `get_rune(binding.initial)`
    // is null and the rune EXEMPTION does not apply, even though the declarator
    // was written with a rune init. Oracle (verified): emits
    // `var state = $.store_get(($$store_subs ??= {}), '$state', state)(0)` — it
    // reclassifies. tsv also refuses this via its rune guard, but that is an
    // ACCIDENTAL save on an unrelated path; this pins the modelled one.
    assert_unsupported(
        "<script>\n\t{\n\t\tvar state = $state(0);\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}{state}</p>",
        "rune $state whose base is also an instance binding",
    );
    assert_unsupported(
        "<script>\n\t{\n\t\tvar props = $props.id();\n\t}\n\tlet { a } = $props();\n</script>\n<p>{a}{props}</p>",
        "rune $props whose base is also an instance binding",
    );
}

#[test]
fn compile_refuses_every_class_expression_position_holding_a_static_block() {
    // The fence's whole point: a static block declares at script scope from ANY
    // position a class can occupy, and enumerating those positions is what shipped
    // holes twice. Each shape below was a live MISMATCH under the enumerated walk
    // (tsv compiled it as the rune; the oracle emitted `$.store_get`), and none
    // needs its own arm now — the scan never asks where the class sits.
    //
    // A representative per structural family, since the fix is categorical: a
    // for-head (the `init`/`test`/`update`/`right` positions all behaved alike), a
    // class-DECLARATION member position (a property initializer is NOT a function
    // scope — `phases/scope.js` has no `PropertyDefinition` visitor — and neither
    // is a computed key or a `super_class`), and a function PARAMETER DEFAULT (the
    // oracle gives a function's parameters a POROUS scope, `scope.js:1143/1155/1163`).
    for source in [
        // for-head init expression
        "<script>\n\tvar y;\n\tfor (y = class { static { var state = 5 } }; false; ) {}\n\tlet count = $state(0);\n</script>\n<p>{count}</p>",
        // for-of right
        "<script>\n\tvar z;\n\tfor (z of [class { static { var state = 5 } }]) {}\n\tlet count = $state(0);\n</script>\n<p>{count}</p>",
        // class-declaration super_class
        "<script>\n\tclass C extends (class { static { var state = 5 } }) {}\n\tlet count = $state(0);\n</script>\n<p>{count}</p>",
        // class-declaration property initializer
        "<script>\n\tclass C { p = class { static { var state = 5 } }; }\n\tlet count = $state(0);\n</script>\n<p>{count}</p>",
        // class-declaration computed member key
        "<script>\n\tclass C { [(class { static { var state = 5 } }, 'k')]() {} }\n\tlet count = $state(0);\n</script>\n<p>{count}</p>",
        // function parameter default
        "<script>\n\tfunction f(a = class { static { var state = 5 } }) {}\n\tlet count = $state(0);\n</script>\n<p>{count}</p>",
    ] {
        assert_unsupported(source, "rune $state whose base is also an instance binding");
    }
}

#[test]
fn compile_refuses_a_static_block_that_declares_nothing() {
    // The deliberate over-refusal the fence buys, pinned so it is a choice rather
    // than a surprise: this static block binds no rune stem at all and the oracle
    // compiles it fine, but the fence cannot tell without traversing the positions
    // it exists to avoid traversing. Measured cost: zero — no `.svelte` file in the
    // ~4900-file compile corpus contains a static block.
    assert_unsupported(
        "<script>\n\tclass C {\n\t\tstatic {\n\t\t\tconsole.log('hi');\n\t\t}\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}{C}</p>",
        "rune $state whose base is also an instance binding",
    );
    // ⚠️ It also swallows the oracle's rune EXEMPTION inside a static block — a
    // `var state = $state(0)` there keeps its initializer (no scope, so no
    // re-declare) and the oracle does NOT reclassify. tsv refused this already, on
    // its rune-guard path (a `$state` call inside a class body); the fence now
    // refuses it first. Either way it is a refusal, never a wrong compile, and the
    // exemption arm for this shape was unreachable even before the fence.
    assert_unsupported(
        "<script>\n\tclass C {\n\t\tstatic {\n\t\t\tvar state = $state(0);\n\t\t\tconsole.log(state);\n\t\t}\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}{C}</p>",
        "rune $state whose base is also an instance binding",
    );
}

#[test]
fn compile_allows_ordinary_class_using_code_with_no_static_block() {
    // The other side of the fence, and the one that protects parity: ordinary
    // class-using code — including `static` MEMBERS, which are not static blocks —
    // must not trip it. Only `static` followed by `{` does.
    assert_compiles(
        "<script>\n\tclass C {\n\t\tstatic label = 'c';\n\t\tstatic make() {\n\t\t\treturn new C();\n\t\t}\n\t}\n\tlet count = $state(0);\n</script>\n<p>{count}{C.label}</p>",
    );
    // A class EXPRESSION in the shapes the enumerated walk used to descend into,
    // with no static block anywhere: still compiles.
    assert_compiles(
        "<script>\n\tconst K = class {\n\t\tvalue = 1;\n\t};\n\tlet count = $state(0);\n</script>\n<p>{count}{K}</p>",
    );
}

#[test]
fn compile_refuses_static_block_separated_by_zwnbsp() {
    // The static-block fence's whitespace class must be ECMAScript's, not Rust's.
    // U+FEFF (<ZWNBSP>) is ECMAScript `WhiteSpace` but carries no Unicode
    // `White_Space` property, so `char::is_whitespace` says NO and a
    // `static\u{feff}{ … }` block was INVISIBLE to the scan — the fence never
    // fired, `script_declarations_of` stopped at the class body, and tsv compiled
    // the rune where the oracle emits a store read. Oracle (verified live, both
    // script kinds): `$.store_get(($$store_subs ??= {}), '$state', state)(0)`.
    //
    // The reverse code point needs no handling: U+0085 (<NEL>) is Unicode
    // whitespace but NOT ECMAScript whitespace, so treating it as a boundary only
    // ever OVER-reports — an extra refusal, on source the JS lexer rejects anyway.
    assert_unsupported(
        "<script>\n\tclass C { static\u{feff}{ var state = 5 } }\n\tlet x = $state(0);\n</script>\n<p>{x}</p>",
        "rune $state whose base is also an instance binding",
    );
    // The module script is scanned by the same fence — the collision pre-pass
    // tests `instance.scope.get`, which walks up into module scope.
    assert_unsupported(
        "<script module>\n\tclass C { static\u{feff}{ var state = 5 } }\n</script>\n<script>\n\tlet x = $state(0);\n</script>\n<p>{x}</p>",
        "rune $state whose base is also an instance binding",
    );
}

#[test]
fn compile_refuses_global_pseudo_class_with_a_non_ascii_ident_char() {
    // A CSS name must NOT be trimmed with a Unicode-whitespace notion: every code
    // point at or above U+0080 is a CSS *ident* code point, so `:global\u{a0}` is
    // the pseudo-class `global\u{a0}`, not `:global`. Rust's `str::trim` stripped
    // the NBSP, tsv read `:global`, and scoped the element while the oracle pruned
    // the rule as unused — an oracle-verified MISMATCH, now a safe over-refusal
    // (the selector matches no element).
    assert_unsupported(
        "<div class=\"x\">a</div>\n<style>\n\tdiv :global\u{a0}.x { color: red }\n</style>",
        "matches no element",
    );
}

#[test]
fn compile_refuses_rune_name_bound_by_escaped_identifier() {
    // An ESCAPED binding identifier binds the decoded name. `plain_identifier_name`
    // reports `None` for one, so the walk resolves through the interner instead —
    // the reference here is plain, so refusing the escaped *reference* elsewhere
    // does not cover this. Oracle (verified): `$.store_get(…, '$state', state)(1)`.
    assert_unsupported(
        "<script>\n\timport { state as \\u0073tate } from './store.js';\n\tconst x = $state(1);\n</script>\n<p>{x}{state}</p>",
        "rune $state whose base is also an instance binding",
    );
}

#[test]
fn compile_refuses_rune_reference_separated_by_unicode_whitespace() {
    // NBSP (U+00A0) is ECMAScript whitespace, so `$state\u{a0}(1)` is a genuine
    // `$state` reference. A byte-level boundary test that counts every byte
    // `>= 0x80` as identifier text reads NBSP's `0xC2` lead byte as a continuation
    // and MISSES it — an under-refusal. Oracle (verified): reclassifies.
    assert_unsupported(
        "<script>\n\timport { state } from './store.js';\n\tconst x = $state\u{a0}(1);\n</script>\n<p>{x}{state}</p>",
        "rune $state whose base is also an instance binding",
    );
}

#[test]
fn compile_allows_rune_stem_bound_only_in_a_child_scope() {
    // ⚠️ The lookup walks UP, never DOWN. A function parameter, a block-scoped
    // `let`, and a `var` inside a nested FUNCTION all live in CHILD scopes
    // `instance.scope.get` never reaches, so none collide — refusing any of these
    // would be a spurious over-refusal on ordinary code. All three verified at
    // parity against the oracle.
    assert_compiles(
        "<script>\n\tfunction f(state) {\n\t\treturn state;\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}{f(2)}</p>",
    );
    assert_compiles(
        "<script>\n\tif (true) {\n\t\tlet state = 5;\n\t\tconsole.log(state);\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}</p>",
    );
    assert_compiles(
        "<script>\n\tfunction f() {\n\t\tvar state = 5;\n\t\treturn state;\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}{f()}</p>",
    );
}

#[test]
fn compile_allows_rune_binding_initialized_by_its_own_rune() {
    // ⚠️ THE EXEMPTION. `get_rune(init) !== null` is the oracle's carve-out and
    // covers the overwhelmingly common real-world shapes — refusing them would
    // crater corpus parity, so they are pinned here.
    assert_compiles("<script>\n\tlet state = $state(0);\n</script>\n<p>{state}</p>");
    assert_compiles("<script>\n\tlet derived = $derived(1);\n</script>\n<p>{derived}</p>");
    assert_compiles("<script>\n\tlet props = $props();\n</script>\n<p>{props.a}</p>");
    assert_compiles("<script>\n\tlet state = $state.raw(0);\n</script>\n<p>{state}</p>");
    // A binding of a rune stem with NO `$stem` reference anywhere is untouched:
    // the oracle's loop only ever sees names that are actually referenced.
    assert_compiles("<script>\n\timport { state } from './store';\n</script>\n<p>{state}</p>");
    // `$derived` beside `import { derived } from 'svelte/store'` is the oracle's
    // explicit exception ("one is not a subscription to the other").
    assert_compiles(
        "<script>\n\timport { derived } from 'svelte/store';\n\tlet d = $derived(1);\n</script>\n<p>{d}{derived}</p>",
    );
}

#[test]
fn compile_non_ascii_identifier_beside_erased_typescript() {
    // ⚠️ REGRESSION GUARD (a PANIC, not a mismatch). The erased-region comment
    // window scans bytes with `text_class::js_char_at`; a scan that advanced by
    // ONE BYTE over a non-whitespace character landed on a continuation byte of
    // a multi-byte identifier and panicked (`byte index N is not a char boundary;
    // it is inside 'é'`) on ordinary, legal TypeScript. Both halves of the window
    // are exercised: a `<T>` parameter list (the BACKWARD half — `prev_token_end`
    // scans over the name to find the token before the detached region) and a
    // `: T` annotation (the FORWARD half).
    let js = compile_js(
        "<script lang=\"ts\">\n\tfunction caf\u{e9}<T>(x: T): T {\n\t\treturn x;\n\t}\n\tlet v = caf\u{e9}(1);\n</script>\n<p>{v}</p>",
    );
    assert!(js.contains("caf\u{e9}"), "identifier must survive:\n{js}");
    assert!(!js.contains(": T"), "type annotation must erase:\n{js}");

    // The annotation shape: a multi-byte PARAMETER name immediately preceding the
    // erased `: number`.
    let js = compile_js(
        "<script lang=\"ts\">\n\tfunction f(caf\u{e9}: number): number {\n\t\treturn caf\u{e9};\n\t}\n\tlet v = f(1);\n</script>\n<p>{v}</p>",
    );
    assert!(js.contains("caf\u{e9}"), "identifier must survive:\n{js}");
    assert!(!js.contains(": number"), "annotation must erase:\n{js}");

    // A multi-byte name before an erased `implements` clause and a return type —
    // the two other detached regions the backward half exists for.
    let js = compile_js(
        "<script lang=\"ts\">\n\tinterface I {}\n\tclass \u{4e2d}\u{6587} implements I {\n\t\tm(): void {}\n\t}\n\tlet v = new \u{4e2d}\u{6587}();\n</script>\n<p>{v}</p>",
    );
    assert!(
        js.contains("\u{4e2d}\u{6587}"),
        "identifier must survive:\n{js}"
    );
    assert!(!js.contains("implements"), "implements must erase:\n{js}");
}

#[test]
fn compile_attr_whitespace_collapse_trims_the_js_class_not_the_narrow_one() {
    // The oracle is `chunk.data.replace(/[ \t\n\r\f]+/g, ' ').trim()`
    // (`server/visitors/shared/utils.js:208` + `phases/patterns.js:11`): a NARROW
    // ASCII collapse, then JavaScript's WIDE `String.prototype.trim`. Fusing them
    // into one narrow-class pass left every boundary character that is JS
    // whitespace but not `[ \t\n\r\f]` in place — 7 oracle-verified MISMATCHes.
    // All four affected sites, with two distinct exotic characters each.

    // Site 1 — an unscoped plain `class` attribute.
    let js = compile_js("<div class=\"\u{a0}x\u{a0}\">t</div>");
    assert!(js.contains("class=\"x\""), "U+00A0 must trim:\n{js}");
    let js = compile_js("<div class=\"\u{feff}x\u{feff}\">t</div>");
    assert!(js.contains("class=\"x\""), "U+FEFF must trim:\n{js}");

    // Site 2 — a plain `style` attribute.
    let js = compile_js("<div style=\"\u{a0}color: red;\u{a0}\">t</div>");
    assert!(
        js.contains("style=\"color: red;\""),
        "U+00A0 must trim:\n{js}"
    );
    let js = compile_js("<div style=\"\u{3000}color: red;\u{3000}\">t</div>");
    assert!(
        js.contains("style=\"color: red;\""),
        "U+3000 must trim:\n{js}"
    );

    // Site 3 — a SCOPED `class` attribute (the hash concat runs on the trimmed
    // value, so a stray boundary character would sit between value and hash).
    let js = compile_js(
        "<div class=\"\u{a0}x\u{a0}\">t</div>\n<style>\n\t.x {\n\t\tcolor: red;\n\t}\n</style>",
    );
    assert!(
        js.contains("class=\"x svelte-tsvhash\""),
        "scoped class must trim before the hash concat:\n{js}"
    );

    // Site 4 — the `class:` directive BASE (`$.attr_class(base, …)`).
    let js = compile_js("<div class:on={true} class=\"\u{a0}x\u{a0}\">t</div>");
    assert!(
        js.contains("$.attr_class('x'"),
        "directive base must trim:\n{js}"
    );
    // U+000B is ASCII yet outside the narrow collapse class — the same bug.
    let js = compile_js("<div class:on={true} class=\"\u{b}x\u{b}\">t</div>");
    assert!(js.contains("$.attr_class('x'"), "U+000B must trim:\n{js}");

    // The spread-object path and a `style:` directive value share the function.
    let js = compile_js(
        "<script>\n\tlet o = {};\n</script>\n\n<div {...o} class=\"\u{a0}x\u{a0}\">t</div>",
    );
    assert!(js.contains("class: 'x'"), "spread object must trim:\n{js}");
    let js = compile_js("<div style:color=\"\u{a0}red\u{a0}\">t</div>");
    assert!(
        js.contains("color: 'red'"),
        "style directive value must trim:\n{js}"
    );

    // ⚠️ THE DISCRIMINATOR. `U+0085` (<NEL>) and `U+180E` carry Unicode's
    // `White_Space` property but are NOT ECMAScript whitespace, so JS `.trim()`
    // KEEPS them — and so must tsv. This is what a `str::trim` would get wrong in
    // the opposite direction; both are oracle-verified at parity.
    let js = compile_js("<div class=\"\u{85}y\u{85}\">t</div>");
    assert!(
        js.contains("class=\"\u{85}y\u{85}\""),
        "U+0085 must NOT trim (not ECMAScript whitespace):\n{js}"
    );
    let js = compile_js("<div class=\"\u{180e}y\u{180e}\">t</div>");
    assert!(
        js.contains("class=\"\u{180e}y\u{180e}\""),
        "U+180E must NOT trim (not ECMAScript whitespace):\n{js}"
    );

    // Interior narrow-class runs still collapse to one space, and an interior
    // `&nbsp;` still survives (why the oracle's collapse is not `\s+`).
    let js = compile_js("<div class=\"a  \t b\">t</div>");
    assert!(
        js.contains("class=\"a b\""),
        "interior run must collapse:\n{js}"
    );
    let js = compile_js("<div class=\"a\u{a0}b\">t</div>");
    assert!(
        js.contains("class=\"a\u{a0}b\""),
        "interior U+00A0 must survive the narrow collapse:\n{js}"
    );
}

// ── Invalid assignment targets (the oracle's `validate_assignment` family) ──

#[test]
fn compile_constant_assignment_refuses() {
    // `validate_no_const_assignment` (`shared/utils.js:84`): a write to a `const`
    // declarator or an import local is `constant_assignment`, keyed on the
    // DECLARATION KEYWORD — so a reactive `const a = $state(0)` is refused too.
    // Both operator forms reach it (`AssignmentExpression.js:11`,
    // `UpdateExpression.js:11`).
    assert_unsupported("<script>const a = c(); a += 1;</script>", "a constant");
    assert_unsupported("<script>const a = c(); a++;</script>", "a constant");
    assert_unsupported(
        "<script>const a = $state(0); a += 1;</script>",
        "a constant",
    );
    assert_unsupported("<script>const a = $state(0); a++;</script>", "a constant");
    assert_unsupported(
        "<script>import {a} from './m.js'; a = 1;</script>",
        "a constant",
    );
    // A module-script const is unassignable on the same terms.
    assert_unsupported(
        "<script module>const a = 1;</script><script>a = 2;</script>",
        "a constant",
    );
    // Inside a dropped event handler too — the oracle validates in phase 2,
    // before it decides what SSR emits.
    assert_unsupported(
        "<script>const a = $state(0);</script><button onclick={() => a++}>x</button>",
        "a constant",
    );
    // Destructuring assignment: the rule recurses through ArrayPattern elements
    // and ObjectPattern property VALUES (`validator/samples/assignment-to-const-5`
    // and `-7`).
    assert_unsupported(
        "<script>const arr = [1, 2]; [arr, arr[1]] = [arr[1], arr[0]];</script>",
        "a constant",
    );
    assert_unsupported(
        "<script>const arr = [1]; ({a: {arr}} = x);</script>",
        "a constant",
    );
}

#[test]
fn compile_each_item_assignment_refuses() {
    // `validate_assignment` (`shared/utils.js:33`): a write to an `{#each}`
    // context binding is `each_item_invalid_assignment` in runes mode, which this
    // compiler unconditionally is. Reached from an assignment, an update, and a
    // `bind:` alike (`BindDirective.js:181`).
    assert_unsupported(
        "<script>let arr = $state([1]);</script>\
         {#each arr as value}<button onclick={() => value += 1}>x</button>{/each}",
        "an {#each} item",
    );
    assert_unsupported(
        "<script>let arr = $state([1]);</script>\
         {#each arr as value}<button onclick={() => value++}>x</button>{/each}",
        "an {#each} item",
    );
    assert_unsupported(
        "<script>let arr = $state([{element: null}]);</script>\
         {#each arr as {element}}<input bind:this={element} />{/each}",
        "an {#each} item",
    );
    // The fallback shares the block's scope (`scope.js:1280`).
    assert_unsupported(
        "<script>let arr = $state([1]);</script>\
         {#each arr as value}<p>a</p>{:else}<button onclick={() => value = 1}>x</button>{/each}",
        "an {#each} item",
    );
}

#[test]
fn compile_snippet_parameter_assignment_refuses() {
    // `validate_assignment` (`shared/utils.js:37`): a write to a `{#snippet}`
    // parameter is `snippet_parameter_assignment` — and, unlike the each rule,
    // NOT gated on runes mode.
    assert_unsupported(
        "{#snippet foo(value)}<button onclick={() => value += 1}>x</button>{/snippet}",
        "a {#snippet} parameter",
    );
}

#[test]
fn compile_invalid_assignment_boundary_accepts() {
    // The acceptance half of the rule, so a widening over-refusal is caught.
    //
    // A MEMBER target writes THROUGH the binding and never rebinds it — it
    // matches no branch of the oracle's validator.
    assert_compiles(
        "<script>const o = $state({v: 1});</script><button onclick={() => o.v = 2}>x</button>",
    );
    assert_compiles(
        "<script>let arr = $state([{v: 1}]);</script>\
         {#each arr as item}<button onclick={() => item.v = 2}>x</button>{/each}",
    );
    // A `let` is not a constant, and the block scopes END at their block.
    assert_compiles("<script>let a = $state(0);</script><button onclick={() => a++}>x</button>");
    assert_compiles(
        "<script>let a = $state(0); let arr = $state([1]);</script>\
         {#each arr as a}<p>{a}</p>{/each}<button onclick={() => a++}>x</button>",
    );
    assert_compiles(
        "{#snippet foo(v)}<p>{v}</p>{/snippet}<script>let v = $state(0);</script>\
         <button onclick={() => v++}>x</button>",
    );
    // The pattern recursion stops exactly where the oracle's does: a RestElement
    // and an AssignmentPattern match no branch, so a const there is accepted.
    assert_compiles(
        "<script>const a = 1;</script><button onclick={() => { [...a] = x; }}>x</button>",
    );
}

#[test]
fn compile_each_item_assignment_refuses_the_checklist_repro() {
    // The exact repro `docs/checklist_svelte_compiler.md` carried as an open
    // over-acceptance until this rule landed — pinned so the doc's claim that the
    // row is closed cannot silently rot.
    assert_unsupported(
        "<script>let b = 1;</script>{#each [0] as b}<button onclick={() => { b++; }}>x</button>{/each}",
        "an {#each} item",
    );
}
