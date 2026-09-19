# `(`-headed type-argument region whose content is a parameter list — Svelte Divergence

A `<` region opening on a `(` is a type-argument list only if what the paren holds
is a type. tsv grades that body, so `a < (b || c) > (t, u)` and every other
paren-headed region whose content is plainly an expression read as the comparison
chain both oracles read. **One family is the exception**, and it is tsc's own: a
group whose content spells a **parameter list**.

`isUnambiguouslyStartOfFunctionType` (`src/compiler/parser.ts`) claims `( )`,
`( ...`, `( ident :`, `( ident ,`, `( ident ?`, `( ident =` and `( ident ) =>` for
a function type before any body is read, so tsc commits to the type-argument list
and then reports `'=>' expected.` — `a < (b = c) > (t, u)` is a syntax error to the
compiler at every committing follower, and at a `>` that ends a line too.
acorn-typescript instead backs off the failed list and reads the comparison chain.

tsv follows **tsc** here: the compiler's verdict is a grammar decision about the
`(`-headed region, not the error recovery that produces its other rejections, and
the four spellings above are the ones where a parameter list is the only reading of
the parenthesized text. `expected_svelte.json` records what acorn-typescript keeps,
so the divergence is pinned from both sides; `tsv_rejects.txt` pins tsv's own error.

Because the canonical parser accepts the input, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject), and with no
accepted parse there is nothing for a formatter to claim — hence no `expected.json`
and no format-claim siblings.

The **printer** side of the same region is separate and unaffected: a shell the
printer keeps still takes a paren pair around the `>`'s left operand, so tsv never
emits one of these bare. See
[conformance_prettier_ts.md §Relational chain type-argument parens](../../../../../../docs/conformance_prettier_ts.md).

See [conformance_svelte.md](../../../../../../docs/conformance_svelte.md) §TypeScript
Corrections (paren-headed type-argument region whose content is a parameter list).
