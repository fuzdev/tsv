# case_test_prettier_ignore_head_prettier_divergence

A `case` label's **`case`→test head**: an own-line directive in that gap freezes the whole test,
the assignment family's rule one family over with the `case` keyword as the delimiter. The
frozen slice is the test's own node span, so the `:` and the case body stay parent-owned, a
sibling case the freeze does not reach still normalizes, and the clarity parens the *position*
supplies around an assignment test (`(jjj = kkk)`) stay outside the slice.

Both formatters honor the directive. They differ on **where the comment sits** and, as the
uncommented-by-a-directive sibling
[case_test_gap_line_comment](../case_test_gap_line_comment_prettier_divergence/) already
records, on the test's **indent**: tsv keeps a comment the author gave its own line on that line
and drops the test one level in, while prettier pulls the comment up to trail `case` and leaves
the test flush at the case's own indent.

```ts
// tsv (own line preserved)          // prettier (pulled onto the `case` line)
switch (aaa) {                        switch (aaa) {
	case                                  case // prettier-ignore
		// prettier-ignore                  bbb  +  ccc:
		bbb  +  ccc:                          fn(ddd);
		fn(ddd);                          }
}
```

## Why tsv differs

A directive **trailing** the keyword is inert under tsv's own placement floor, which reads only
a directive alone on its line. Following prettier's relocation would therefore cost the freeze
on tsv's own second pass: pass 1 would print `case // prettier-ignore`, pass 2 would read no
freeze and normalize the test, and the directive's whole effect would vanish with nothing
dropped and no gate firing. The same argument, at the same seam, as the enum member's `=`→value
head and the ternary branch heads.

The indent half is the already-sanctioned
[§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and is unchanged by the directive.

## Expected behavior

- **tsv**: the directive keeps its own line, the test prints verbatim one level in, and the
  input is a fixed point. Both directive spellings behave alike — placement keys the freeze, not
  the spelling.
- **prettier**: honors the freeze with the comment pulled onto the `case` line and the test
  flush (`output_prettier.svelte`).

A directive the author wrote **on** the `case` line is the mirror case, and the input pins it:
under tsv it is inert (the comment keeps its line, the test normalizes) while prettier honors
it, so `unformatted_ours_inert.svelte` — the same document with the un-normalized spacing
prettier would protect — normalizes to the input under tsv alone, and
`divergent_variant_inert.svelte` holds the stable form prettier reaches from it.

## Reason

◆comment_preservation — tsv preserves the authored line wherever relocating it would cost the
freeze on the next pass. Sanctioned for the placement in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and for the freeze in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
The ternary face of the same cluster is
[branch_prettier_ignore_head](../../../expressions/ternary/branch_prettier_ignore_head_prettier_divergence/).
