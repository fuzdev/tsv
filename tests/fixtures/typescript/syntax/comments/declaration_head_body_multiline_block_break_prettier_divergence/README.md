# Divergence: declaration head→body `{` multiline block, authored break kept

A **multiline** block comment in a declaration's head→body-`{` gap that the author
**broke after** (`function a() /* x⏎y */⏎{`). The break after a multiline block is
authoring signal — the rule every other head and pre-separator gap already applies
(`const a /* x⏎y */⏎= 1`, `type A /* x⏎y */⏎= number`, `class D /* x⏎y */⏎extends E`) —
so tsv keeps it here too: the comment trails the head and the body `{` drops to its own
line, **flush with the head**.

Flush, not continuation-indented: the tail here is the body's own `{`, which owns the
indent level beneath it and must stay aligned with its closing `}`. The forced-continuation
indent belongs to a separator + value that has no block structure of its own. It is the
same landing the class/interface line-comment sibling already takes (`class A // c⏎{`).

```ts
// tsv (preserve the break)              // prettier (three landings, see below)
function a() /* x                        function a /* x
y */                                   y */() {
{                                          fn();
	fn();                                  }
}
```

Prettier answers the constructs here three different ways, none of them preservation
(`output_prettier.svelte`):

- **relocated before the parameter list** for a function declaration and a class method
  (`function a /* x⏎y */() {`) — the same destination it hoists an in-paren comment to
  ([open_paren_block_comment](../../../declarations/function/open_paren_block_comment_prettier_divergence/)),
  and it takes two passes to get there
- **absorbed into the body** for a class and an interface, a move across the `{` the
  comment was written outside
- **the break collapsed** for an `enum` and a `namespace`, where prettier's first pass
  keeps it and its second glues it back

Prettier gluing an `enum`/`namespace` break is not a reason to glue: tsv already keeps the
break where prettier glues at six other gaps — a class property `=`, an enum member `=`, a
property-signature key→`:`, and all three binding defaults
([default_equals_multiline_block_break](../../../expressions/destructuring/default_equals_multiline_block_break_prettier_divergence/),
where prettier "draws no distinction between the two authorings at all").

The constructs prettier **agrees** with tsv on — a function expression and an object method
— live in the non-divergence sibling
[declaration_head_body_multiline_block_break](../declaration_head_body_multiline_block_break/),
which also carries the two controls: a multiline block whose `{` shares its closing line
stays glued (the not-broke-after form), and a **single-line** block's breaks stay unforced
and collapse from any authored position. Only the authored break distinguishes the two,
exactly as at the value and pre-separator gaps. The own-line single-line authoring is pinned
by its three siblings — prettier
[relocates](../declaration_head_body_own_line_block_relocated_prettier_divergence/),
[keeps the break](../declaration_head_body_own_line_block_break_kept_prettier_divergence/),
or [takes two passes](../declaration_head_body_own_line_block_two_pass_prettier_divergence/)
— and the inline form both formatters hold stable is
[declaration_head_body_comment](../declaration_head_body_comment/).

See [conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)
for the rule and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
for the catalog entry.
