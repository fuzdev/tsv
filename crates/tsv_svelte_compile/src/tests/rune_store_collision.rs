//! The rune-vs-store name collision pre-pass and the static-block fence.

use super::support::*;

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
    let _ = compile_js(
        "<script>\n\tclass C {\n\t\tstatic label = 'c';\n\t\tstatic make() {\n\t\t\treturn new C();\n\t\t}\n\t}\n\tlet count = $state(0);\n</script>\n<p>{count}{C.label}</p>",
    );
    // A class EXPRESSION in the shapes the enumerated walk used to descend into,
    // with no static block anywhere: still compiles.
    let _ = compile_js(
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
    let _ = compile_js(
        "<script>\n\tfunction f(state) {\n\t\treturn state;\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}{f(2)}</p>",
    );
    let _ = compile_js(
        "<script>\n\tif (true) {\n\t\tlet state = 5;\n\t\tconsole.log(state);\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}</p>",
    );
    let _ = compile_js(
        "<script>\n\tfunction f() {\n\t\tvar state = 5;\n\t\treturn state;\n\t}\n\tconst x = $state(1);\n</script>\n<p>{x}{f()}</p>",
    );
}

#[test]
fn compile_allows_rune_binding_initialized_by_its_own_rune() {
    // ⚠️ THE EXEMPTION. `get_rune(init) !== null` is the oracle's carve-out and
    // covers the overwhelmingly common real-world shapes — refusing them would
    // crater corpus parity, so they are pinned here.
    let _ = compile_js("<script>\n\tlet state = $state(0);\n</script>\n<p>{state}</p>");
    let _ = compile_js("<script>\n\tlet derived = $derived(1);\n</script>\n<p>{derived}</p>");
    let _ = compile_js("<script>\n\tlet props = $props();\n</script>\n<p>{props.a}</p>");
    let _ = compile_js("<script>\n\tlet state = $state.raw(0);\n</script>\n<p>{state}</p>");
    // A binding of a rune stem with NO `$stem` reference anywhere is untouched:
    // the oracle's loop only ever sees names that are actually referenced.
    let _ = compile_js("<script>\n\timport { state } from './store';\n</script>\n<p>{state}</p>");
    // `$derived` beside `import { derived } from 'svelte/store'` is the oracle's
    // explicit exception ("one is not a subscription to the other").
    let _ = compile_js(
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
