# yield_operand_paren_prettier_divergence

An authored pair around a decorated class at the leftmost position of a plain `yield`
operand is kept, broken open like every decorated class in a pair:

```ts
// tsv                                  // prettier
const c = yield (                       const c = yield @dec
	@dec                                  class {}.x;
	class {}
).x;
```

## Why tsv differs

◆prettier_bug ◆parser_compat. Both spellings are the same program to tsc, and the bare one
is grammatical ([yield_operand](../yield_operand_svelte_divergence/)), but acorn rejects a
`@` right after `yield`, so prettier's output does not parse under Svelte: a component whose
`<script>` held `yield (@dec class {})` no longer compiles once prettier strips the pair.
tsv keeps the pair the author wrote and adds none of its own. A bare `yield @dec class {}`
stays bare in both tools, as written. The pair is the leftmost class's own, so it wraps the
class alone under a member, a call or an operator, as at the arrow-body leftmost position
([body_leftmost_decorated_class](../../../expressions/arrow/body_leftmost_decorated_class_prettier_divergence/)),
and a pair the author wrote around the whole operand moves onto the class.

Where the operand prints a pair of its own ahead of the `@`, that pair is the class's and no
second is added: an assignment's (`yield (@dec⏎class {}.x = 1)`) and a sequence's
(`yield (@dec⏎class {},⏎1)`), which prettier prints alike. So is the hanging pair a `//`
comment after the `(` takes ([yield_operand_paren_line_comment](../yield_operand_paren_line_comment_prettier_divergence/)).
A comment inside the author's shell around the class alone, behind the class
(`yield (@dec class {} // c⏎)!.x`), is printed by what follows that shell, so there the class
keeps no pair and prints bare, as prettier prints it: a known gap.

`yield*` is the control: acorn reads the `@` after the `*`, and both tools print the class
bare.

## Expected behavior

- **tsv**: the authored pair stays; the input is a fixed point and parses under acorn.
- **prettier**: strips it (see `output_prettier.svelte`), which acorn and Svelte reject.
- **`unformatted_ours_compact`**: the inline authoring; tsv breaks each pair open, prettier
  strips it. Its assignment and sequence lines carry the pair around the class, which the
  operand's own pair absorbs.
- **`unformatted_ours_outer_pair`**: the author's pair around the WHOLE operand
  (`yield (@dec class {}.x)`, `yield (@dec class {} % 2)`, `yield (@a @b class {}())`); tsv
  moves it onto the class, prettier strips it.

## Reason

◆prettier_bug ◆parser_compat. See
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index)
and
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(`yield` operand decorated class pair).
