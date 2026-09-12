# body_leftmost_decorated_class_prettier_divergence

An arrow's concise body cannot START with `@`: the grammar reads a decorator only ahead of
a class in a *declaration-ish* position, so `() => @dec class {}.bbb` does not reparse (tsc:
`'{' expected`). A **decorated class expression** at the body's leftmost position therefore
takes a paren pair of its own — around the class alone, the same answer a leftmost object
literal gets for its `{`.

```ts
// tsv                                  // prettier (does not reparse)
const aaa = () =>                       const aaa = () =>
	(                                     	@dec
		@dec                                	class {}.bbb;
		class {}
	).bbb;
```

## Why tsv differs

◆prettier_bug. Prettier drops the pair at every shape the leftmost rule reaches — member,
call, ternary test, and the bare class body — and its output is a syntax error for tsc, for
prettier's own TypeScript parser, and for tsv's. Prettier is nevertheless idempotent on that
output at the `.svelte` level, because prettier-plugin-svelte catches that `SyntaxError` and
passes the script block through verbatim, so the absent `audit_signature.txt` is the
ordinary F4b fixed point. tsv's parser accepts the pair-less form, which is how the output
survived to be printed at all; the fix is the paren rule, not the parser.

An **undecorated** `class {}` opens a concise body fine and stays bare in both tools, and at
**statement** position the same leftmost rule already parenthesized the class — both are
carried here as controls, unchanged in `output_prettier.svelte`.

A frozen body whose **root** is the class takes the same pair: it belongs to the position,
so it rides outside the slice exactly as every other required pair does. Prettier keeps the
freeze there but loses the pair too, and adds a blank line and an orphan `;`.

A frozen **composite** body (`() =>⏎// prettier-ignore⏎(@dec class {}.k)`) is outside this
fixture's claim and still prints pair-less. The slice is emitted from the ARROW-BODY
position, which asks the paren rule of the body's root and so never reaches the leftmost
node's target; the plain-object twin (`{a: 1}.k`) has the identical shape and is the older
half of the same gap.

## Expected behavior

- **tsv**: the pair wraps the class alone and breaks open (the decorator owns its line); the
  input is a fixed point and reparses.
- **prettier**: emits the pair-less form (see `output_prettier.svelte`), which no parser
  accepts — its own TypeScript parser included, though the Svelte plugin swallows that and
  re-emits the block verbatim.

## Reason

◆prettier_bug. See
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index)
and
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Arrow-body leftmost decorated class). The frozen cell's rule is
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*).
