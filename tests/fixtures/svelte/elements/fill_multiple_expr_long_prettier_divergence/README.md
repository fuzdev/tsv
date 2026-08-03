# fill_multiple_expr_long_prettier_divergence

Fill content with multiple expression tags that exceeds printWidth at a deep indent: tsv
keeps the opening tag hugging and breaks the run at the whitespace boundary in front of the
first welded unit that no longer fits — `text2{…}` travels whole to the next line and every
expression stays intact. Prettier instead reads the compact authoring as an instruction to
dangle the opening bracket (`<small⏎>{…`), keeps the whole run on one line at 101+, and
opens the last ternary mid-line (`prettier_variant_compact.svelte`, its stable form).

Prettier also keeps the traveled form, so `input.svelte` is a fixed point of **both**
formatters and the divergence is one of normalization: `unformatted_ours_compact.svelte`
(the one-line authoring) converges to `input.svelte` under tsv and to the dangle form under
prettier.

## Reason

Print width. tsv treats printWidth as a hard limit, and the whitespace boundaries in the
run are render-free, so it spends one — keeping `<tag>{content}` on one line and every
expression whole — where prettier overshoots and tears a ternary open at the widest column.
The travel rule is `fill_multi_expr_travel_long_prettier_divergence`'s; this fixture pins it
at an indent deep enough that prettier switches to the bracket dangle.

See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
