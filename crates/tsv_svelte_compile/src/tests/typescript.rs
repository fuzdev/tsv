//! TypeScript erasure, the `lang="ts"` gate, and the refuse-don't-erase set.

use super::support::*;
use crate::*;

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
