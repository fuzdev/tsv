# constrained_extends_parens_prettier_divergence

Prettier strips parens that a constrained `infer` requires in a conditional
type's extends-type, producing output that **fails to re-parse** (acorn-typescript
rejects it). tsv keeps the parens so its output stays valid TypeScript.

The diverging site is a **nested-arrow return** ending in a constrained infer:

- **Nested-arrow return.** `M extends (() => () => infer U extends string) ? …`
  - tsv: keeps the parens
  - Prettier: `M extends () => () => infer U extends string ? …` (unparseable)
  - Prettier's `needs-parentheses` rule only inspects the *immediate* return
    type, so a constrained infer behind more than one arrow escapes it.

## Reason

Prettier bug. `infer` only appears in a conditional's extends-type, so a trailing token always
follows the constraint. When a nested arrow's return ends in a position that abuts
the enclosing `? :`, the parens are the only thing keeping the parse unambiguous —
TypeScript requires them. A bare `<T extends (A extends B ? C : D)>` type-parameter
declaration is **not** affected (the `>` terminates it, so Prettier strips those
parens and tsv matches — see `../constrained_extends_parens/`).

## Non-diverging siblings

Two contrast cases keep their parens in **both** formatters:

- `CondConstraint` (`X extends infer U extends (A extends B ? C : D) ? …`) — a
  conditional-type infer constraint. Prettier keeps the parens here; tsv matches.
- `SingleArrow` (`M extends (() => infer U extends string) ? …`) — Prettier's
  single-level rule covers it.

They mark the boundary: Prettier preserves the single-arrow and
conditional-constraint forms but still drops the nested-arrow form.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§TypeScript "Constrained infer extends-operand parens".
