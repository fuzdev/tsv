# Reserved-keyword qualified type head (`void.X` / `null.X`) — Svelte Divergence

A type keyword immediately followed by `.` is the HEAD of a qualified type name
(`string.X` → a `TSTypeReference` over a `TSQualifiedName`). acorn-typescript's
`tsParseNonArrayType` accepts this for every keyword-type name *plus* the
reserved `void`/`null`, so `void.X` / `null.X` parse as a `TSQualifiedName`.

**tsv** follows tsc and prettier: `void`/`null` are reserved operators, not
entity-name heads, so tsv qualifies only the *contextual* type keywords
(`string`/`number`/`any`/`undefined`/…) and rejects the reserved heads
(`Expected ';'`). This is deliberate tsc-over-acorn strictness — the reverse
direction of most parser corrections, where tsv is broader.

Because the canonical parser accepts these inputs, the rejection cannot be an
`input_invalid_*` fixture (which requires both parsers to reject). This
`tsv_rejects.txt` fixture pins the divergence from the other side: tsv rejects
(`tsv_rejects.txt` substring), while `expected_svelte.json` proves acorn still
accepts. The contextual-keyword accept direction is pinned by the sibling
[type_keyword_qualified_head](../type_keyword_qualified_head/) fixture, whose
`input_invalid_true_qualified_head` pins the both-reject `true.X`.

**Upstream**: @sveltejs/acorn-typescript — `tsParseNonArrayType` accepts
`void`/`null` as qualified-name heads.

See [conformance_svelte.md](../../../../../docs/conformance_svelte.md) §TypeScript Corrections
(Reserved-keyword qualified type head).
