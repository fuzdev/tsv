# prettier_ignore_semicolon_export_prettier_divergence

An own-line directive in the `export`→declaration gap freezes the declaration, and a frozen
`VariableDeclaration` that relies on ASI gets the `;` restored — on both formatters
(`unformatted_ours_no_semicolons.svelte` → `const a  =  x;`). Without it the next
statement's printed form can fabricate a leading `(` that re-binds the pair into one broken
statement (`const a  =  x` + `y => 1` → `x(y) => 1`), output that does not reparse — the
`export` spelling of [prettier_ignore_semicolon](../prettier_ignore_semicolon/).

The divergence is the directive's PLACEMENT, not the semicolon: prettier **relocates** the
directive flush onto the `export` line (`export // prettier-ignore`) and freezes anyway;
tsv keeps the line the author gave it and indents the continuation — the same divergence
[named_declaration_prettier_ignore_head](../../../modules/exports/named_declaration_prettier_ignore_head_prettier_divergence/)
pins for the whole declaration-form family, load-bearing for the same reason: a
head-trailing directive is inert under the placement floor, so following the relocation
would lose the freeze on tsv's own second pass.

## Reason

Rule A binds a directive to the node that follows it; ◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
