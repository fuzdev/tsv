# `using` in a C-style `for` head — Svelte Divergence

`for (using x = a; ; )` and, in an async function, `for (await using x = a; ; )`: a
using declaration as the init of an ordinary three-part `for`. The proposal makes
`UsingDeclaration` a `LexicalDeclaration`, and `ForStatement` already takes a
`LexicalDeclaration` as its init (ecma262 sec-for-statement), so the head is the
same production `let` and `const` use there, declarator list included. The for-of
`ForDeclaration` spelling is the sibling fixture
([basic](../basic_svelte_divergence/), [await](../await_svelte_divergence/)); a
for-in head takes no using declaration and stays rejected on both sides.

## Why tsv Differs

**tsc parses it** with no diagnostic, and prettier formats it (its `babel`,
`typescript`, `oxc` and `meriyah` parsers all accept it), which is the accept test.
**acorn rejects it at the oracle's pin** — the canonical parse runs acorn at
`ecmaVersion: 2025`, and acorn reads `using` in a `for` head only from ES2026
(`isUsingKeyword` gates on the edition); at `'latest'` the same acorn accepts every
spelling here. The rejection is the edition pin, not a judgement about the grammar —
the same reason the sibling fixtures diverge — so `expected_svelte.json` is the error
marker.

The `input_invalid_using_for_in` variant leans on that pin too. The for-in head takes
no using declaration, and tsv rejects it at parse time as the unconditional-local rule
it is — but tsc's *parser* accepts it (TS1493 is its grammar checker's), and so does
acorn at `'latest'`, whose for-head path hands a using declaration to its for-in branch
without checking the kind. Once the oracle's pin reaches ES2026, acorn accepts that
input, and it has to leave `input_invalid_*` for a fixture of its own: tsv rejecting
what the canonical parser accepts (`tsv_rejects.txt`).

See [conformance_svelte.md §TypeScript Corrections](../../../../../../docs/conformance_svelte.md#typescript-corrections).
