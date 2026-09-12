# body_prettier_ignore_object_leaf_paren_prettier_divergence

A frozen arrow body whose slice **opens with `{`** takes a paren pair around the whole body:

```ts
const aaa = () =>
	// prettier-ignore
	({bbb:   1} | ccc);
```

Without the pair the `{` is the start of a **block body**, so the output is not the program
the author wrote — it does not parse at all. The pair is needed only under the freeze: the
ordinary path has a finer one available and takes it, parenthesizing the leftmost object
inside the expression (`() => ({ bbb: 1 }) | ccc`), and a verbatim slice has no inside to
reach into. A body that IS an object needs no such rule — the node itself takes the pair at
every arrow body (`body_prettier_ignore_head`'s object cell).

## Why tsv differs

Prettier emits the body bare (`output_prettier.svelte`) and the result is **unparseable** —
prettier's own next pass cannot read it back, so it has no fixed point here either. There is
no parity to keep: a formatter's output has to parse.

## Reason

◆prettier_bug — output validity outranks prettier parity, the same call the `for`-init
declaration freeze makes against the same kind of prettier slip. Sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On delimiter-owned value heads, and on sequence operands*).
