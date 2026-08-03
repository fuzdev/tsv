# expr_trailing_indented_content_prettier_divergence

The **indented-content** counterpart of
[expr_trailing_line](./../expr_trailing_line_prettier_divergence/), which pins a trailing
line comment at every braced position but always over content sitting on the head's own
level. Prettier drops trailing comments in template expressions, so tsv alone has to answer
where the closing `}` goes — and once something *indents* the head's content, "reuse the
comment's own break" and "land at the tag's column" stop being the same instruction.

A `//` runs to end of line, so the `}` takes the break the comment already emitted rather
than adding a second one (which would render as a blank line above it). The renderer writes
a line's indentation from the line command that produced it, so that break has to be emitted
one level out whenever the content it ends sits inside an `indent(…)` — otherwise the `}`
lands at the *content's* column instead of the tag's.

tsv:

```svelte
{#if // c1
	cond // c2
}
	text
{/if}
```

Prettier: `{#if // c1⏎cond}` (the trailing comment stripped, the continuation flush).

Two things indent that content, and the control pins that they are the only difference:

- **a leading line comment**, which breaks the head and hangs its continuation one level in
  ([condition_breaking_comment](../../../blocks/if/condition_breaking_comment_prettier_divergence/)
  is that shape's own fixture) — covered on `{#if}` at column zero and on `{#key}` one level
  in, since the two share a head builder.
- **the break-after-operator layout** of a `{@const}` init, which indents the value under
  the `=` while the tag's `}` stays outside it.

The **control** is the same `{#if}` with the leading comment removed: nothing indents the
content there, the comment's own break is already on the right column, and the `}` lands in
the same place — so the fixture asserts one closer column reached two ways rather than two
layouts.

Prettier's `{@const}` output is additionally **corrupt** — it emits an unmatched paren and
relocates the comment past the tag (`{@const a = item && cond)} // c`), then throws on its
own output. The committed `output_prettier.svelte` records those bytes verbatim; there is no
`audit_signature.txt` because prettier cannot take a second pass over them. Same bug as the
`{@const}` case in [expr_trailing_line](./../expr_trailing_line_prettier_divergence/).

## Reason

User comments are valuable and shouldn't be silently removed, and a preserved comment must
not move the delimiter around it. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and the trailing-comment entry in
[§Svelte: Attributes](../../../../../../docs/conformance_prettier.md#svelte-attributes).

## Related

- [expr_trailing_line](./../expr_trailing_line_prettier_divergence/) — one trailing line comment, every position, over unindented content
- [expr_trailing_run](./../expr_trailing_run_prettier_divergence/) — the run's last comment decides the closer
- [braced_value_trailing_line](../../prettier_ignore/braced_value_trailing_line_prettier_divergence/) — the same closer rule on a frozen value, the other thing that indents head content
- [condition_breaking_comment](../../../blocks/if/condition_breaking_comment_prettier_divergence/) — the comment-forced head break itself, without a trailing comment
