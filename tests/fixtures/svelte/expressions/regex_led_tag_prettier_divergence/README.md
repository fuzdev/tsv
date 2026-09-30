# regex_led_tag_prettier_divergence

An expression tag in text position whose expression PRINTS first a regex literal — the author wrote the literal in a pair the printer strips (`{(/a/).test(b)}`), or the whole expression in one: prettier-plugin-svelte prints the tag with its `{` against the literal's `/`, and tsv wraps the expression in a pair.

tsv: `{(/a/.test(b))}`, `{(/a/ ? b : c)}`
Prettier: `{/a/.test(b)}`, `{/a/ ? b : c}`

## Reason

**Parser compatibility.** In text position Svelte reads `{/` as a block close (`{/if}`), so prettier's output does not parse — not for Svelte, not for prettier's own next pass, and not for tsv, whose next save of its own output would fail. The pair is decided from what the tag PRINTS, not from the source: the walk down the expression's left spine asks each position's own paren question and stops at the first pair the printer keeps (`{(/a/ + b).c}` already opens on one and takes nothing more, and so does a root assignment's own pair, `{(/a/.x = 1)}`). A pair written around the literal alone moves out to the whole expression (`unformatted_ours_inner_pair`).

Two positions need nothing, and both formatters agree there: a comment printed ahead of the literal keeps the `{` off the `/` (`{/* c */ /a/.test(b)}` parses), and an attribute value — quoted, unquoted or a directive's — and a `<textarea>`'s RCDATA content are read by Svelte's `read_sequence`, which takes a `{/` as an expression.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Prettier bug index and [conformance_prettier_svelte.md §Svelte: Regex-led expression tag](../../../../../docs/conformance_prettier_svelte.md#svelte-regex-led-expression-tag).
