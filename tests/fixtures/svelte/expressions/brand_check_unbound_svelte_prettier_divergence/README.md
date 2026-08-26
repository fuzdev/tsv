# The unbound brand check in template positions — a tracked over-acceptance

`#x in y` is the sole production in which a private name stands as an operand
(`RelationalExpression : PrivateIdentifier in ShiftExpression`), and it is in the grammar
unconditionally. What confines it is **binding, not containment** —
`AllPrivateIdentifiersValid`, a whole-`Script` early error tsv defers — so tsv's expression
parser accepts it everywhere an expression is read, the template included.

## What each side does

- **Svelte** parses each template expression in isolation with acorn, which tracks the
  binding rule in the parser and rejects (`Unexpected token`). It rejects even with the name
  bound in an enclosing `<script>` class, since the island never sees that class.
  `expected_svelte.json` holds the parse-failure marker.
- **prettier** (via prettier-plugin-svelte) reaches the template through Svelte's parser and
  inherits that verdict, so no format oracle exists — which is what `prettier_rejects.txt`
  pins.
- **tsv** accepts, and formats the input as a fixed point — `expected_ours.json`.

## Why these two spellings

They are what survives the sequence placement guard, and the guard is why the set is only
two. A `{#…}` marker in an attribute value or in `<textarea>` content is a block written
where only a sequence belongs, and is rejected there with Svelte's own wording — at the
separated `{ #x in y}` spelling too. Neither survivor carries a marker:

- `{...#x in y}` — a spread. The `.` is no marker, and the attribute position reads
  `...` before it reads anything else.
- `{/* c */ #x in y}` — a comment lead. `skip_svelte_ws` is whitespace only, so a comment
  leaves the brace's interior an expression on tsv's side, exactly as an unprefixed `{`
  would.

Both are fixed points, which is what makes them pinnable at all: the whitespace-separated
spelling is not, because tsv's printer normalizes `{ #x in y}` to `{#x in y}`.

## Why this fixture exists

It is the ledger entry for a claim that otherwise lives only as prose. The acceptance
belongs to the expression parser, so it is reachable from every expression position, and the
`<script>` half of it is pinned by
[typescript/expressions/private_brand_check_unbound](../../../typescript/expressions/private_brand_check_unbound_svelte_divergence/);
without this fixture the template half is asserted nowhere a gate reads.

See [conformance_svelte.md §TypeScript Corrections](../../../../../docs/conformance_svelte.md#typescript-corrections)
("Brand check with no binding class") for the oracle table and the reject-vs-defer argument,
and [conformance_prettier_svelte.md §Svelte: prettier inherits Svelte's parse verdict](../../../../../docs/conformance_prettier_svelte.md#svelte-prettier-inherits-sveltes-parse-verdict)
for the formatter half.
