# expand_first_binary_tail_long_prettier_divergence

The expand-first hug keeps a short tail argument inline past the callback's closing brace
(`fn(() => {…}, a && b)`), so that closing line is what print width measures. At 101 the
tail's operator no longer fits it: tsv breaks the operator and indents the continuation,
while prettier holds the tail flat however far the line runs.

```
// tsv                          // prettier
fn(() => {                      fn(() => {
	a();                            a();
}, aaa &&                       }, aaa && bbb);   // 101
	bbb);
```

## Reason

Print width is a hard limit in tsv, and the tail's operator is a break that restores it —
so tsv takes it. See
[conformance_prettier.md §Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy).
Both forms are idempotent, so nothing but a width measurement separates them: prettier
keeps its flat form stable and normalizes tsv's broken one back to it, which makes the
divergence one of normalization.

The 100-char group is the boundary control — there the tail fits and both formatters hold
it flat.

A tail carrying its own **forced** break is outside the divergence: the hug is refused and
every argument breaks out, where both formatters print the same continuation indent — the
same argument context the width case above uses. That half is pinned by
[expand_first_tail_arg_breaks](../expand_first_tail_arg_breaks/).

Covers the plain-call, `new`, and chained-call argument paths. See
[conformance_prettier_ts.md](../../../../../../docs/conformance_prettier_ts.md) §TypeScript.
