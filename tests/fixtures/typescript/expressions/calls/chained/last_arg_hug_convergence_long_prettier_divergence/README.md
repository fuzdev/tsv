# last_arg_hug_convergence_long_prettier_divergence

A member chain whose last call's argument (an object — bare, or an arrow's
parenthesized object body) is too wide to fit on any chain line while the chain's
head fits. Both formatters share one fixed point — the chain stays flat and the
argument breaks inside (the hug) — but prettier needs 2 passes to reach it from a
flat authoring; tsv converges in one.

## Pattern

Source (flat): `const a1 = expr.fn1().map((item1) => ({ …wide… }));`

- **Prettier pass 1** (unstable): the flat argument carries no forced break, so the
  chain's one-line measurement reads the whole flat content, overflows, and the chain
  expands — the argument, unfittable on the expanded line too, breaks inside it.
- **Prettier pass 2** (stable): the now-multiline argument re-reads as
  authored-expanded (forced break), the fit measurement truncates at that break and
  sees only the chain head, which fits — the chain collapses back to flat with the
  argument hugging.

tsv prints the pass-2 form directly, in every position the chain can sit — bare
initializer, call argument, property value, and a Svelte template expression —
for both window kinds (an arrow's parenthesized object body, and a bare object
argument). `unformatted_ours_flat` is the flat authoring (ours normalizes in one
pass; prettier's first pass is pinned by `prettier_intermediate_flat`);
`unformatted_expanded` is the broken-chain authoring, which both formatters take
to `input` in one pass.

The window is exact and the fixture pins its boundary: when the argument *does* fit
flat on the expanded chain line (the 100-char case), the broken chain is the shared
stable form and both formatters keep it — byte-matched, no hug. One char wider (101)
and the chain hugs.

## Reason

Prettier bug (non-idempotent): from the flat authoring prettier's first pass is not a
fixed point. tsv prints prettier's own settled form in one pass — a convergence-speed
divergence with a single authoring-independent fixed point.

See [conformance_prettier_ts.md §TypeScript](../../../../../../../docs/conformance_prettier_ts.md#typescript) (Member-chain wide-last-argument hug convergence).
