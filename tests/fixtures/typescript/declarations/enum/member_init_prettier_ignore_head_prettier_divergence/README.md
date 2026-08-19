# member_init_prettier_ignore_head_prettier_divergence

An enum member's **`=`→value head**: an own-line directive in that gap freezes the whole
value, the same rule every other host of the assignment family already carries (a declarator
initializer, an assignment RHS, an object property value, a class field value, a default
value). The frozen slice is the value's own node span, so the member name, the `=` and the
enclosing member list stay parent-owned and a sibling member the freeze does not reach still
normalizes.

Both formatters honor the directive. They differ on **where the comment sits**, and the enum
member is the one host of the family where they do: tsv keeps a comment the author gave its
own line on that line, while prettier pulls it up to trail the `=`. The last case shows the
rule is not the directive's — an ordinary `// c` in the same gap is placed the same way by
each formatter.

```ts
// tsv (own line preserved)          // prettier (pulled onto the `=` line)
enum Aaa {                            enum Aaa {
	Bbb =                                 Bbb = // prettier-ignore
		// prettier-ignore                  ccc  +  ddd
		ccc  +  ddd                       }
}
```

## Why tsv differs

A directive **trailing** an operator is inert under tsv's own floor, which only reads a
directive alone on its line. Following prettier's relocation would therefore cost the freeze
on tsv's own second pass: pass 1 would print `Bbb = // prettier-ignore`, pass 2 would read no
freeze and normalize the value, and the directive's whole effect would vanish with nothing
dropped and no gate firing.

**Prettier demonstrates exactly that loss.** It is not idempotent on its own output here —
`audit_signature.txt` pins the chain, and its pass 2 floats the directive past the value
(`Bbb = ccc + ddd // prettier-ignore`), where the freeze no longer applies and the spacing
the author protected is gone. Same second-pass loss, same cause, one formatter over; it is
the reason tsv's enum and namespace **body** heads keep their own line too.

The placement half is the sibling of the before-`=` continuation rule and of the declaration
heads: wherever an ordinary own-line comment would be relocated onto a head's line, an
honored directive keeps its line instead.

## Expected behavior

- **tsv**: the directive keeps its own line and the value prints verbatim; the sibling member
  the freeze does not reach normalizes; the input is a fixed point.
- **prettier**: honors the freeze on the first pass with the comment pulled onto the `=` line
  (`output_prettier.svelte`), then loses it on the second (`audit_signature.txt`).

## Reason

◆comment_preservation — tsv preserves the authored line wherever relocating it would cost the
freeze on the next pass. Sanctioned for the placement in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and for the freeze in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
The before-`=` face of the same gap is
[before_eq_comment_value_head_freeze](../../variable/before_eq_comment_value_head_freeze_prettier_divergence/).
