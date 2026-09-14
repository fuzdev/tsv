# debug_comma_line_comment_prettier_divergence

A **line** comment in a `{@debug}` tag's comma gap. Prettier strips every `{@debug}`
comment; tsv preserves it and gives the gap the `bind:` sequence's comma-gap answer: a
comment on the comma's line trails it, and from the first comment the author put on its own
line the run leads the next identifier on its own line. A `//` breaks the tag, so the
identifiers below it hang one level in and the `}` drops to the tag's column — the geometry
every braced head a `//` breaks already has.

tsv:

```svelte
{@debug a, // c1
	b
}
{@debug a,
	// c2
	b
}
```

Prettier: `{@debug a, b}` (comments stripped).

## Reason

Content preservation, as in [debug_comment](../debug_comment_prettier_divergence/) and
[debug_comma_comment](../debug_comma_comment_prettier_divergence/) (the block-comment
cases); the placement is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)'s
— own-line-ness is authoring signal for a leading position, so an own-line comment is not
pulled up onto the comma's line — and the indent is
[§Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent)'s.
An author blank below the own-line comment survives, as at every braced head
([expr_leading_blank](../../../syntax/comments/expr_leading_blank_prettier_divergence/)).

A `//` the author wrote **before** the comma (`a // c1⏎, b`) trails the comma too — the comma
is structure, and the comment trails the identifier either way, so this is the pure-separator
carve-out §Comment Position Philosophy names; left where it was written the comment ended its
line ahead of the comma, which then opened the next line alone. The same rule at the `bind:`
sequence matches prettier ([function_comment](../../../directives/bind/function_comment/),
its `unformatted_comma_below_line_comment`). `unformatted_ours_comma_below_line_comment`
spells c1 and the c4/c5 run that way, each reaching the form above.

See [conformance_prettier_svelte.md §Svelte: Elements](../../../../../../docs/conformance_prettier_svelte.md#svelte-elements)
(the `@debug comments` catalog entry).

## Related

- [debug_comma_comment](../debug_comma_comment_prettier_divergence/) — block comments on either side of the comma
- [function_comment](../../../directives/bind/function_comment/) — the `bind:` sequence's comma gap, the same partition where prettier keeps the comments
