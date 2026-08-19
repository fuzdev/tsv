# template_arg_prettier_ignore_expands_prettier_divergence

A sole multiline template argument that starts on the `(` line normally **hugs**
(`calls/template_arg_multiline_hugged`). An honored directive in the `(`→argument gap
declines that hug and the call expands instead:

```ts
fn(
	// prettier-ignore
	/* x */ `${a  +  b}
y`
);
```

The hug is a flat concat with no line of its own to put the run on, so it would land the
directive on the `(`'s line — where a directive is **inert** — and the freeze this very gap
grants would be gone on the second pass. Prettier hugs (`fn(// prettier-ignore⏎…`) and stays
frozen anyway: it decides a directive by comment *attachment*, so the line the author gave it
carries no weight, where tsv decides by placement and therefore never relocates one. The same
one-sided invariant as the
[keyword gap](../../../syntax/comments/keyword_gap_prettier_ignore_own_line_prettier_divergence/),
and the same "a lone huggable item expands rather than hugging" rule the parameter list
already states — there prettier agrees, because it has no hug at that position to disagree
with.

All four call spellings answer it identically (plain call, `new`, member chain, dynamic
`import()`), as does the block spelling of the directive. The last case is the control: with
nothing glued to the backtick, the newline before it declines the hug on its own, so both
tools expand and the freeze is never at risk.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
