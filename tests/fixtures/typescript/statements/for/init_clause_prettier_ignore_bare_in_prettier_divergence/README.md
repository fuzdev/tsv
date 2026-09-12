# init_clause_prettier_ignore_bare_in_prettier_divergence

An `in` binary lexically under a `for` header's init is parenthesized so the header does
not read as a for-in head. The unfrozen clause places that pair at the `in`'s **own**
position, one per `in`. A **frozen** clause prints verbatim and has no inner positions, so
the single pair goes around the whole slice — and the question becomes the slice's: the
clause root need not be the `in`, only reach one through a position the grammar threads
`[~In]` into: a sequence operand, an assignment or compound value, a binary operand
(logical operators included), a conditional's test or alternate, an arrow's concise body, a
`yield` / `yield*` argument, and an `as` / `satisfies` operand.

The arrow-body cells freeze at the arrow's own `=>`→body head rather than at the clause's
`(`, so they exercise the same slice question one seam over: the root case takes its pair
from the ARROW-BODY position (through that position's own `needs_parens`, whose ambient `[~In]` rule fires there), a descended one
takes it from the slice.

```ts
// tsv                                   // prettier (does not reparse)
for (                                    for (
	// prettier-ignore                     	// prettier-ignore
	(ccc = 'aaa'  in  bbb);                	ccc = 'aaa'  in  bbb;
	;                                      	;
) {}                                     ) {}
```

## Why tsv differs

◆prettier_bug. Prettier's own rule is the mirror image and covers the same `in`s
(`isPathInForStatementInitializer` walks every ancestor to the root), but its ignore path
emits the slice bare, giving a four-clause header no parser accepts — tsc reports
`')' expected`, tsv's next pass rejects it, and so does prettier's own TypeScript parser.
Prettier is nevertheless idempotent on that output at the `.svelte` level, because
prettier-plugin-svelte catches that `SyntaxError` and passes the script block through
verbatim, so the absent `audit_signature.txt` is the ordinary F4b fixed point.

## Expected behavior

- **tsv**: one pair around the frozen slice; the input is a fixed point and reparses.
- **prettier**: the pair-less form (`output_prettier.svelte`), which no parser accepts.

The controls carry the two shapes that need no pair and must not grow one: an `in` the
author already parenthesized (the slice carries that pair) and a conditional's
**consequent**, which ecma262 gives `AssignmentExpression[+In]` whatever the conditional's
own parameter — so `for (ooo ? 'aaa' in bbb : qqq; ;)` parses bare. Every position the walk
omits is omitted because its operand cannot BE an unparenthesized `in` (a prefix operator
binds tighter: `typeof a in b` is `(typeof a) in b`, and the `in` is then the root) or
because it is `[+In]` in its own right (a call or `new` argument, an object value, a
computed index, a template substitution, a braced body).

⚠️ Two `[+In]` positions carry a spec-vs-tsc split: an **array element** and a **parameter
default**. The spec threads `[+In]` into both and V8 accepts them, but tsc reports
`',' expected`; tsv's parser sides with tsc on the arrow spelling, and its unfrozen printer
supplies the pair like prettier does. The frozen path supplies it for a BARE element or
default too; only a DESCENDED `in` there (`[ccc || 'aaa' in bbb]`) is left unparenthesized,
since the walk does not descend into a `[+In]` position. Pre-existing, and not
re-synthesized by the freeze.

## Reason

◆prettier_bug. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On delimiter-owned value heads*, the `[~In]` paragraph) and
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index).
The positions where both tools agree are the ordinary
[clauses_prettier_ignore_in_parens](../clauses_prettier_ignore_in_parens/).
