# branch_prettier_ignore_head_prettier_divergence

A ternary's **`?`→consequent and `:`→alternate heads**: an own-line directive in either gap
freezes the whole branch, the assignment family's rule one family over with `?` / `:` as the
delimiter. The frozen slice is the branch's own node span, so the operator, the sibling branch
and the enclosing statement stay parent-owned — and the clarity parens the *position* supplies
(`(nnn ?? ooo)`) stay outside the slice, exactly as they do at every other value head.

Both formatters honor the directive. They differ on **where the comment sits**: tsv keeps a
comment the author gave its own line on that line, while prettier pulls it up to trail the
operator. The last case shows the rule is not the directive's — an ordinary `// c` in the same
gap *is* pulled up under tsv too, and the branch normalizes.

```ts
// tsv (own line preserved)          // prettier (pulled onto the `?` line)
const aaa = cond                      const aaa = cond
	?                                     ? // prettier-ignore
		// prettier-ignore                    bbb  +  ccc
		bbb  +  ccc                       : ddd;
	: ddd;
```

## Why tsv differs

A directive **trailing** an operator is inert under tsv's own placement floor, which reads only
a directive alone on its line. Following prettier's relocation would therefore cost the freeze
on tsv's own second pass: pass 1 would print `? // prettier-ignore`, pass 2 would read no freeze
and normalize the branch, and the directive's whole effect would vanish with nothing dropped and
no gate firing. The same argument, at the same seam, as the enum member's `=`→value head.

Because the directive must own its line, an honored one also forces the ternary's **breaking**
layout in both spellings — a block directive that stayed inline would be glued to the `?` and
therefore inert. Prettier collapses that case (`cond ? /* prettier-ignore */ jjj  +  kkk : lll`)
on its own second pass; `audit_signature.txt` pins the chain.

## Expected behavior

- **tsv**: the directive keeps its own line, the branch prints verbatim, and the input is a
  fixed point. The gap between the frozen slice and the `:` (or the statement's `;`) belongs to
  the enclosing scan and answers as the unfrozen branch does — a block inline after the slice, a
  line comment trailing it, an alternate's own trailing comment floating past the `;`.
- **prettier**: honors the freeze with the comment pulled onto the operator's line
  (`output_prettier.svelte`), and is not idempotent on the block spelling
  (`audit_signature.txt`).

A directive the author wrote **on** the operator's line is the mirror case, and the input pins
it: under tsv it is inert (the comment keeps its line, the branch normalizes) while prettier
honors it, so `unformatted_ours_inert.svelte` — the same document with the un-normalized
spacing prettier would protect — normalizes to the input under tsv alone, and
`audit_signature_inert.txt` pins what prettier does with it instead.

## Reason

◆comment_preservation — tsv preserves the authored line wherever relocating it would cost the
freeze on the next pass. Sanctioned for the placement in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and for the freeze in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
The `case`→test face of the same cluster is
[case_test_prettier_ignore_head](../../../statements/switch/case_test_prettier_ignore_head_prettier_divergence/),
and the enum member is
[member_init_prettier_ignore_head](../../../declarations/enum/member_init_prettier_ignore_head_prettier_divergence/).
