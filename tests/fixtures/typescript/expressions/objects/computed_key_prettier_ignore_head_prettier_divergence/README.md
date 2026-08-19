# Computed-key `[`→key gap, format-ignore head

An own-line directive in a computed **key**'s `[`→key gap freezes the whole key — the
value-head rule with the `[` as the delimiter, at every host that prints a computed key:
an object property, a class field, a class method, a destructuring pattern, and a type
literal's member — all five route through the one bracket emitter, so they cannot drift.

The **shell** each host supplies is the host's own, though, and the emitter is told which one:
the four value spellings give a computed key its clarity parens (`[(ppp = qqq)]`), a type
member gives it none (`[eee1 = 0]`, prettier agreeing — pinned by
[computed_key_grammar](../../../types/computed_key_grammar/)). The frozen slice reproduces
whatever its own position's ordinary builder would emit, so the frozen and unfrozen forms cannot
disagree about the parens.

The slice is the key's own node span, so the **brackets stay parent-owned** (a slice that
swallowed the `]` would emit a property that no longer parses), and so do parens the key
**requires** — those are the printer's, not the author's, and ride outside the slice
exactly as at every other value head (`[(ppp  =  qqq)]`).

Both formatters honor the directive. They differ on **where the comment sits**: tsv keeps a
comment the author gave its own line on that line, while prettier pulls it up to trail the
`[`.

```ts
// tsv (own line preserved)          // prettier (pulled onto the `[` line)
const aaa = {                        const aaa = {
  [                                    [// prettier-ignore
    // prettier-ignore                 bbb  +  ccc]: ddd
    bbb  +  ccc                      };
  ]: ddd
};
```

## Why tsv differs

A directive **trailing** the `[` is inert under tsv's own placement floor, which reads only
a directive alone on its line. Following prettier's relocation would therefore cost the
freeze on tsv's own second pass: pass 1 would print `[// prettier-ignore`, pass 2 would read
no freeze and normalize the key, and the directive's whole effect would vanish with nothing
dropped and no gate firing. The same argument, at the same seam, as the enum member's
`=`→value head, the ternary branch heads, and the `await`/`new` keyword gaps.

The `[`-line placement half is prettier's already-cataloged relocation at this delimiter
(§Comment relocation, "Object/array/block open-delimiter trailing") and is unchanged by the
directive.

## Expected behavior

- **tsv**: the directive keeps its own line, the key prints verbatim one level in, and the
  input is a fixed point. Both spellings behave alike — placement keys the freeze, not the
  spelling, which is why the block spelling reaches the breaking bracket layout too (see
  [computed_key_own_line_block_comment](../computed_key_own_line_block_comment_prettier_divergence/),
  the ordinary-comment face of that same layout rule).
- **prettier**: honors the freeze with the comment pulled onto the `[` line
  (`output_prettier.svelte`), and is not idempotent on its own output — pinned by
  `audit_signature.txt`.

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
[spread argument](../../spread/argument_prettier_ignore_head_prettier_divergence/).
