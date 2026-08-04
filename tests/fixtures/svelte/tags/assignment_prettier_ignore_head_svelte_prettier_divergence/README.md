# Frozen head + an assignment's clarity parens

An assignment used as a value gets **clarity parens** (`{@html (a = b)}`,
`{#if (a = b)}`). Those parens are the *printer's*, not the author's, so when an
own-line directive freezes the head they stay **outside** the verbatim slice —
only the interior stays as authored. This is the same rule the freeze already
follows for the prefix keyword and the closing `}`, one delimiter further in.

The rule is keyed on the **value**, not on the head's shape, so every braced
position answers it identically: the prefixed heads (a tag, a block, a `{...}`
spread) and the unprefixed `{…}` values (an attribute value, an expression tag)
alike. Skipping the parens on a frozen value would not preserve *more* of what the
author wrote — it would **delete** the parens they did write, inside the one region
the directive says not to touch. The single exception is `{@const}`'s initializer,
where the paren is fully redundant and normalizes away frozen or not (the ordinary
[const/value_prettier_ignore_head](../const/value_prettier_ignore_head/) pins it).

Both divergences land on one input, so this is a `_svelte_prettier_divergence`.

**Formatter (vs prettier).** On a head with no parens prettier **relocates** the
directive flush onto the prefix's line and freezes anyway — inert under tsv's
placement floor, so following it would lose the freeze on tsv's own second pass;
the sibling
[prefixed_value_prettier_ignore_head](../prefixed_value_prettier_ignore_head_prettier_divergence/)
pins that shape. Here the parens make it worse: the same `remove_parens` pass that
discards the wrapper's `leadingComments` (below) **deletes the directive
outright**, so `output_prettier.svelte` carries neither the comment nor the freeze
— content loss, not a layout difference. tsv keeps the directive on the line the
author gave it, freezes the interior, and leaves the parens outside the slice. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).

**Parser (vs Svelte).** The parens make this the sanctioned paren-attachment
case: Svelte parses template expressions with `preserveParens: true` and then
`remove_parens` discards the wrapper **and its `leadingComments`**, so the
directive survives only in the root `comments` array; tsv (no
`ParenthesizedExpression` node) attaches it to the inner expression
(`expected_ours.json` vs `expected_svelte.json`). The comment is never lost —
only its attachment differs. Reaching it is unavoidable here rather than
incidental: tsv's own canonical output for a frozen assignment head *is* a
directive leading a parenthesized expression, so every fixture for this rule
carries the parser divergence too. The sibling
[template_expr_paren_comment_svelte_divergence](../../syntax/comments/template_expr_paren_comment_svelte_divergence/)
isolates the parser difference on its own. See
[conformance_svelte.md §Comment Attachment Differences](../../../../../docs/conformance_svelte.md).
