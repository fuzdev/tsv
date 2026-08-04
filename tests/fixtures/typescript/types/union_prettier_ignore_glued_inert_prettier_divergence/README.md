# union_prettier_ignore_glued_inert_prettier_divergence

A **glued** ignore directive (`/* prettier-ignore */` on the same line as what
follows it) is **inert**: tsv honors a directive only when it is the only thing
on its line, so a glued directive is an ordinary block comment and the union
formats normally — whole-union position (`type A = /* prettier-ignore */ …`),
mid-list member positions (leaf, composite, intersection host), and from inside
a comment run alike.

Prettier honors the glued placement — it freezes the whole union at the leading
position and the adjacent member at a member gap. `prettier_variant_frozen.svelte`
holds those frozen forms (prettier-stable, internal perturbations kept); tsv
normalizes them to `input.svelte`. `unformatted_ours_perturbed.svelte` carries
whitespace perturbations that only tsv normalizes.

## Reason

The placement rule is total and exception-free: a directive freezes the
following construct only when it is alone on its line. Glued placements do not
appear in real corpora, and the one-sentence rule is more predictable than
prettier's placement-dependent scope changes.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
