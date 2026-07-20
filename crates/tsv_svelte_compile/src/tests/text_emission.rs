//! Static text/element emission: whitespace collapse, entities, template escapes.

use super::support::*;
use crate::*;

#[test]
fn compile_static_element() {
    let out = compile_checked("<p>text</p>");
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
    let out = compile_checked("<script>\n\tlet { prop } = $props();\n</script>\n\n<p>{prop}</p>\n");
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
    let out = compile_checked("<p>a`b\\c</p>");
    assert!(
        out.js.contains("`<p>a\\`b\\\\c</p>`"),
        "template metachars must be escaped: {}",
        out.js
    );
    assert_eq!(canonicalize_js(&out.js).unwrap(), out.js);
}

#[test]
fn compile_collapses_sibling_whitespace() {
    // Inter-sibling whitespace runs (newlines, blank lines) collapse to one
    // space; element-boundary whitespace trims (the oracle's clean_nodes).
    let out = compile_checked("<p>text1</p>\n\n<div>\n\t<p>text2</p>\n\t<p>text3</p>\n</div>\n");
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
    let out = compile_checked("<script>let { a } = $props();</script>\n<p>text  x {a} y</p>");
    assert!(
        out.js.contains("`<p>text  x ${$.escape(a)} y</p>`"),
        "interior/expr-adjacent whitespace mangled: {}",
        out.js
    );
}

#[test]
fn compile_preserves_pre_whitespace() {
    let out = compile_checked("<pre>  a\n  b  </pre>");
    assert!(
        out.js.contains("`<pre>  a\n  b  </pre>`"),
        "pre whitespace not preserved: {}",
        out.js
    );
}

#[test]
fn compile_marks_text_first_root_fragment() {
    let out = compile_checked(" x <p>text</p> ");
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
    let out = compile_checked("<p>&amp; &lt; &gt; &quot;</p>");
    assert!(
        out.js.contains("`<p>&amp; &lt; > \"</p>`"),
        "entity decode/re-escape wrong: {}",
        out.js
    );
    // Attribute values re-escape &, ", and < (escape_html attr rule).
    let out = compile_checked("<p title=\"&amp; &lt; &gt; &quot;q\">text</p>");
    assert!(
        out.js.contains(" title=\"&amp; &lt; > &quot;q\""),
        "attribute entity escaping wrong: {}",
        out.js
    );
}
