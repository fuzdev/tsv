# root_leading_nbsp_prettier_divergence

A non-breaking space at the **root fragment's** boundary is content, so tsv keeps it. Prettier
deletes it.

## Reason

The boundary trim removes render-free whitespace, and it stops at content — an NBSP is content
everywhere else already ([inline_boundary_whitespace](../inline_boundary_whitespace_prettier_divergence/),
[text_non_breaking_whitespace](../text_non_breaking_whitespace_prettier_divergence/)). The root
fragment is the one place that used a **wider** whitespace class than the rest of the printer
(Rust's `str::trim`, i.e. the Unicode `White_Space` property) and so deleted it. Svelte's compiler
keeps it: `clean_nodes` classifies with `regex_not_whitespace` = `/[^ \t\r\n]/`, which matches
U+00A0, so the node is not whitespace-only and survives — `&nbsp;<div>block</div>` compiles to
`<!---->&nbsp;<div>block</div>`. Deleting it is content loss, and the same class as
[text_form_feed](../text_form_feed_prettier_divergence/), one level out. See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

**The line break after it is render-free**, which is why `input.svelte` may carry one: whitespace
at a block-level boundary is not rendered, so `&nbsp;<div>` and `&nbsp;⏎<div>` are the same render
(confirmed by the `svelte-render-key` oracle, which reduces both to `["text:\u{a0}", "text:block"]`).
Only the NBSP itself is visible, and only its deletion changes the render.

## Cases

- `unformatted_ours_leading_nbsp.svelte` — the NBSP glued to the block element.
- `unformatted_ours_leading_nbsp_mixed.svelte` — the NBSP followed by a collapsible run. The ASCII
  half of the run is render-free and normalizes away; the NBSP does not.

Both reach `input` under tsv. Prettier deletes the NBSP outright, reaching `output_prettier.svelte`
— which is the *whole* divergence: nothing here is about layout.
