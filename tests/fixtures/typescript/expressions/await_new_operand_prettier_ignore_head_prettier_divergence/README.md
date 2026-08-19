# `await`→operand / `new`→callee gap, format-ignore head

The **freeze** claim over the gap whose *indent* claim is
[await_new_operand_line_comment](../await_new_operand_line_comment_prettier_divergence/): an
own-line directive in either keyword→operand gap freezes what follows it, the assignment
family's value-head rule with `await` / `new` as the delimiter.

The slice is the operand's own node span, so a `new` expression's **type arguments and
argument list** — which sit past the callee — stay parent-owned and still normalize
(`Fff<Ggg>(hhh)`), while an `await` whose operand *is* the call freezes the arguments with
it (`fn2(  jjj  )`). Parens the operand **requires** are the printer's, not the author's, so
they ride outside the slice exactly as at every other value head.

Both formatters honor the directive. They differ on **where the comment sits**: tsv keeps a
comment the author gave its own line on that line, while prettier pulls it up to trail the
keyword.

```ts
// tsv (own line preserved)          // prettier (pulled onto the keyword line)
const aaa = new                       const aaa = new // prettier-ignore
	// prettier-ignore                  Bbb.Ccc(ddd);
	Bbb.Ccc(ddd);
```

## Why tsv differs

A directive **trailing** the keyword is inert under tsv's own placement floor, which reads
only a directive alone on its line. Following prettier's relocation would therefore cost the
freeze on tsv's own second pass: pass 1 would print `new // prettier-ignore`, pass 2 would
read no freeze and normalize the operand, and the directive's whole effect would vanish with
nothing dropped and no gate firing. The same argument, at the same seam, as the enum member's
`=`→value head and the ternary branch heads.

The indent half of the divergence is the sibling fixture's already-sanctioned
[§Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and is unchanged by the directive.

## Expected behavior

- **tsv**: the directive keeps its own line, the operand prints verbatim one level in, and
  the input is a fixed point. Both spellings behave alike — placement keys the freeze, not
  the spelling. The last two cases pin the mirror: a directive the author wrote **on** the
  keyword's line is inert here, so the comment keeps its line and the operand normalizes.
- **prettier**: honors the freeze with the comment pulled onto the keyword's line
  (`output_prettier.svelte`), and is not idempotent on the block spelling — its second pass
  collapses the whole expression back onto one line (`audit_signature.txt`).

## Reason

◆comment_preservation — tsv preserves the authored line wherever relocating it would cost the
freeze on the next pass. Sanctioned for the placement in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and for the freeze in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
The ternary and `case` faces of the same rule are
[branch_prettier_ignore_head](../ternary/branch_prettier_ignore_head_prettier_divergence/) and
[case_test_prettier_ignore_head](../../statements/switch/case_test_prettier_ignore_head_prettier_divergence/).
