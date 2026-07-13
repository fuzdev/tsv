# Svelte Compiler Conformance

Where `tsv_svelte_compile` deviates from the canonical Svelte compiler's output —
and why.

## Contract

The oracle is Svelte's own `compile()` (server and client generation, run
deterministically: fixed `cssHash`, constant filename, non-dev). Parity is judged
on the **canonical reprint** of both sides' JS
(`tsv_svelte_compile::canonicalize_js`), so a divergence here is a *real code
difference*, never formatting.

### Refusing is not diverging

Two different things can follow from an oracle bug, and only one of them lands
here.

**REFUSE** — the oracle is wrong (it emits invalid runtime JS, or it crashes)
and tsv declines to compile the shape at all. This is the ordinary path, and it
needs **no entry in this catalog**: the refusal contract already covers it. A
refusal is a `Refusal` variant with a stable bucket key, documented in
[checklist_svelte_compiler.md](./checklist_svelte_compiler.md). This class is
real and populated — e.g. `import x = require(…)`, `export = …`, and
`export as namespace …` all print verbatim inside the component function; an
`abstract` class property prints as `abstract x;`; a class index signature and a
dotted `namespace A.B` both make the oracle throw. tsv refuses each rather than
reproduce garbage.

**DIVERGE** — tsv compiles the shape *and deliberately emits different bytes*
than a working oracle. That is what this catalog is for, and it is why the
catalog is expected to stay **empty**: it is a safety valve, not a tolerance
budget. Any entry requires:

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
