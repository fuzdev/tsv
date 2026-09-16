# relational_chain_type_arg_parens_long_prettier_divergence

Width alone re-lexes a bare relational chain, and prettier takes the break without the pair that makes it safe.

tsv: `(aaa… < bbb…) >⏎ccc…`
Prettier: `aaa… <⏎bbb… >⏎ccc…`, which its own second pass writes out as `aaa…<bbb…>;` plus `ccc…;`

## Reason

**Semantic preservation.** Ahead of an identifier, a line terminator after the `>` settles the list as type arguments, to tsv, acorn-typescript and tsc alike (which followers a break commits, for which parser, is stated in the catalog entry linked below). Nothing else has to be unusual: three plain identifiers over 100 columns are enough, with no comment, blank line or object anywhere in the statement. Once the printer breaks the chain one operand per line, `aaa < bbb > ccc` re-parses as a `TSInstantiationExpression` plus a separate expression statement — two `BinaryExpression` nodes gone. Prettier's own second pass writes that out, pinned by `audit_signature.txt`.

tsv keeps a pair around the `>`'s left operand, which takes the region out of type-argument position at every layout. The pair is **layout-blind** — whether the `>` ends a line is not knowable where parens are decided — so `l1`, which never breaks, carries it too.

`input.svelte` pins the **post-fix layout boundary** — the one a fixture whose input is tsv's own fixed point can hold. With the pair, `l1`'s continuation line is exactly 100 columns and holds the chain flat; `l2`'s would be 101 and breaks after the `>`, the first layout that puts a line terminator at the join. `l3` is wide enough that the operands break one per line, which is the shape that loses nodes today.

## Where the loss begins

The **bug boundary** is a different width, and no cell of `input.svelte` can carry it: the pair costs two columns, so a cell whose bare form measures 100 has a paired form of 102 — already broken, with no "stays flat" left to claim. Measured on the bare spelling prettier emits, three plain identifiers on one `const` continuation line:

- **100 columns** — `aaa…(29) < bbb…(29) > ccc…(31);` stays flat, and re-parses as the program that was written: `BinaryExpression >` over `BinaryExpression <`.
- **101 columns** — `aaa…(29) < bbb…(29) > ccc…(32);` breaks one operand per line, and re-parses as a `TSInstantiationExpression` plus a separate `ExpressionStatement`. Both `BinaryExpression` nodes are gone.

`output_prettier.svelte`'s own width cells sit just under that line: bare, `l1` measures 98 and `l2` 99, both flat and both still the comparison chain — so neither shows the loss. `l3` is the cell that crosses it. Bare, its operands break one per line (52 / 52 / 51 columns) and the re-parse is the instantiation plus the free statement, which is what `audit_signature.txt` pins prettier's second pass writing out.

The as-authored half of the same bug (a break the printer REMOVES rather than adds) is [relational_chain_type_arg_parens](../relational_chain_type_arg_parens_prettier_divergence/); the object-operand half, where prettier's pass-1 output can end worse still, is [relational_chain_type_arg_parens_object_long](../relational_chain_type_arg_parens_object_long_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript (Relational chain type-argument parens).
