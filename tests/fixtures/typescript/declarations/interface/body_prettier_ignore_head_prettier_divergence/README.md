# body_prettier_ignore_head_prettier_divergence

An own-line directive in the header→`{` gap of an `interface`, where the following node
is the interface **body**. tsv freezes that body over its own node span — the braces and
every member inside them — while the head (name, `extends` clause) stays parent-owned
and normalizes:

```ts
interface Bbb extends Aaa
// prettier-ignore
{
	nnn   :   2;
}
```

Prettier **relocates** the directive inside the body, pulling the `{` up onto the head
line, and freezes only the **first member** there. So the same authoring freezes the
whole body for tsv and one member for prettier, and prettier's form no longer holds the
directive at the position the author wrote it.

**Both spellings** behave alike — placement keys the freeze, not the comment's spelling.

This is the `interface` face of the class rule
([class/body_prettier_ignore_head](../../../class/body_prettier_ignore_head_prettier_divergence/)),
which prettier answers the same way for both; the `enum` and `namespace` faces, where
prettier instead relocates onto the header line and drops the freeze on its second pass,
sit beside it ([enum](../../enum/body_prettier_ignore_head_prettier_divergence/),
[namespace](../../namespace/body_prettier_ignore_head_prettier_divergence/)). All four
resolve the gap through `Printer::gap_frozen_span` and place the run through
`Printer::build_header_pre_body_doc`.

## Reason

tsv never relocates a directive: the placement the author wrote is the placement that
decides the freeze, so the frozen node is the one that actually follows the directive;
◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
