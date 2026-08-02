# fill_spaced_tag_travel_long_prettier_divergence

A `{expr}` tag separated from the preceding word by whitespace, whose expression **can
break**: the text→tag boundary measures the tag as a whole flat unit (pairwise — last word,
separator, tag), so a tag that does not fit flat starts on a fresh line — collapsing flat
there when it fits (the expression stays intact), breaking internally there when even a
full line cannot hold it. The tag never opens mid-line: the wide-element rule's tag analog
(content that cannot fit flat starts on a fresh line rather than tearing open at the end of
the text line). The spaced sibling of `fill_glued_tag_travel_long_prettier_divergence`,
where the welded word+tag pair travels as one unit; here the whitespace boundary sits
directly before the tag, so the tag travels alone.

Prettier's boundary measurement stops at the expression's first internal break, so it
reports a fit, keeps the tag on the text line, and opens it mid-line
(`… aaaa {cond1 === cond2` / `? 'aaa'` / `: 'bbb'}`). That form is prettier-stable — see
`prettier_variant_midline.svelte` — and prettier also keeps the traveled form, so
`input.svelte` is a fixed point of **both** formatters and the divergence is one of
normalization: which form the other authorings converge to.

The first and second cases pin the exact boundary: at exactly 100 the tag packs flat onto
the text line (a form both formatters keep); at 101 it travels and collapses flat on the
fresh line. The third case is a far wider expression that still fits flat on its own line
(travel + collapse, at 99); the fourth is too wide even for a full line, so it travels
first and breaks internally there.

`unformatted_ours_compact.svelte` is the one-line authoring: tsv → `input.svelte`, prettier
→ the mid-line-open forms of `prettier_variant_midline.svelte`.

## Reason

Design choice — the wide-element drop's tag analog, uniform with the glued pair travel
(`fill_glued_tag_travel_long_prettier_divergence`) and the welded-run travel
(`inline_break_before_glued_long_prettier_divergence`): the whitespace boundary in front of
the tag is render-free, so breaking it costs nothing, and starting the tag on a fresh line
keeps the expression intact where the mid-line form tears it open at the widest column.
See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).

See also: `fill_break_before_expr_long_prettier_divergence/` (the flat-tag boundary, where
the divergence is prettier's one-past-printWidth pack) and
`fill_expr_travel_continuation_long_prettier_divergence/` (text continuing after the
traveled tag).
