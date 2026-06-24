# constrained_extends_parens_prettier_divergence

Prettier strips parens that a constrained `infer` requires in a conditional
type's extends-type, producing output that **fails to re-parse** (acorn-typescript
rejects it). tsv keeps the parens so its output stays valid TypeScript.

Two distinct sites, same root cause — the constraint's `extends` and the
conditional's `? :` are ambiguous without the parens:

- **Nested-arrow return.** `M extends (() => () => infer U extends string) ? …`
  - tsv: keeps the parens
  - Prettier: `M extends () => () => infer U extends string ? …` (unparseable)
  - Prettier's `needs-parentheses` rule only inspects the *immediate* return
    type, so a constrained infer behind more than one arrow escapes it.
- **Conditional-type constraint.** `X extends infer U extends (A extends B ? C : D) ? …`
  - tsv: keeps the parens around the conditional constraint
  - Prettier: `X extends infer U extends A extends B ? C : D ? …` (unparseable)

## Reason

Prettier bug. `infer` only appears in a conditional's extends-type, so a trailing token always
follows the constraint. When the constraint (or a nested arrow's return) ends in
a position that abuts the enclosing `? :`, the parens are the only thing keeping
the parse unambiguous — TypeScript requires them. A bare `<T extends (A extends B
? C : D)>` type-parameter declaration is **not** affected (the `>` terminates it,
so Prettier strips those parens and tsv matches — see `../constrained_extends_parens/`).

## Non-diverging sibling

`SingleArrow` (`M extends (() => infer U extends string) ? …`) keeps its parens
in **both** formatters — Prettier's single-level rule covers it. It is included
to show the boundary: Prettier preserves the single-arrow form but drops the
nested-arrow and conditional-constraint forms.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§TypeScript "Constrained infer extends-operand parens".
