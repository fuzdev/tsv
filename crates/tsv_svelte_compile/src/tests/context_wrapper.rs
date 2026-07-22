//! The `$$renderer.component(...)` wrapper: `needs_context` and import hoisting.

use super::support::*;

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
