# destructure_member_target_prettier_divergence

A `{#await … then PATTERN}`, `{:then PATTERN}` or `{:catch PATTERN}` value may hold a
**member expression** as a destructuring target (`{#await p then [a.e]}`,
`{:catch { k: a.g }}`). Svelte's `read_pattern` parses the pattern as the left side of an
assignment (`(${pattern} = 1)`), and a member expression is a valid
`DestructuringAssignmentTarget`, so Svelte's parser accepts every case here. tsv matches the
canonical AST and formats the document as a fixed point.

prettier-plugin-svelte's block-pattern printer has no arm for a `MemberExpression` and
throws:

```
unknown node type: MemberExpression
```

so no format oracle exists. `prettier_rejects.txt` pins the error, and rule F6
live-verifies that prettier still throws on the input.

The fixture also guards the parser. `{:then}` and `{:catch}` reach `tsv_ts` through the
same cover-grammar conversion as a declaration's binding pattern, where a member target is
a syntax error (`let [a.b] = x`, pinned by
[member_target](../../../../typescript/expressions/destructuring/member_target/)). A binding
rule applied to the block pattern would over-reject every case below.

Covered: the `then` shorthand, and the `{:then}` and `{:catch}` branches. The `{#each}` face
is [each/destructure_member_target](../../each/destructure_member_target_prettier_divergence/).

See [conformance_prettier_svelte.md §Svelte: member-expression block-pattern target](../../../../../../docs/conformance_prettier_svelte.md#svelte-member-expression-block-pattern-target).
