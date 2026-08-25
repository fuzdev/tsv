# inline_adjacent_comment_space_long_prettier_divergence

A space-separated run-ending follower — a comment, a `{@debug}` — after a component, at the
print-width boundary of the **element**. At exactly 100 chars everything stays inline and the
separator is the rendered space (both formatters). At 101 the `<p>` overflows and goes
block-style; its content line is then only 97 wide, and the follower keeps its authored space on
it (`<Comp /> <!-- c -->text2`). Prettier's `line` before a comment breaks with the container
instead (`output_prettier.svelte`).

`unformatted_ours_inline.svelte` is the one-line authoring of every case: tsv lays the 101-char
elements out block-style and normalizes to `input.svelte` in one pass.

`variant_split.svelte` is prettier's form itself, and tsv keeps it stable too — the newline spelling of
every boundary is held — so the two forms are two fixed points; a rule change that started
normalizing prettier's form back to `input.svelte` fails this file rather than passing in prose.

## Reason

Design choice — the rule of
[inline_sibling_space_before_bounding](../inline_sibling_space_before_bounding_prettier_divergence/)
(which carries the follower's own 100/101 boundary), met here by the element's block-style
conversion: a space before a comment or a `{@debug}` is that follower's own per-width wrap after
any sibling, as it already is after text, and the container's break is no reason to turn it into
a newline. This fixture used to pin the opposite — a bare break resolved by the container —
because the inline and multiline arms then disagreed on this separator and the wrap was a
two-pass cycle; with both arms on the wrap, the hazard is gone.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
