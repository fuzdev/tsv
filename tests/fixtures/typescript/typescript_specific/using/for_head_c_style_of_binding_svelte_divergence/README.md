# `using of` in a C-style `for` head — Svelte Divergence

`for (using of = a; ;)`: a using declaration whose binding is named `of`, as the init
of an ordinary three-part `for`. The head is a `LexicalDeclaration`, and `of` is an
ordinary `BindingIdentifier` there; the proposal's `[lookahead ≠ using of]` belongs to
the for-of `ForDeclaration` alone, where it makes `for (using of items)` a for-of over
the identifier `using` rather than a declaration of `of`.

So `using of` reads two ways in a `for` head, and one token past `of` decides: `=`,
`;` or `:` continues a declaration, anything else is the for-of over `using`. That is
tsc's rule (`nextTokenIsEqualsOrSemicolonOrColonToken`). `for (using of of items)`
matches neither reading — a declaration of `of` cannot be followed by `of`, and the
for-of form's right-hand side cannot start with `of` — and every parser rejects it
(`input_invalid_using_of_of`).

## Why tsv Differs

**tsc parses every spelling here** with no diagnostic, and prettier formats them (its
`babel` and `typescript` parsers both accept them). **acorn rejects them**, at the
oracle's `ecmaVersion: 2025` pin for the reason the sibling
[for_head_c_style](../for_head_c_style_svelte_divergence/) diverges (acorn reads
`using` in a `for` head only from ES2026), and at `'latest'` too: its for head tests
`isUsing(true)`, which declines `using of` outright, so the declaration of `of` is
read as a for-of whose right-hand side begins at `=`. `expected_svelte.json` is the
error marker.

The spelling with no initializer, `for (using of; ; )`, stays out: tsc's parser takes
it, but prettier's `typescript` parser raises TS1155 (`'using' declarations must be
initialized`), so it has no formatting oracle.

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
