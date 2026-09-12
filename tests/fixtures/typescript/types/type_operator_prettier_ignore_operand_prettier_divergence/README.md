# Prefix type operator / `typeof` / `infer` keyword→operand gap, format-ignore head

The **freeze** claim over the gap whose *indent* claim is
[type_operator_keyword_line_comment](../type_operator_keyword_line_comment_prettier_divergence/):
an own-line directive between a prefix type operator (`keyof` / `readonly` / `unique`), the
`typeof` type query or `infer` and its operand freezes what follows it — the single-child type
head rule with the operator keyword as the delimiter.

The slice is the operand's own node span, so a `typeof` query's **name→`<` gap comment and type
arguments** — both past the entity name — stay parent-owned and still normalize
(`k/* l */ <{ m: 3 }>`), exactly as a `new` expression's arguments do past its frozen callee.
`infer`'s child is the type PARAMETER, so a constraint rides *inside* the freeze
(`V extends {  w: 8  }`). What is past the slice hangs *with* it, at the gap's own indent, so a
type-argument list that breaks lands exactly where the same list does with an ordinary comment in
that gap (`type AA`). `unformatted_ours_past_the_slice` re-spells only what sits outside a
freeze — those type arguments and the union's later members — and pins that it all normalizes. Parens the operand **requires** are the
printer's, not the author's, so they ride outside the slice; a redundant paren drops under the
freeze, which is what `unformatted_ours_paren_shell` pins.

A union or intersection operand **declines** the whole-operand freeze: it keeps its required
parens and claims the directive through its own leading run, so Rule A applies inside and the
first member freezes (`type M`) — the composite-transparency the single-child heads all share.

Both formatters honor the directive. They differ on **where the comment sits**: tsv keeps a
comment the author gave its own line on that line, while prettier pulls it up to trail the
keyword.

```ts
// tsv (own line preserved)          // prettier (pulled onto the keyword line)
type A = keyof                        type A = keyof // prettier-ignore
	// prettier-ignore                  {  a: 1  };
	{  a: 1  };
```

## Why tsv differs

A directive **trailing** the keyword is inert under tsv's own placement floor, which reads only
a directive alone on its line. Following prettier's relocation would therefore cost the freeze
on tsv's own second pass: pass 1 would print `keyof // prettier-ignore`, pass 2 would read no
freeze and normalize the operand, and the directive's whole effect would vanish with nothing
dropped and no gate firing. The same argument, at the same seam, as the annotation `:`, the
function-type `=>` return, and `await` / `new`.

The indent half of the divergence is the sibling fixture's already-sanctioned
[§Uniform Forced-Continuation Indent](../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and is unchanged by the directive.

## Expected behavior

- **tsv**: the directive keeps its own line, the operand prints verbatim one level in, and the
  input is a fixed point. Both spellings behave alike — placement keys the freeze, not the
  spelling. The last case pins the mirror: a directive the author wrote **on** the keyword's
  line is inert here, so the comment keeps its line and the operand normalizes.
- **prettier**: honors the freeze with the comment pulled onto the keyword's line
  (`output_prettier.svelte`), and is not idempotent on the block spelling — its second pass
  collapses that alias back onto one line (`audit_signature.txt`).

## Reason

◆comment_preservation — tsv preserves the authored line wherever relocating it would cost the
freeze on the next pass. Sanctioned for the placement in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and for the freeze in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On single-child type positions*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
The expression face of the same rule is
[await_new_operand_prettier_ignore_head](../../expressions/await_new_operand_prettier_ignore_head_prettier_divergence/).
