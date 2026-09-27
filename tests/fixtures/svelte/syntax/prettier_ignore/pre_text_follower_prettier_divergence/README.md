# pre_text_follower_prettier_divergence

A `prettier-ignore` followed by text, then an element. The directive freezes the next node, and
the next node is the text: it keeps its bytes (as all text inside `<pre>` does anyway), and the
element after it formats. That is the reach the directive has everywhere — outside `<pre>`, and
inside a `<pre>`'s block sections — and tsv keeps it inside a `<pre>`'s element bodies too.

- `input.svelte` — tsv's form, which is also prettier's fixed point for it: prettier leaves
  every `<b a="1">` as written.
- `unformatted_ours_spaces.svelte` — extra spaces in every `<b>` tag: tsv normalizes all five
  back to `input.svelte`. Prettier normalizes only the two in a block section and outside
  `<pre>`, and keeps the three in an element body inside `<pre>` — the `<pre>` itself, an `<i>`
  and a `<svelte:element>` there — as written, which is `prettier_variant_spaces.svelte`.
- `prettier_variant_spaces.svelte` — prettier's fixed point for that authoring: the three `<b>`
  tags in an element body inside `<pre>` keep their extra spaces (and the `<svelte:element>`
  line, now past 100 characters, dangles its closing `>`), the other two are normalized. tsv
  normalizes it to `input.svelte`.

Every difference lands inside a tag, so all of them render the same text.

## Reason

Design choice. Prettier's reach there is an artifact of how it prints the content of an element
inside `<pre>`: it copies each text child straight from the source instead of printing it as a
node, so the text never takes the pending freeze and the freeze falls through to the next
non-text node. A block section's content is printed node by node, text included, so there the
text takes it — two reaches for one directive, keyed on whether the parent is an element or a
block. tsv gives the directive one reach: the next node, whatever it is, skipping only a
whitespace-only text (as both formatters do everywhere).

See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

## Related

- [pre_node_kinds](../pre_node_kinds/) — the directive directly before each node kind inside
  `<pre>`, where prettier agrees
