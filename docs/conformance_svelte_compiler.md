# Svelte Compiler Conformance

Where `tsv_svelte_compile` deviates from the canonical Svelte compiler's output —
and why.

## Contract

The oracle is Svelte's own `compile()` (server and client generation, run
deterministically: fixed `cssHash`, constant filename, non-dev). Parity is judged
on the **canonical reprint** of both sides' JS
(`tsv_svelte_compile::canonicalize_js`), so a divergence here is a *real code
difference*, never formatting.

**One mechanism-level, cataloged relaxation: comment position.** The parity
comparison (`compare_canonical`) tolerates two canonical forms that differ ONLY in
where comments sit — same code, same comment sequence, no bundler annotation. tsv
places comments by its own philosophy (a deliberate divergence from prettier/esrap);
in machine-consumed compiled output that position carries no correctness signal, so
the bar is relaxed to code + comment presence + semantic-comment binding rather than
raw bytes. This is a standing stance, not a per-fixture `_compiled_divergence` entry
(the catalog below stays empty). Full contract: the crate `CLAUDE.md` §The Parity Bar.

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

`compile_conformance_audit` (gated in `deno task check`) checks each
`_compiled_divergence` fixture for its catalog entry (1↔3) and its README
back-link (2). Both checks are per-fixture, so while the catalog is empty they
inspect nothing and gate nothing — they are a tripwire armed for the first
entry, not a standing gate. Two properties the parser-side `conformance_audit`
carries are deliberately absent here: it does not check this document for stray
READMEs on matching fixtures, and it does not check its own Markdown links —
those resolve under `conformance_audit`, which link-checks the compiler doc pair
alongside the parser conformance docs. The audit's one check that holds today,
independent of the catalog, is the checklist ↔ `Refusal` bucket-key drift check
(see [checklist_svelte_compiler.md](./checklist_svelte_compiler.md)).

## Catalog

None. Every compile fixture matches the canonical compiler's output exactly
(after canonical reprint).

## Candidates (recorded, not yet fixtures)

A candidate is a shape where the oracle's own output looks wrong and tsv's differs.
Recording it here is *not* an entry in the catalog above — no fixture exists, nothing
is sanctioned, and the audit is untouched. It is a note so a future burn-down does not
re-triage it as a compiler bug.

### Module-script comment teleported into the instance script

The oracle prints the transformed program with **esrap**
(`packages/svelte/src/compiler/phases/3-transform/index.js:4`), which binds a comment to
the next-following printed node **by source offset**. A `<script module>` placed *after*
`<script>` puts its comments at offsets that immediately precede an instance-script or
template expression — so the comment is re-attached across the module→instance boundary
and printed inside an expression it has nothing to do with. tsv omits it.

The most legible instance is a comment landing between `function` and its name:

```js
function // a module comment
  s($$renderer) {
```

Others land mid-expression (`$.attr('id', /* comment */ id)`, `value: /* comment */ w`).

Mechanically confirmed: moving the `<script module>` block **above** `<script>` — the
same component otherwise byte-for-byte — reaches full parity, the comment vanishing
entirely rather than moving. Every occurrence found by `compile_fuzz` had the module
script second.

tsv's behavior (omitting the comment) is the more defensible one, and the parity bar's
comment-position tolerance does not cover this — the comment crosses into an unrelated
subtree, which is a comment *presence* difference in that subtree, not a position one.
No fixture is proposed: the shape is an oracle print artifact, and pinning it would pin
esrap's offset behavior rather than anything about tsv.
