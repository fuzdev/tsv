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
prettier's own TypeScript parser, and for tsv's. Prettier's second pass on that output does
not run at all — the Svelte plugin re-parses the script block and throws — and F4b
explicitly tolerates an erroring pass (no `audit_signature.txt` can represent a truncated
chain), so the absent signature is sanctioned rather than an ordinary fixed point. tsv's
parser accepts the pair-less form, which is how the output survived to be printed at all;
the fix is the paren rule, not the parser.

An **undecorated** `class {}` opens a concise body fine and stays bare in both tools, and at
**statement** position the same leftmost rule already parenthesized the class — both are
carried here as controls, unchanged in `output_prettier.svelte`.

A frozen body whose **root** is the class takes the same pair: it belongs to the position,
so it rides outside the slice exactly as every other required pair does. Prettier keeps the
freeze there but loses the pair too, and adds a blank line and an orphan `;`.

A frozen **composite** body — one whose leftmost node is the class rather than the body's
root — carries that pair INSIDE the slice, and nothing re-synthesizes it. acorn's member and
call spans begin at the author's `(`, so the `=>`-gap cells here (`(@dec  class  {}).ppp`,
`(@dec  class  {})()`) freeze the pair along with the class and print back byte for byte; the
plain-object twin (`({zzz:   1}).aaa1`, in
[body_prettier_ignore_head](../body_prettier_ignore_head/)) is the identical shape. The
position-pair seam that also serves the expression statement is what produces that same form
from the **shell** authoring, where the directive sits inside the parens the parser erased:
`() => (⏎// prettier-ignore⏎@dec class {}⏎).ppp` prints `(@dec class {}).ppp`, the run hoisted
ahead of the construct and the pair minted — the object spelling of that authoring is pinned
by the twin fixture's `unformatted_paren_shell.svelte`. The pair-less spelling is not a cell
at all: with no pair the `@` (like a leftmost `{`) never opens a concise body, so tsc,
prettier and tsv all reject it. Prettier prints these two cells unchanged, so they are matches inside a divergent file.

## Expected behavior

- **tsv**: the pair wraps the class alone and breaks open (the decorator owns its line); the
  input is a fixed point and reparses.
- **prettier**: emits the pair-less form (see `output_prettier.svelte`), which no parser
  accepts — its own TypeScript parser included, which is why prettier throws on its own
  output instead of reaching a second pass.

## Reason

◆prettier_bug. See
[conformance_prettier.md §Prettier bug index](../../../../../../docs/conformance_prettier.md#prettier-bug-index)
and
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Arrow-body leftmost decorated class). The frozen cell's rule is
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*).
