# test_call_expanded_params_long_prettier_divergence

A test call keeps its callback's parameter list flat at any width — but that
licence belongs to the flat one-line **layout**, not to the callee's name. A
comment in an argument gap gives that layout up
([test_call_arg_comment](../test_call_arg_comment_prettier_divergence/)), and the
expanded call is then an ordinary call in every respect: its callback's
parameters — value and type alike — break at the ordinary 100/101 boundary.
Prettier keeps the flat layout, and holds both lists on one line at any width.

tsv: the expanded call's parameter list breaks at 101, holding 100
Prettier: the flat layout is kept and the list stays on one line at any width

```
// tsv                       // prettier
it(                          it(// c
	// c                       'text', (<a>: A, <b>: B) => {
	'text',                      a();
	(                          });
		<a>: A,
		<b>: B
	) => {
		a();
	}
);
```

## Reason

**The carve-out's argument stops here.** §Print Width Philosophy licenses flat
parameters on two grounds: they are part of a head the reader takes in as one
unit, and the line is *already* licensed to overrun, so breaking them buys a
worse shape without restoring the limit. In the expanded form both grounds
lapse — the name and the callback already sit on separate lines, and breaking the
list now *does* restore the limit (each 101 case here holds 100). tsv's default
governs from there: a line tsv can break is a line tsv does break.

**Prettier is not evidence for the expanded form, because it is never in it.**
`printCallExpression` takes its test-call branch unconditionally whenever
`isTestCall` holds, so for the 2- and 3-argument shapes tsv implements,
`isTestCall(parent)` in `print/function-parameters.js` is true exactly when the
call already printed flat. (The `parent` argument that could separate the two
matters only for the one-argument Angular wrapper, which tsv does not implement.)
The flat lists in `output_prettier.svelte` are its flat layout showing through,
not a judgment about an expanded call.

This is the same shape as every other way out of the test-call rule — an optional
callee, a non-string first argument, a non-callback second, the 3-argument
narrowing — each of which restores ordinary width-driven breaking.

The control case pins the other side: with the gap comment removed, the same
101-wide list keeps the flat layout and stays on one line, where both formatters
agree.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Print Width Philosophy.
