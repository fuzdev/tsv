//! The `$state` family: folding, class state fields, and their refusals.

use super::support::*;

#[test]
fn compile_state_rune_folds_known_read() {
    // `$state(0)` drops the wrapper; the never-updated binding is
    // statically known, so `{a}` folds into the template (the oracle's
    // evaluator behavior).
    let out = compile_checked("<script>let a = $state(0);</script>\n<p>{a}</p>");
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
    let out = compile_checked(
        "<script>\n\tlet a = $state(0);\n\tfunction inc() {\n\t\ta += 1;\n\t}\n</script>\n<p>{a}</p>",
    );
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
    let out = compile_checked(
        "<script>\n\tclass Counter {\n\t\tcount = $state(0);\n\t}\n\tconst c = new Counter();\n</script>\n<p>{c.count}</p>",
    );
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
fn compile_class_state_lone_escaped_store_arg_refuses() {
    // The escaped spelling of a lone store argument (`$state($count)`) must refuse
    // exactly as the plain `$state($count)` does: the store rewrite now DECODES an
    // escaped `$`-identifier, so the lone-argument check decodes too — otherwise the
    // escaped argument slips the refusal and the store rewrite subscribes it
    // (`$.store_get(…)`) where the oracle keeps the field bare (a MISMATCH).
    let store = "import { writable } from 'svelte/store';\n\tconst count = writable(0);";
    assert_unsupported(
        &format!(
            "<script>\n\t{store}\n\tclass C {{\n\t\tx = $state(\\u0024count);\n\t}}\n\tconst c = new C();\n</script>\n<p>{{c.x}}</p>"
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

// ── Destructured `$state` / `$state.raw` / `$state.snapshot` ────────────────────
// The 1→N `create_state_declarators` lowering. Durable coverage is the
// `state_destructure/*` compile fixtures; these pin the shape + fold + refusals.

#[test]
fn compile_state_destructure_object_projects_from_tmp() {
    // `let { a, b } = $state({…})` → `let tmp = {…}, a = tmp.a, b = tmp.b` (no
    // `$.derived` wrap — the leaves are plain `let`s). An object arg is UNKNOWN, so
    // the reads stay dynamic (`$.escape`), not folded.
    let out = compile_checked(
        "<script>\n\tlet { a, b } = $state({ a: 1, b: 2 });\n</script>\n<p>{a}{b}</p>",
    );
    assert!(
        out.js
            .contains("let tmp = { a: 1, b: 2 },\n\t\ta = tmp.a,\n\t\tb = tmp.b;"),
        "state object destructure not lowered to a temp-destructure: {}",
        out.js
    );
    assert!(
        !out.js.contains("$state"),
        "no $state may survive: {}",
        out.js
    );
    assert!(
        out.js.contains("${$.escape(a)}${$.escape(b)}"),
        "object-arg leaves must stay dynamic: {}",
        out.js
    );
}

#[test]
fn compile_state_destructure_array_uses_bare_array_reads() {
    // `let [ a, b ] = $state([…])` mints a PLAIN-`const` `$$array = $.to_array(tmp,
    // 2)`, and the reads are BARE `$$array[i]` — no `()` call (the state array
    // intermediate is not a derived, unlike `$$derived_array()`).
    let out =
        compile_checked("<script>\n\tlet [ a, b ] = $state([ 1, 2 ]);\n</script>\n<p>{a}{b}</p>");
    assert!(
        out.js.contains("$$array = $.to_array(tmp, 2)"),
        "state array intermediate must be a plain $.to_array: {}",
        out.js
    );
    assert!(
        out.js.contains("a = $$array[0]") && out.js.contains("b = $$array[1]"),
        "state array reads must be bare (no call): {}",
        out.js
    );
    assert!(
        !out.js.contains("$$array()"),
        "a state $$array read must never be a call: {}",
        out.js
    );
}

#[test]
fn compile_state_snapshot_destructure_drops_wrapper() {
    // `let { a } = $state.snapshot(obj)` lowers exactly like `$state` — the snapshot
    // wrapper is dropped (`let tmp = obj, a = tmp.a`). The snapshot binding stays
    // UNKNOWN, so `{a}` reads dynamically even for a known-ish arg.
    let out = compile_checked(
        "<script>\n\tlet obj = {};\n\tlet { a } = $state.snapshot(obj);\n</script>\n<p>{a}</p>",
    );
    assert!(
        out.js.contains("let tmp = obj,\n\t\ta = tmp.a;"),
        "snapshot destructure must drop the wrapper into a temp-destructure: {}",
        out.js
    );
    assert!(
        !out.js.contains("$.snapshot") && !out.js.contains("$state"),
        "no snapshot/$state syntax may survive: {}",
        out.js
    );
    assert!(
        out.js.contains("${$.escape(a)}"),
        "a snapshot leaf never folds: {}",
        out.js
    );
}

#[test]
fn compile_state_destructure_leaf_folds_through_scalar_arg() {
    // The oracle's over-fold: `let n = 5; let { a } = $state(n)` folds `{a}` to the
    // CONTAINER value `5` (using `eval(arg)`, ignoring the `.a` projection) — every
    // leaf takes the rune's computed initial (`Expr(n)` → 5). Reproduced byte-exact.
    let out =
        compile_checked("<script>\n\tlet n = 5;\n\tlet { a } = $state(n);\n</script>\n<p>{a}</p>");
    assert!(
        out.js.contains("<p>5</p>") && !out.js.contains("$.escape"),
        "state scalar-arg leaf must fold to the container value: {}",
        out.js
    );
}

#[test]
fn compile_state_destructure_default_and_rest() {
    // `$.fallback` (default) and `$.exclude_from_object` (object rest) are shared
    // with the derived lowering, byte-identical.
    let dflt = compile_js(
        "<script>\n\tlet obj = {};\n\tlet { a = 1 } = $state(obj);\n</script>\n<p>{a}</p>",
    );
    assert!(
        dflt.contains("a = $.fallback(tmp.a, 1)"),
        "state default must use $.fallback: {dflt}"
    );
    let rest = compile_js(
        "<script>\n\tlet obj = {};\n\tlet { a, ...rest } = $state(obj);\n</script>\n<p>{a}</p>",
    );
    assert!(
        rest.contains("rest = $.exclude_from_object(tmp, ['a'])"),
        "state object rest must use $.exclude_from_object: {rest}"
    );
}

#[test]
fn compile_state_destructure_refuses_exotic_key() {
    // A computed object key inside the pattern is a safe over-refusal (the corpus has
    // identifier keys only). The extractor's `object_member` refuses.
    assert_unsupported(
        "<script>\n\tlet obj = {};\n\tlet key = 'a';\n\tlet { [key]: x } = $state(obj);\n</script>\n{x}",
        "destructuring a $state declarator",
    );
    assert_unsupported(
        "<script>\n\tlet obj = {};\n\tlet key = 'a';\n\tlet { [key]: x } = $state.snapshot(obj);\n</script>\n{x}",
        "destructuring a $state.snapshot declarator",
    );
}

#[test]
fn compile_state_destructure_refuses_multi_declarator() {
    // A destructured state alongside another declarator in one `let` needs
    // per-source-declarator grouping tsv doesn't reproduce — refuse (the oracle
    // compiles it; a safe over-refusal, mirroring the derived arm).
    assert_unsupported(
        "<script>\n\tlet o = {};\n\tlet x = 1,\n\t\t{ a, b } = $state(o);\n</script>\n{x}{a}{b}",
        "destructuring a $state declarator",
    );
}

#[test]
fn compile_state_destructure_refuses_comments() {
    // A carried script comment alongside a destructured state refuses — the 1→N
    // split scatters leaves across `tmp`/`$$array` intermediates whose comment
    // windows sweep. A safe over-refusal.
    assert_unsupported(
        "<script>\n\t// note\n\tlet { a } = $state({ a: 1 });\n</script>\n{a}",
        "comments in a script with a destructured $state declarator",
    );
}
