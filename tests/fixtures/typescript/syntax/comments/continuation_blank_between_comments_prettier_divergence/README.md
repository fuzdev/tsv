# continuation_blank_between_comments_prettier_divergence

An authored blank line **inside a forced-continuation gap** — between two own-line comments, or
between the last comment and the tail — survives, at every site the uniform forced-continuation
indent covers:

```ts
const e // c1

	// c2
	= 1;
```

The gap's break is **forced** (a `//` runs to end-of-line, so the tail cannot share it), and a
blank line is a property of a break: it survives exactly where the break survives. That is the
same rule the value side of these constructs already follows — the `=`→value control at the end
of this fixture keeps its blank, and so do the module-header gaps, which reach the run through a
different emitter. The two emitters answer one question one way.

## Reason

Prettier **agrees** at four of the sites here — declaration header, `:`→type annotation, prefix
type-operator operand, callee→empty argument list — and differs only in the continuation indent
already cataloged under
[§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
It differs on the **blank** only where it relocates the comment run out of the gap entirely: at
the before-`=` initializer it moves the run past the `=` and flattens it (`const e = // c1⏎// c2⏎1`),
and with a single comment it floats it to end-of-line and drops the blank with the break
(`const f = 1; // c1`). Where the run stays in place prettier keeps the blank too, so the drop is
incidental to its relocation rather than a considered answer.

Blank-line preservation is Tier-1 authoring intent; ◆design_choice. See
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).
