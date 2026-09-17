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
    // The oracle strips a block comment's start-line indentation from every line and
    // re-indents the rest to the emit position; tsv carries a preserved (non-gutter)
    // comment verbatim, so they diverge. Refuse.
    let what = "multi-line block comment in script";
    for src in [
        "<script>\n\t/*\n\tmulti\n\tline\n\t*/\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        // A blank line makes a gutter comment preserved, not indentable.
        "<script>\n\t/**\n\t * doc\n\n\t * two\n\t */\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        // Svelte's strip also eats the start of the FIRST line when it opens with the
        // indentation, which the canonical reprint keeps.
        "<script>\n\t/*\t*\n\t * x\n\t */\n\tlet x = 1;\n</script>\n<p>{x}</p>",
        "<script>\n  /*  * a\n   * x\n   */\n  let x = 1;\n</script>\n<p>{x}</p>",
        // The strip starts a line at U+2028; esrap and the printer split on `\n` alone.
        "<script>\n\t/**\n\t * a\u{2028}\t * b\n\t */\n\tlet x = 1;\n</script>\n<p>{x}</p>",
    ] {
        assert_unsupported(src, what);
    }
}

#[test]
fn compile_carries_gutter_block_comments() {
    // A `*`-gutter comment is rebuilt from its trimmed lines by the canonical reprint on
    // both sides, so neither side's indentation survives to differ — whatever the
    // authored indent (spaces, misaligned gutter, nesting depth).
    for src in [
        "<script>\n  /**\n   * doc\n   */\n  let x = 1;\n</script>\n<p>{x}</p>",
        "<script>\n  /**\n       * doc\n  */\n  let x = 1;\n</script>\n<p>{x}</p>",
        "<script>\n\tfunction f() {\n\t\t/**\n\t\t * deep\n\t\t */\n\t\treturn 1;\n\t}\n</script>\n<p>{f()}</p>",
        "<script>\n\t/** head\n\t * x\n\t */\n\tlet x = 1;\n</script>\n<p>{x}</p>",
    ] {
        let js = compile_js(src);
        assert_eq!(js.matches("/**").count(), 1, "{src}\n{js}");
    }
}

#[test]
fn compile_carries_comments_with_store() {
    // `var $$store_subs;` and every script store mint are fictional-span nodes, so a
    // comment beside a read, inside an assignment's `=` gap, after an update, or in a
    // later function body prints exactly once — each of these swept into, or out of,
    // an appendix-span mint's window before.
    let base = "<script>\n\timport { writable } from 'svelte/store';\n\tconst s = writable(1);\n";
    for (body, comments) in [
        (
            "\tlet d = /* a */ $s /* b */ + 1; // c\n\tlet e = 2;\n",
            &["/* a */", "/* b */", "// c"][..],
        ),
        (
            "\tfunction set() {\n\t\t$s = /* v */ 5; // set\n\t\t// next\n\t\treturn 1;\n\t}\n",
            &["/* v */", "// set", "// next"][..],
        ),
        (
            "\tfunction inc() {\n\t\t$s /* l */ += /* v */ 2; // t\n\t}\n",
            &["/* l */", "/* v */", "// t"][..],
        ),
        (
            "\tfunction inc() {\n\t\t/* a */ $s++ /* b */;\n\t\t--$s; // c\n\t}\n",
            &["/* a */", "/* b */", "// c"][..],
        ),
        (
            "\tlet v = $s\n\t\t// c\n\t\t.toString(); // d\n",
            &["// c", "// d"][..],
        ),
    ] {
        let src = format!("{base}{body}</script>\n<p>{{$s}}</p>");
        let js = compile_js(&src);
        for comment in comments {
            assert_eq!(js.matches(comment).count(), 1, "{comment} in\n{src}\n{js}");
        }
    }
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

#[test]
fn compile_carries_comments_in_rewritten_rune_calls() {
    // The rewrite drops the rune call's syntax, not its comments: the oracle writes
    // each one beside the kept argument (probed per position), and so does the
    // positional carry — exactly once each. The positions prettier-formatted fixtures
    // can't hold (a callee gap, a member gap) are pinned here.
    for (src, comment) in [
        (
            "<script>let a = $state /* c */ (1); a = 2;</script><p>{a}</p>",
            "/* c */",
        ),
        (
            "<script>let a = $state./* c */raw(1); a = 2;</script><p>{a}</p>",
            "/* c */",
        ),
        (
            "<script>let a = $state /* c */ .raw(1); a = 2;</script><p>{a}</p>",
            "/* c */",
        ),
        (
            "<script>let n = $state(1); n = 2; let d = $derived./* c */by(() => n);</script><p>{d}</p>",
            "/* c */",
        ),
        (
            "<script>class C { x = $state /* c */ (1); } const c = new C();</script><p>{c.x}</p>",
            "/* c */",
        ),
        (
            "<script>let n = $state(1);\n$effect(() => { /* c */ n; });\nlet m = 2;</script><p>{n}{m}</p>",
            "/* c */",
        ),
        // The body-anchored `$derived` thunk leaves the argument's comments to the
        // `$.derived(…)` call windows and the borrowed body, once each — including a
        // body opening with `(`, whose parens an init-anchored parameter scan claimed.
        (
            "<script>let n = $state(1); n = 2;\nlet d = $derived(/* c */ n * /* d */ 2);</script><p>{d}</p>",
            "/* c */",
        ),
        (
            "<script>let n = $state(1); n = 2;\nlet d = $derived((/* c */ n));\nlet e = $derived(n);</script><p>{d}{e}</p>",
            "/* c */",
        ),
        (
            "<script>let n = $state(1);\n$inspect(/* c */ n).with(console.log);\nlet m = 2;</script><p>{n}{m}</p>",
            "/* c */",
        ),
    ] {
        let js = compile_js(src);
        assert_eq!(js.matches(comment).count(), 1, "{src}\n{js}");
    }
}

#[test]
fn compile_refuses_comment_in_rune_call_the_carry_cannot_place() {
    let what = "comment inside a rewritten rune call";
    // An empty `$props()` call becomes `$$props`, which has no window for it.
    assert_unsupported(
        "<script>let { a } = $props(/* c */);</script><p>{a}</p>",
        what,
    );
    // Past a trailing comma the carry drops the comment the oracle keeps.
    for src in [
        "<script>let a = $state(1, /* c */); a = 2;</script><p>{a}</p>",
        "<script>let s = $state({}); let a = $state.snapshot(s, /* c */); a = 2;</script><p>{a}</p>",
        "<script>let n = $state(1); n = 2;\nlet d = $derived.by(() => n, /* c */);</script><p>{d}</p>",
        "<script>let n = $state(1); n = 2;\nlet d = $derived(n * 2, /* c */);</script><p>{d}</p>",
        "<script>class C { x = $state(1, /* c */); } const c = new C();</script><p>{c.x}</p>",
    ] {
        assert_unsupported(src, what);
    }
    // The discriminating pair: the same call without the comma carries.
    let js = compile_js("<script>let a = $state(1 /* c */); a = 2;</script><p>{a}</p>");
    assert_eq!(js.matches("/* c */").count(), 1, "{js}");
}

#[test]
fn compile_refuses_comment_reflushed_into_a_block() {
    // esrap flushes a comment starting on a statement's end line as that statement's
    // trailing comment — even one inside the NEXT statement — and the block holding
    // it then seeks the comment index back and writes it again. tsv writes it once.
    let what = "the oracle prints it twice";
    for src in [
        "<script>let n = 1; if (n) { /* c */ n = 2; }</script><p>{n}</p>",
        "<script>let n = 1; function f() { /* c */ return n; }</script><p>{f()}</p>",
        "<script>let n = 1; let a = f(() => { /* c */ return 1; }); function f(x) { return x; }</script><p>{n}{a}</p>",
        "<script>function g() { let n = 1; if (n) { /* c */ n = 2; } return n; }</script><p>{g()}</p>",
        "<script>class A { x = 1; m() { /* c */ return 1; } } const a = new A();</script><p>{a.x}</p>",
        "<script>let n = $state(1); n = 2; let d = $derived.by(() => { /* c */ return n; });</script><p>{d}</p>",
        "<script>let n = $state(1); n = 2; $effect(() => {}); if (n) { /* c */ n = 3; }</script><p>{n}</p>",
        // Every block kind the census notes: a loop body, a `try` block, an object
        // method, an array element's arrow, a `case` block, a class static block.
        "<script>let n = 1; while (n) { /* c */ n = 0; }</script><p>{n}</p>",
        "<script>let n = 1; try { /* c */ n = 2; } catch { n = 3; }</script><p>{n}</p>",
        "<script>let n = 1; const o = { m() { /* c */ return 1; } };</script><p>{n}{o.m()}</p>",
        "<script>let n = 1; let a = [1, () => { /* c */ return 1; }];</script><p>{n}{a.length}</p>",
        "<script>let n = 1; switch (n) { case 1: { /* c */ n = 2; } }</script><p>{n}</p>",
        "<script>class A { x = 1; static { /* c */ } } const a = new A();</script><p>{a.x}</p>",
    ] {
        assert_unsupported(src, what);
    }
    // The discriminating pairs: the block opening on its own line, or the comment
    // sitting ahead of the block, carries once on both sides.
    for src in [
        "<script>let n = 1;\nif (n) { /* c */ n = 2; }</script><p>{n}</p>",
        "<script>let n = 1; /* c */ if (n) { n = 2; }</script><p>{n}</p>",
        "<script>let a = f(() => { /* c */ return 1; }); function f(x) { return x; }</script><p>{a}</p>",
    ] {
        let js = compile_js(src);
        assert_eq!(js.matches("/* c */").count(), 1, "{src}\n{js}");
    }
}

#[test]
fn compile_refuses_comment_in_a_collapsed_longhand_property() {
    // `{ a: a }` compiles to the shorthand `{ a }` the oracle prints, which leaves no
    // window for a comment in the dropped `a:` gap — so one there refuses.
    assert_unsupported(
        "<script>let a = 1; let o = { a /* c */: a };</script><p>{o.a}</p>",
        "comment inside an erased",
    );
    let js = compile_js("<script>let a = 1; let o = { /* c */ a: a };</script><p>{o.a}</p>");
    assert_eq!(js.matches("/* c */").count(), 1, "{js}");
    // Names compare decoded, as acorn's `name` does: an escaped key or value collapses.
    let js = compile_js(
        "<script>let a = 1; let o = { \\u0061: a }; let p = { a: \\u0061 };</script><p>{o.a}{p.a}</p>",
    );
    assert!(js.contains("let o = { a };"), "{js}");
    assert!(js.contains("let p = { a };"), "{js}");
}
