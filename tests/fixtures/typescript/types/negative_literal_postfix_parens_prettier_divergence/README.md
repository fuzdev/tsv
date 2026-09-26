# Negative literal postfix-operand parens divergence

A negative literal type as the operand of a type's postfix brackets — an array element type
or an indexed-access object type. Prettier strips the parens; tsv keeps them:

- `type A = (-1)[];` → prettier `type A = -1[];`
- `type F = (-1)[K];` → prettier `type F = -1[K];`
- the same in every position the type can sit: `readonly (-1)[]`, `[...(-1)[]]`, an
  annotation, a type argument, an `as` / `satisfies` target, a template expression

## Reason

Prettier bug, and a Svelte-parser break. tsc reads the stripped text as the same type: `-` is
a negative literal type when a numeric or bigint literal follows, and its postfix loop then
takes the brackets. acorn-typescript — the parser Svelte uses — reads the literal with its
expression parser, which takes every `[…]` that follows as a computed member:

- `-1[]` is a syntax error at the `]`, so a `<script lang="ts">` that compiled stops
  compiling, and prettier's own next pass over the component throws;
- `-1[K]` is the literal type `-(1[K])`, a different tree.

With the parens every parser reads the type tsc reads.

The `unformatted_ours_parens` variant collapses redundant pairs to the one the operand needs.
A negative literal anywhere else strips in both formatters —
[negative_literal_redundant_parens](../negative_literal_redundant_parens/).

See [conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript).
