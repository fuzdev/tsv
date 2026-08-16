# expr_leading_line_prettier_divergence

The **leading** counterpart of
[expr_trailing_line](./../expr_trailing_line_prettier_divergence/): one line comment at every
braced position, but in the head→value gap rather than after the value. A `//` runs to end of
line, so the value cannot stay on the head's line — and tsv drops it to a continuation line
**indented one level**, uniformly at every braced head.

tsv:

```svelte
{@html // c
	expr}
```

Prettier keeps the continuation **flush** (`{@html // c⏎expr}`), which is what it does at
every site of this rule — see
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).
Flush, the value sits at the column of the *sibling* template nodes around it, so `expr}`
reads as text beside the tag rather than as the tag's own value.

The whole braced family is enumerated because the rule is what makes them agree, and before
this they did not: the block heads and the `{@const}` init hung the value, the
block-structure directives (`bind:`, `class:`, `use:`, `on:`) hung it inside their braces, and
the prefixed tags, the `{expr}` tag and plain attribute values left it flush — three answers
to one question. The three shapes that remain differ only in what the *host* does with its
delimiters, never in whether the value hangs:

- **continuation indent, hugged `}`** — the prefixed tags (`{@html}`, `{@render}`,
  `{@debug}`, `{...}`, `{@attach}`), the `{expr}` tag, and attribute values.
- **continuation indent, dangling closer** — the block heads, whose `}` drops to base
  whatever broke the head ([§Svelte: Blocks](../../../../../../docs/conformance_prettier_svelte.md#svelte-blocks);
  [condition_breaking_comment](../../../blocks/if/condition_breaking_comment_prettier_divergence/)
  is that shape's own fixture), and the `{#each}` **key**, a head of its own inside the head:
  its `)` drops the same way and the tag's `}` continues on that line, and the construct goes
  multiline for a broken key exactly as for a broken head expression — the two `{#each}` cases
  here are the same shape asked of the head and of its key
  ([each/key_long](../../../blocks/each/key_long_prettier_divergence/)).
- **block form** — a directive value that block-wraps (`bind:` always; `class:`, `use:` and
  `on:` whenever the expression does not self-expand), where the block's own `indent` is the
  hang, so the comment takes its own line inside the braces and the value the next; and the
  `{@const}` init, whose break-after-operator layout indents the comment and the value
  together under the `=`. A directive whose expression *does* self-expand hugs its braces
  instead and takes the first shape
  ([on/line_comment](../../../directives/on/line_comment_prettier_divergence/)).

`{@debug // c⏎x}` is the one position where prettier does not merely flatten the continuation
but **drops the comment**, the same stripping it applies to every `{@debug}` comment
(`tags/debug/debug_comment_prettier_divergence`). The committed `output_prettier.svelte`
records that loss.

## Reason

User comments are valuable and shouldn't be silently removed or flattened into a position that
reads as a sibling. See
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)
and [§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [expr_trailing_line](./../expr_trailing_line_prettier_divergence/) — the same position sweep with the comment *after* the value
- [expr_trailing_indented_content](./../expr_trailing_indented_content_prettier_divergence/) — where the `}` lands once something indents the head's content, this rule included
- [condition_breaking_comment](../../../blocks/if/condition_breaking_comment_prettier_divergence/) — the block head's own fixture for the comment-forced head break
- [expr_html](./../expr_html/), [expr_render](./../expr_render/), [expr_spread](./../expr_spread/) — the same heads with block comments, which force no break
