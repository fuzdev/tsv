//! The `$`-prefixed identifier rules and the oracle exemptions.

use super::support::*;
use crate::*;

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
