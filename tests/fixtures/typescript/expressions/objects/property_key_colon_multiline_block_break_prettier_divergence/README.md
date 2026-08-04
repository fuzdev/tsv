# Divergence: object-property key→`:` multiline block, authored break kept

A **multiline** block comment in an object property's key→`:` gap that the
author **broke after** (`key1 /* a⏎b */⏎: 1`). The break after a multiline block
is authoring signal — the same rule the value gap applies — so tsv keeps it: the
comment trails the key and `: value` drops to a continuation line **indented one
level** (the uniform forced-continuation indent, the same landing as the
line-comment sibling
[property_key_colon_line_comment](../property_key_colon_line_comment_prettier_divergence/)).
Prettier instead **hoists** the comment to its own line leading the key
(`/* a⏎b */⏎key1: 1` — `output_prettier.svelte`).

```ts
// tsv (preserve + continuation indent)   // prettier (hoist to leading)
const o = {                               const o = {
	key1 /* a                             	/* a
b */                                      b */
		: 1                               	key1: 1
};                                        };
```

A multiline block whose `:` shares its closing line (`key2 /* c⏎d */: 2`) stays
glued — the not-broke-after form, kept by both formatters (the second case).
Only the authored break distinguishes the two, exactly as at the value gap. A
single-line block's breaks stay unforced and collapse either way (the
own-line-block sibling
[property_key_colon_own_line_block_comment](../property_key_colon_own_line_block_comment_prettier_divergence/)).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
