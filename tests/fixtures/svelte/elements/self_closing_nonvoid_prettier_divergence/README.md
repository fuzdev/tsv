# self_closing_nonvoid_prettier_divergence

tsv: normalizes to `<div></div>`
Prettier: preserves whichever form is used (both `<div />` and `<div></div>` are stable)

## Reason

**Spec precedence.** Self-closing tags do not exist in HTML. Per the HTML Standard's
`non-void-html-element-start-tag-with-trailing-solidus` parse error, a `/` before the `>` of a
start tag for an element that is neither void nor foreign content is a **parse error**, and "the
parser behaves as if the U+002F (/) is not present". The spec's own example is decisive:

```html
<div/><span></span><span></span>
```

parses to `html > head > body > div > span, span` — the spans become **children** of the `div`.
The spec states it directly: "The trailing U+002F (/) in a start tag name can be used only in
foreign content to specify self-closing tags. (Self-closing tags don't exist in HTML.) It is also
allowed for void elements, but doesn't have any effect in this case."

So the authored form is ambiguous *across readers*: Svelte's parser treats the `/` as self-closing
and produces an empty element, while an HTML parser ignores it and treats the tag as open. `<i />`
and `<i></i>` denote the same document only under Svelte. Normalizing emits bytes that mean the
same thing under both readings — which is why Svelte itself warns that the authored form is
ambiguous (`element_invalid_self_closing_tag`), why `svelte.migrate()` rewrites it, and why the
compiler's own `print()` never emits it for a non-void element.

This is also why the normalization is not an arbitrary allowlist: tsv permits self-closing exactly
where the `/` carries meaning — foreign content (SVG/MathML) and namespaced elements, where the
spec says it specifies a self-closing tag; void elements, where the spec allows it as a no-op; and
components, which are not HTML elements at all.

See the HTML Standard §13.2 (parse errors,
`non-void-html-element-start-tag-with-trailing-solidus`),
https://svelte.dev/e/element_invalid_self_closing_tag, and
[conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
