# instantiation_operand_left_side_shell_comment_prettier_divergence

An own-line comment inside the grouping parens of an instantiation expression's operand, or of
that operand's own leftmost node (`x = (// c⏎a)<T>`, `x = (// c⏎a)()<T>`,
`x = (// c⏎x.a())[x]<T>`). The operand's pair is redundant, so the instantiation strips it and
prints the run ahead of the operand: the value's doc opens with the comment and a line break,
exactly as it does without the type arguments (`x = (// c⏎a)()`). Every value seam answers it
the same way too — the [left-side shell rule](../../../expressions/assignment/rhs_left_side_shell_comment_prettier_divergence/)
at an assignment, a declarator, a class field, an object value and a parameter default, the
`await` keyword gap, and the restricted productions' hanging pair at `return`, `yield` and
`throw` (`c9`–`c12`), as `return (/* c */⏎a)()` takes it.

tsv, one pass (`input.svelte`):

```ts
x =
	// c1
	a<T>;
```

Prettier takes two passes from the authored form (`audit_signature_inner_shell.txt`): its first
prints the comment flush on the operator's line (`x = // c1⏎a<T>;`) and its second indents the
continuation (`x = // c1⏎\ta<T>;`). The other cells land elsewhere:

- `c5`: the second pass hoists the comment above the key (`// c5⏎k: a.b<T>`).
- `c6`: the block collapses inline (`w = /* c6 */ a<T>;`).
- `c7`: the comment moves past the value (`b = a<T> // c7`).
- `c8`: the continuation stays flush under `await` (`output_prettier.svelte`), the
  [forced-continuation indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
  divergence.
- `c9`–`c11`: prettier's left-side walk (`hasNakedLeftSide`) stops at an instantiation, so it
  hoists every own-line comment, `//` and block alike, onto the keyword's line. ASI then takes
  the argument away (`return; /* c9 */⏎a<T>;`, `return; // c10`, `yield; /* c11 */`).
- `c12`: at `throw` the same output does not parse at all. It rides
  `unformatted_ours_inner_shell.svelte` in the already-normalized form, since prettier's chain
  from the authored one dies.

`unformatted_ours_inner_shell.svelte` carries the authoring. A pair the position requires
keeps the run inside it instead
([instantiation_head_paren_leading_line_comment](../instantiation_head_paren_leading_line_comment_prettier_divergence/)).
The `new` callee's cell (`new ((// c⏎[]())<U>)()` prints `new // c⏎\t([]()<U>)()`) is asserted in
[`tests/new_callee_instantiation_parens.rs`](../../../../../new_callee_instantiation_parens.rs),
since acorn-typescript hoists a parenthesized callee's list onto the `new` in a script body.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Assignment value's left-side shell comment; Return/throw hanging comment, left-side operand;
`await`→operand / `new`→callee line comment, trailing authoring) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
