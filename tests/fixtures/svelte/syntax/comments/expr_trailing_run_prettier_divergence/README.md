# expr_trailing_run_prettier_divergence

The **run** counterpart of [expr_trailing_line](./../expr_trailing_line_prettier_divergence/)
and [expr_trailing](./../expr_trailing_prettier_divergence/), which each pin a *single*
trailing comment. Prettier drops trailing comments in template expressions; tsv preserves
them, so tsv alone has to answer what a run of two does — and a run of two is not the
one-comment rule applied twice.

A `//` comment ends the line, so whatever follows it starts a new one. A trailing block
comment's leading space is the separator from the content it *trails*; after a line
comment's break there is no content on the line to trail, so the space would be leading
whitespace — indentation tsv emits nowhere else.

tsv:

```svelte
{@html expr // c
/* d */}
```

Prettier: `{@html expr}` (both comments stripped).

The run's **last** comment decides the closer, at every position that chooses one. A run
ending in a line comment already supplies the break the closing `}` needs, so the `}` reuses
it (the two siblings' whole subject). A run ending in a block comment supplies none, so a
directive value takes the ordinary block form instead of hugging — the hug exists only to
avoid doubling a break that is already there, and here there is none to double. An
expression tag hugs its braces either way: it is inline content wherever it appears, so it
never had the choice.

## Reason

User comments are valuable and shouldn't be silently removed, and a preserved comment must
land where the author put it. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Related

- [expr_trailing_line](./../expr_trailing_line_prettier_divergence/) — one trailing line comment, every position
- [expr_trailing](./../expr_trailing_prettier_divergence/) — one trailing block comment, every position
- [braced_value_trailing_line](../../prettier_ignore/braced_value_trailing_line_prettier_divergence/) — the same closer rule on a frozen value
