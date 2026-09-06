# ws_sensitive_style_value_prettier_divergence

A `style:` directive whose quoted value holds a whitespace-only run, on the two
whitespace-sensitive elements. tsv gives that run its own break and re-indents the
continuation to the attribute's column — the same answer it gives on every other element
([directives/style/multiline_value](../../directives/style/multiline_value/)) — so the break
wraps the head. Prettier formats the identical value on a `<div>` and leaves it alone here:
its raw-text guard `isPreTagContent` is satisfied by any `pre` / `textarea` ancestor, so the
value's text comes back verbatim, carrying no break for the head to wrap around.

tsv:

```svelte
<pre
	style:transform-origin="{expr1}
	{expr2}">text</pre>
```

Prettier keeps the head on one line and the value exactly as authored — the space-indented
authoring is a second fixed point for it, pinned by `prettier_variant_spaces.svelte`.

An **ancestor** is enough for prettier's guard, so the last case sits the directive on a
nested `<span>` and both formatters answer it exactly as they answer the cases above — tsv
wraps the head and hugs the `>`, prettier keeps the head on one line. That parity is the
nested case's whole point, and it is the head shape
[ws_sensitive_head_attrs_wrap](../ws_sensitive_head_attrs_wrap/) pins directly: a
whitespace-sensitive head answers a breaking list one way whether the element is the `<pre>`
itself or an inline element inside it.

The break lands **inside the tag**, where no character is element content, so the render is
unchanged — the same licence
[ws_sensitive_attr_comment_line](../ws_sensitive_attr_comment_line_prettier_divergence/)
spends. What is whitespace-sensitive here is the element's *content*; a `style:` value is a
CSS property value wherever it is written, so tsv answers it the same way in every host. The
contrast case pins the discriminator: a plain attribute value is opaque to both formatters
here as everywhere, and both wrap the head for it.

See [conformance_prettier_svelte.md §Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).
