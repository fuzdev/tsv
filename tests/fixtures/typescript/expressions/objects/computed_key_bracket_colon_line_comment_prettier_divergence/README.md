# Divergence: computed-key `]`→`:` line comment (object)

A line comment in a computed property key's `]`→`:` gap (`{ [x] // c⏎: 1 }`).
tsv keeps the comment after the `]` and drops `: value` to a continuation line
**indented one level** (uniform forced-continuation indent — the index-signature
layout). Without the break, the `//` would swallow `: 1` — content loss.

Prettier has **two destinations from this one gap, keyed on the authoring**:

- From the forced-break authoring (input) it **hoists** the comment to the
  property's own leading line (`{⏎\t// c⏎\t[x]: 1⏎}` — `output_prettier.svelte`).
- From the **own-line** authoring it instead crosses the `:` and hangs the
  comment leading the value (`[x]:⏎\t\t// c⏎\t\t1` — `variant_own_line.svelte`,
  dual-stable in both formatters; one pass, no intermediate).
  `unformatted_ours_own_line.svelte` pins tsv's side: the own-line authoring
  pulls up to trail `]` and normalizes to input in one pass.

```ts
// tsv (preserve + continuation indent)   // prettier (hoist to leading)
const p = {                               const p = {
	[x] // c                              	// c
		: 1                               	[x]: 1
};                                        };
```

The object face of the after-`]` separator gaps: the index-signature sibling is
[index_signature_bracket_colon_line_comment](../../../types/type_members/index_signature_bracket_colon_line_comment_prettier_divergence/)
(where prettier relocates into the brackets instead), the class `]`→`=` sibling is
[computed_key_bracket_line_comment](../../../statements/class/computed_key_bracket_line_comment_prettier_divergence/),
and the own-line **block** sibling is
[computed_key_bracket_colon_own_line_block_comment](../computed_key_bracket_colon_own_line_block_comment_prettier_divergence/).
See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
