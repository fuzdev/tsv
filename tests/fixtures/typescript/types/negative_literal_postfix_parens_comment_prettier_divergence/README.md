# Negative literal postfix-operand parens with comments divergence

The kept parens of a negative literal array element or indexed-access object type
([negative_literal_postfix_parens](../negative_literal_postfix_parens_prettier_divergence/)),
with a comment inside the parens or between them and the brackets. tsv keeps the parens and
the comment where it was written, as it does for any retained paren shell (`(keyof A)[]`).
Prettier strips the parens and relocates the comments:

- `(/* c */ -1)[]` → `/* c */ -1[]`
- `(-/* c */ 1)[]` → `-(/* c */ 1)[]`, which no parser accepts
- `(⏎-1 // c⏎)[K]` → `-1[K]; // c`
- `(-1)[/* c */]` → `-1 /* c */[]`

## Reason

Prettier bug, and a Svelte-parser break: without the parens acorn-typescript, the parser
Svelte uses, rejects the array form and reads the indexed-access form as the different
literal type `-(1[K])`. The comment rides the kept pair.

See [conformance_prettier_ts.md §TypeScript](../../../../../docs/conformance_prettier_ts.md#typescript).
