# else_blank_before_comment_prettier_divergence

An own-line comment before `else` (line or block) with a **blank line above it**
(`}\n\n// c\nelse`): **tsv drops** the blank, **prettier preserves** it.

tsv never lets a body block's opening `{` sit below a blank line — an authored
blank in a control-flow gap before a body-leading comment is always dropped,
uniformly across `if`/`while`/`for`/`do`/`else`/`try`/`catch`/… (and matching
tsv's own handling when `{` is on the header line). For the `}`→`else` gap
prettier treats the blank as a statement-level gap and keeps it, so tsv diverges
here; `prettier_variant_blank_before.svelte` pins prettier's stable form (tsv
normalizes it back to input). The comment's *position* (before `else`) is
unchanged in both formatters — only the blank differs.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
