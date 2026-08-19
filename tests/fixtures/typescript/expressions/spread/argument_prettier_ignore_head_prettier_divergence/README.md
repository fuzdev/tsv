# Spread `...`→argument gap, format-ignore head

An own-line directive in a spread's `...`→argument gap freezes the whole argument — the
value-head rule with `...` as the delimiter, in all three spread positions (call argument,
array element, object property).

The slice is the argument's own node span, so parens the argument **requires** stay outside
it: they are the printer's, not the author's, exactly as at every other value head. A
**sequence** argument is the exception that proves it — its pair is printed by its own node
rather than by this context, so the frozen slice gets that pair back or the grouping (and with
it the argument count) would be lost. The
slice→`)` gap is answered by the shell's own trailing emitter, so a comment the author wrote
there rides inside the pair (`(mmm  ??  nnn /* c */)`).

Both formatters honor the directive. They differ on **where the comment sits**: tsv keeps a
comment the author gave its own line on that line and hangs the argument below it, while
prettier pulls it up to trail the `...`.

```ts
// tsv (own line preserved)        // prettier (pulled onto the `...` line)
fn1(                               fn1(
  ...                                ...// prettier-ignore
    // prettier-ignore                 (aaa  ??  bbb)
    (aaa  ??  bbb)                 );
);
```

## Why tsv differs

A directive **trailing** the `...` is inert under tsv's own placement floor, which reads only
a directive alone on its line. Following prettier's relocation would therefore cost the
freeze on tsv's own second pass: pass 1 would print `...// prettier-ignore`, pass 2 would
read no freeze and normalize the argument, and the directive's whole effect would vanish with
nothing dropped and no gate firing. The same argument, at the same seam, as the enum member's
`=`→value head, the ternary branch heads, and the `await`→operand / `new`→callee gaps — whose
uniform forced-continuation indent this arm also takes.

An **ordinary** comment in this gap is not relocated by either formatter: it stays after the
`...`, outside any parens the argument needs, in both
([grouped_operand_leading_comment](../grouped_operand_leading_comment/)). Only the directive's
own-line requirement parts the two here.

## Expected behavior

- **tsv**: the directive keeps its own line, the argument prints verbatim one level in, and
  the input is a fixed point. Both spellings behave alike — placement keys the freeze, not
  the spelling. The last case pins the mirror: a directive the author **glued** to the `...`
  is inert, so the comment keeps the line it was written on and the argument normalizes;
  `unformatted_ours_inert.svelte` carries that case un-normalized, which only tsv rewrites.
- **prettier**: honors the freeze with the comment pulled onto the `...` line
  (`output_prettier.svelte`), honors the glued placement too, and is not idempotent on either
  — pinned by `audit_signature.txt` and `audit_signature_inert.txt`.

## Reason

◆comment_preservation — tsv preserves the authored line wherever relocating it would cost
the freeze on the next pass. Sanctioned for the freeze in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*) and for the placement in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation);
the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
The sibling heads of this cluster are
[unary operand](../../unary/operand_prettier_ignore_head/),
[template interpolation](../../literals/template/interpolation_prettier_ignore_head/) and
[computed key](../../objects/computed_key_prettier_ignore_head_prettier_divergence/).
