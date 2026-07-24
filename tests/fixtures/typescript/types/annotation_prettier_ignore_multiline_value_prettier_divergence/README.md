# annotation_prettier_ignore_multiline_value_prettier_divergence

A glued directive freezing a **multi-line** annotation value inside a width-decided
container (a parameter list) breaks the container cleanly — one parameter per line —
around the verbatim slice:

```ts
function fn(
	a: /* prettier-ignore */ {
		x:   1;
	},
	b: c1
) {}
```

Prettier keeps the parameter list FLAT and glues `, b: c1) {}` onto the frozen
slice's last line (`output_prettier.svelte`) — its printed-ignored slice is a plain
string, invisible to `willBreak`, so the container never learns it spans lines. tsv
forces the break explicitly (a frozen slice is `will_break`-opaque by design, so the
seam signals the containing groups) — the single-child analog of the
[multiline member](../union_prettier_ignore_multiline_member_prettier_divergence/) /
[tuple](../tuple_prettier_ignore_multiline_member_prettier_divergence/) /
[type-param](../type_params_prettier_ignore_multiline_member_prettier_divergence/)
divergences. `unformatted_ours_flat.svelte` carries the flat authoring; tsv
normalizes it to input (prettier keeps it flat).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
