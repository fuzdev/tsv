# Divergence: declarator before-`=` multiline block, authored break kept

A **multiline** block comment in a declarator's name→`=` gap that the author
**broke after** (`const a /* x⏎y */⏎= 1;`). The break after a multiline block is
authoring signal — the same rule the value gap applies (`= /* x⏎y */⏎1` hangs in
both formatters) — so tsv keeps it: the comment trails the name and `= value`
drops to a continuation line **indented one level** (the uniform
forced-continuation indent, the same landing as the line-comment sibling
[declarator_before_eq_line_comment](../declarator_before_eq_line_comment_prettier_divergence/)).
Prettier instead **relocates** the comment across the `=` and hangs it leading
the value (`const a =⏎\t/* x⏎y */⏎\t1;` — `output_prettier.svelte`).

```ts
// tsv (preserve + continuation indent)   // prettier (relocate past `=`, hang)
const a /* x                              const a =
y */                                      	/* x
	= 1;                                  y */
                                          	1;
```

A multiline block whose `=` shares its closing line (`const b /* x⏎y */ = 2;`)
stays glued — the not-broke-after form, kept by both formatters (the second
case). Only the authored break distinguishes the two, exactly as at the value
gap. A single-line block's breaks stay unforced and collapse either way (the
own-line-block sibling
[declarator_before_eq_own_line_block_comment](../declarator_before_eq_own_line_block_comment_prettier_divergence/)).

The declarator face of the rule; the enum-member and class-property `=` gaps
share the emitter. See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
