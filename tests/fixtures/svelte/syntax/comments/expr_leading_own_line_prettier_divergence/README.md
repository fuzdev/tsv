# expr_leading_own_line_prettier_divergence

The **own-line** authoring of the sweep
[expr_leading_line](./../expr_leading_line_prettier_divergence/) covers trailing: one line
comment in the head→value gap at every braced position, but written on its own line rather
than after the head. There the comment stays put in both formatters and only the
continuation's indent diverges; here prettier **relocates** — it pulls the comment up onto
the head's line — while tsv keeps the line the author gave it.

Under tsv the two authorings differ in **that line and nothing else**: the content indents one
level and the closer drops to the head's own column either way, and an unprefixed `{` takes the
space before a trailing comment (`{ // c`) that every prefixed literal already carries. So this
fixture and its sibling are one geometry with the comment in two places.

tsv:

```svelte
{@html
	// c
	expr
}
```

Prettier: `{@html // c⏎expr}`.

## Reason

A comment in this gap **leads the head's value**, and own-line-ness is authoring signal for a
leading position — the corollary in
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
So the head breaks before the run and the run and value hang together one level in, the
`head`→value gap answering the way every keyword→value gap does across the two languages
(`keyof`, a switch label's `case`→test, `new`, `await`). The indent itself is the same
[§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
the trailing sibling documents; only where the run sits differs.

**The shape is not new — it is the one an honored `prettier-ignore` in this gap already
took.** A directive flush against the prefix is inert under tsv's placement floor, so the
freeze had to break the head; that break turns out to be the ordinary own-line form, and the
freeze a special case of it. One predicate now answers both
(`Printer::head_layout`), which is why the two cannot drift apart.

The whole braced family is enumerated for the same reason the trailing sibling enumerates it:
the rule is what makes them agree. Two host shapes:

- **the head breaks before the run** — the prefixed tags (`{@html}`, `{@render}`, `{@debug}`,
  `{...}`, `{@attach}`), the `{expr}` tag, plain attribute values, the block heads, and the
  `{#each}` **key**. That break is the only line the trailing sibling does not share.
- **already in this shape** — a directive value that block-wraps (`bind:` always; `class:`,
  `use:`, `on:` where the expression does not self-expand) reaches it through the block's own
  `indent`, and the `{@const}` init through its break-after-operator layout. Both were
  already here before this rule, and are carried as controls rather than as claims.

`{@debug}` is again the one position where prettier does not merely relocate but **drops** the
comment, the stripping it applies to every `{@debug}` comment
(`tags/debug/debug_comment_prettier_divergence`). The committed `output_prettier.svelte`
records that loss.

A leading **block** comment is deliberately outside all of this, multi-line or not: it forces
no break, so there is no continuation to open and it reflows inline — see
[expr_html](./../expr_html/), [expr_render](./../expr_render/),
[expr_spread](./../expr_spread/).

## Related

- [expr_leading_line](./../expr_leading_line_prettier_divergence/) — the same sweep with the comment *trailing* the head, which stays put in both formatters
- [expr_trailing_indented_content](./../expr_trailing_indented_content_prettier_divergence/) — where the `}` lands once something indents the head's content, this rule included
- [condition_breaking_comment](../../../blocks/if/condition_breaking_comment_prettier_divergence/) — the block head's own fixture for the comment-forced head break
- [case_test_gap_own_line_line_comment](../../../../typescript/statements/switch/case_test_gap_own_line_line_comment_prettier_divergence/) — the TypeScript face of the same corollary
