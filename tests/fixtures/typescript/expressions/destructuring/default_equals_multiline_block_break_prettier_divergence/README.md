# Divergence: binding-default before-`=` multiline block, authored break kept

A **multiline** block comment in a binding default's name→`=` gap that the
author **broke after** (`{ a /* x⏎y */⏎= 1 }`), across the default positions —
object shorthand, object non-shorthand (the binding after the `:`), array
element, and a standalone parameter. The break after a multiline block is
authoring signal, so tsv keeps it: the comment trails the name and `= value`
drops to a continuation line **indented one level** (the uniform
forced-continuation indent, the same landing as the line-comment sibling
[default_equals_line_comment](../default_equals_line_comment_prettier_divergence/)).
Prettier keeps the comment where it was written but **glues** the `=` back onto
its closing line.

```ts
// tsv (preserve the break)   // prettier (glue)
const {                       const {
	a /* x                    	a /* x
y */                          y */ = 1
		= 1                   } = obj;
} = obj;
```

A multiline block whose `=` shares its closing line (`f /* x⏎y */ = 2`, the last
case) stays glued in both formatters — only the authored break distinguishes the
two. A single-line block's breaks stay unforced and collapse either way (the
own-line-block sibling
[default_equals_own_line_block_comment](../default_equals_own_line_block_comment_prettier_divergence/)).
In a **run** the multiline block leads, the break it forces carries the whole
rest of the gap: a later single-line block takes the continuation line too,
glued to the `=`. `unformatted_ours_flush.svelte` authors those continuations at
assorted other indents; tsv normalizes them all to input.

Unlike the declarator, property-key and binding-key faces of this rule, prettier
draws **no distinction** between the two authorings here — it glues both, at
every default position. The rule tsv applies is therefore its own rather than a
mirror of prettier's, and it is what keeps the break honored on **both** sides of
the `=`: the value side already hangs a broke-after multiline block
(`d = /* x⏎y */⏎\t\tv`), which prettier answers by relocating the comment back
before the `=`. The declarator face is
[declarator_before_eq_multiline_block_break](../../../declarations/variable/declarator_before_eq_multiline_block_break_prettier_divergence/).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
