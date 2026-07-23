# multiline_value_inline_long_prettier_divergence

An inline `<span>` preceded by same-line text, whose attribute value contains a literal
newline forcing the opening tag to wrap, followed by long trailing text. tsv breaks at the
whitespace boundary before the `<span>` so it starts a **fresh line** rather than dangling its
opening tag after `text`; the long trailing text after `</span>` then fills and wraps at word
boundaries within printWidth.

tsv: `text` on its own line, `<span` on the next, and the trailing text wraps to stay ≤100.
Prettier: keeps `text <span` on one line, dangles the attributes, and lets the trailing text
run past printWidth (no break point after a breaking closing tag) — see `output_prettier.svelte`.

The boundary before the `<span>` is inter-node whitespace (render-free under Svelte 5), so the
break is render-equivalent.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
