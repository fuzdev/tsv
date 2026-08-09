# body_prettier_ignore_head_prettier_divergence

An own-line directive in the header→`{` gap of an `enum`, where the following node is
the enum **body**. tsv freezes that body over its own node span — the braces and every
member inside them — while the head (`const`, the name) stays parent-owned and
normalizes:

```ts
const enum Bbb
// prettier-ignore
{
	ooo   =   2
}
```

Prettier **relocates** the directive onto the header line (`enum Aaa // prettier-ignore`)
and pulls the `{` up on the pass after. That placement is inert, so prettier's own
**second** pass normalizes the body and the freeze is lost entirely — the authored
directive ends up affecting nothing. tsv keeps the directive on the line the author
wrote it on, which is what keeps it honored across tsv's own second pass.

**Both spellings** behave alike — placement keys the freeze, not the comment's
spelling.

The plain-comment form of the same gap is
[header_brace_line_comment](../header_brace_line_comment_prettier_divergence/), where an
ordinary line comment *is* pulled up to trail the header; a directive is the exception
that keeps its own line. The `class` face of this rule is
[class/body_prettier_ignore_head](../../../class/body_prettier_ignore_head_prettier_divergence/),
and the `interface` and `namespace` faces sit beside it
([interface](../../interface/body_prettier_ignore_head_prettier_divergence/),
[namespace](../../namespace/body_prettier_ignore_head_prettier_divergence/)); all four
resolve the gap through `Printer::gap_frozen_span` and place the run through
`Printer::build_header_pre_body_doc`.

## Reason

tsv never relocates a directive: the placement the author wrote is the placement that
decides the freeze, so the frozen node is the one that actually follows the directive;
◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
