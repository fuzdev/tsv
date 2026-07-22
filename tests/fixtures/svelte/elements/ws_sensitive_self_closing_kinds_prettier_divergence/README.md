# ws_sensitive_self_closing_kinds_prettier_divergence

Self-closing forms on the whitespace-sensitive printing path — elements nested inside `<pre>`,
and `<textarea>` itself — one case per tag kind, with and without attributes. tsv applies the
**same** rule it applies everywhere else: `/>` is kept exactly where it carries meaning (component, foreign,
namespaced) and normalized away where it does not (plain non-void HTML). Prettier preserves
whichever form the author wrote, for every kind — so `prettier_variant_selfclosing.svelte` (the
all-`/>` authoring) is stable under prettier while tsv rewrites it to `input.svelte`.

| authored | tsv | why |
| --- | --- | --- |
| `<i />` / `<i class="x" />` | `<i></i>` / `<i class="x"></i>` | plain non-void HTML — self-closing tags don't exist in HTML |
| `<Comp />` / `<Comp a="1" />` | unchanged | a component is not an HTML element |
| `<svg:rect />` | unchanged | namespaced/foreign content — the spec gives `/` meaning here |
| `<textarea />` / `<textarea class="x" />` | `<textarea></textarea>` / `<textarea class="x"></textarea>` | plain non-void HTML, same as `<i>` — and the form matters here: since the `/` is ignored, `<textarea />` *opens* a raw-text element that swallows everything after it |

The long-attribute `<textarea>` case pins the wrap: normalizing does not change the
attribute layout, and the close tag hugs the last attribute exactly as an authored
explicit-empty `<textarea …></textarea>` does (the layout pinned in
[elements/textarea_attrs_long](../textarea_attrs_long/)).

Whitespace-sensitivity does not enter into it. `<pre>` makes *content* whitespace literal; it
says nothing about how a tag serializes, and rewriting `<i />` to `<i></i>` adds no characters to
the rendered text. Before this was unified, the whitespace-sensitive path answered the
close-form question on its own and got it wrong in **both** directions, split by whether the
element had attributes: with attributes it preserved `/>` for every kind (wrong for `<i … />`),
without attributes it expanded every kind (wrong for `<Comp />` and `<svg:rect />`). The rule now
comes from one place — the same `can_self_close` the regular element path uses.

See [elements/pre_void_element](../pre_void_element/) for the void half of the same unification
(a void element has no closing tag in any context), and
[elements/self_closing_nonvoid](../self_closing_nonvoid_prettier_divergence/) for the spec
grounding — the HTML Standard's `non-void-html-element-start-tag-with-trailing-solidus` parse
error, where "the parser behaves as if the U+002F (/) is not present".

See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
