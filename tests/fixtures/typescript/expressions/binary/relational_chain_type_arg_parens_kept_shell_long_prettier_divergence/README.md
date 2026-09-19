# relational_chain_type_arg_parens_kept_shell_long_prettier_divergence

Width alone re-lexes a bare kept-shell chain, and prettier takes the break without the pair that makes it safe.

tsv: `(aaa… < (bbb… = q)) >⏎ccc…`
Prettier: `aaa… < (bbb… = q) > ccc…`, which at the width below breaks one operand per line and no longer parses

## Reason

**Semantic preservation.** A shell the printer KEEPS puts a `(` at the head of the `<`…`>` region, and a `(`-headed region is a type-argument list to any reader that grades bracket matching and the follow token rather than the body — the class [relational_chain_type_arg_parens_kept_shell](../relational_chain_type_arg_parens_kept_shell_prettier_divergence/) enumerates. Flat, the follow token past the matching `)` decides, and an identifier follower keeps the chain a comparison. Once the printer breaks after the `>`, a line terminator sits at the join and the region commits: `aaa… < (bbb… = q) > ccc…` re-parses as an instantiation over a body no parser accepts, and the output is one tsv itself rejects (`Expected ')', found '='`).

tsv keeps a pair around the `>`'s left operand, which takes the region out of type-argument position at every layout. The pair is **layout-blind** — whether the `>` ends a line is not knowable where parens are decided — so `l1`, which never breaks, carries it too.

`input.svelte` pins the **post-fix layout boundary**, the one a fixture whose input is tsv's own fixed point can hold. With the pair, `l1`'s continuation line is exactly 100 columns and holds the chain flat; `l2`'s would be 101 and breaks after the `>`, the first layout that puts a line terminator at the join.

## Where the loss begins

The **bug boundary** is two columns below that, and no cell of `input.svelte` can carry it: the pair costs two columns, so a cell whose bare form measures 100 has a paired form of 102 — already broken, with no "stays flat" left to claim. Measured on the bare spelling, one assignment-shelled operand on a `const` continuation line:

- **100 columns** — `aaa…(36) < (bbb…(31) = q) > ccc…(16);` stays flat, and re-parses as the program that was written: `BinaryExpression >` over `BinaryExpression <`.
- **101 columns** — `aaa…(36) < (bbb…(31) = q) > ccc…(17);` breaks one operand per line, and no longer parses at all: the region's `(` head plus the line terminator past the `>` make it a type-argument list whose body is an assignment.

`output_prettier.svelte`'s own width cells sit under that line — bare, `l1` measures 98 and `l2` 99, both flat and both still the comparison chain — so neither shows the loss on its own; the measurement above is what names where it starts.

The as-authored half of the class, with the operator sweep and the leftmost-printed-spine cells, is [relational_chain_type_arg_parens_kept_shell](../relational_chain_type_arg_parens_kept_shell_prettier_divergence/); the plain-identifier half of the same width bug is [relational_chain_type_arg_parens_long](../relational_chain_type_arg_parens_long_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
