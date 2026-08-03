# default_equals_own_line_block_comment_prettier_divergence

An own-line **block** comment in a binding default's name→`=` gap, across all
four default positions — object/array destructuring, parameter destructuring,
and a standalone parameter default (`{ a⏎/* c1 */⏎= 1 }`). A single-line block
forces nothing, and a comment in this gap trails the name (a trailing
position), so tsv collapses the authored breaks and keeps the comment inline in
its authored syntactic slot (`{ a /* c1 */ = 1 }` — the form both formatters
hold stable when authored inline, pinned as a match in
[default_equals_comment](../default_equals_comment/)). Prettier instead
**hoists** the comment to its own line leading the whole binding, expanding the
pattern or parameter list:

```ts
// tsv (collapse in place)        // prettier (hoist leading)
const { a /* c1 */ = 1 } = obj;   const {
                                  	/* c1 */
                                  	a = 1
                                  } = obj;
```

- `unformatted_ours_own_line.svelte` — the own-line authoring: tsv normalizes it
  to input; prettier takes it to the hoist instead (one pass — no intermediate).
- `variant_own_line.svelte` — prettier's landing form, dual-stable: a comment
  *authored* leading a binding keeps its own line in both formatters.

A third destination for this family: the `=`→**value** gap
([param_default_own_line_block_comment](../../../declarations/function/param_default_own_line_block_comment_prettier_divergence/))
collapses to after the `=` under tsv, before it under prettier. The same-gap
**line** comment (which forces the break → continuation indent) is
[default_equals_line_comment](../default_equals_line_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
