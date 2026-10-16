# relational_sibling_authored_pair_comment_prettier_divergence

Comments around the pair [relational_sibling_authored_pair](../relational_sibling_authored_pair_prettier_divergence/) keeps, in a call, an array and a sequence: each prints once, beside the same tokens, and the pair stays. Prettier strips the pair, and its second pass prints each line as a generic call with the comment inside the type-argument list.

tsv: `fn((a < b) /* k */, c > (d, e))`, `fn(⏎\t(a < b), // k⏎\tc > (d, e)⏎)`
Prettier: `fn(a < b /* k */, c > (d, e))`, `fn(⏎\ta < b, // k⏎\tc > (d, e)⏎)`, which its second pass prints as `fn(a<b /* k */, c>(d, e))` and `fn(⏎\ta<⏎\t\tb, // k⏎\t\tc⏎\t>(d, e)⏎)`

## Reason

**Semantic preservation** — the main fixture's reason; the comments change nothing about it. A comment is no token to the type grammar, so with the pair gone the `<` and the later element's `>` bound one list whatever stands between them, and `audit_signature.txt` pins prettier's second pass printing that list for every cell.

What the cells pin is where the comment prints around the kept pair, the placement tsv gives the pair an assignment element prints (`fn((a = b) /* k */, c)`):

- a block leading the comparison stays inside the pair, after its `(`;
- a block ahead of the pair stays ahead of it, a sequence's later operand included;
- a block behind the pair stays there, ahead of the element's comma;
- a `//` behind the pair ends the element's line, and the list breaks one element a line — at a sequence statement the operands after it continue on an indented line;
- a `//` glued to the list's own `(` or `[` keeps that line, the opening-delimiter rule ([comments.md](../../../../../../docs/comments.md) §The delimiter-line question). Prettier moves it to a line of its own there, as at every opening delimiter.

## Variants

`unformatted_ours_inside_pair` writes the comments that trail the pair INSIDE it: a block before the pair's `)` (`(a < b /* k */)`) and a `//` before it (`(a < b // k⏎)`). tsv prints both as `input.svelte` does, behind the `)` — the block ahead of the comma, the `//` after it, where an assignment element's pair prints them; the separator carries no meaning there, so the move is lossless. Prettier lands the variant on `output_prettier.svelte`, so the chain `audit_signature.txt` pins covers it too.

See [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens, "An authored pair around an earlier comma sibling") and [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation (the opening-delimiter rule at a paren shell, "Paren shell `(`, GLUED leading line comment").
