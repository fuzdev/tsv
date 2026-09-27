# pre_content_prettier_divergence

`<!-- format-ignore -->` inside `<pre>` freezes the next node, as `<!-- prettier-ignore -->` does
there: directly in the `<pre>`, inside a block section, inside an element, and inside a special
element. Prettier doesn't recognize `format-ignore`, so it formats those nodes anyway.

- `input.svelte` — tsv's form: every frozen node keeps its source text, and the
  `prettier-ignore` control keeps it in both formatters.
- `output_prettier.svelte` — prettier's output: each `format-ignore`d node formatted, the
  `prettier-ignore`d one untouched.
- `unformatted_ours_spaces.svelte` — extra spaces inside the tags and block heads the directives
  do not freeze: tsv normalizes them back to `input.svelte`.

Every change either formatter makes lands inside a tag or a `{…}`, so all three render the same
text.

## Reason

`format-ignore` is a tsv-native directive, honored identically to `prettier-ignore` at every
position tsv honors either. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
and [directives.md](../../../../../../docs/directives.md).

## Related

- [prettier_ignore/pre_node_kinds](../../prettier_ignore/pre_node_kinds/),
  [prettier_ignore/pre_hosts](../../prettier_ignore/pre_hosts/) and
  [prettier_ignore/pre_special_element_hosts](../../prettier_ignore/pre_special_element_hosts/) —
  the `prettier-ignore` spelling inside `<pre>`, where prettier agrees
