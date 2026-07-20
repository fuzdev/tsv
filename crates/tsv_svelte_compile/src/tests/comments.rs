//! Script-comment carry, drop, and the divergent-placement refusals.

use super::support::*;
use crate::*;

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
fn compile_carries_script_comments_losslessly() {
    // Leading, trailing-same-line, and between-statement comments carry
    // through: each present exactly once, relative order preserved, and
    // the output is a canonicalize fixed point.
    let out = compile_checked(
        "<script>\n\t// leading\n\tlet { prop } = $props();\n\tlet a = 1; // trailing\n\t// between one\n\t// between two\n\tlet b = 2;\n</script>\n\n<p>{prop}</p>\n",
    );
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
