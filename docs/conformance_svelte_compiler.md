# Svelte Compiler Conformance

Where `tsv_svelte_compile` deviates from the canonical Svelte compiler's output —
and why.

## Contract

The oracle is Svelte's own `compile()` (server and client generation, run
deterministically: fixed `cssHash`, constant filename, non-dev). Parity is judged
on the **canonical reprint** of both sides' JS
(`tsv_svelte_compile::canonicalize_js`), so a divergence here is a *real code
difference*, never formatting.

**This catalog is expected to stay empty.** It exists as a safety valve — the
place to sanction a deliberate refusal to reproduce a genuine bug in the
canonical compiler's output — not as a tolerance budget. Any entry requires:

1. A fixture directory suffixed `_compiled_divergence` under
   `tests/fixtures_compile/`, carrying the input plus both expected outputs
   (the oracle's and ours).
2. A `README.md` in that fixture linking back to its catalog entry in this
   document.
3. A catalog entry below explaining exactly why matching the oracle is wrong.

`compile_conformance_audit` (gated in `deno task check`) enforces 1↔3 linkage
and the README back-link.

## Catalog

None. Every compile fixture matches the canonical compiler's output exactly
(after canonical reprint).
