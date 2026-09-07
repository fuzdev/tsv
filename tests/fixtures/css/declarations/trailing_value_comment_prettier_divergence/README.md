# trailing_value_comment_prettier_divergence

A comment trailing the **last** declaration's value in a block, where the author wrote no
`;`, stays inside that declaration for tsv and moves out of it for prettier.

```
authoring   top: 0 /* comment */⏎}
tsv         top: 0 /* comment */;
Prettier    top: 0; /* comment */
```

Both formatters are stable on both forms, so the divergence is reachable only from the
semicolon-less authoring — `unformatted_ours_no_semicolon` is that source, and
`variant_no_semicolon` is the form prettier normalizes it to (which tsv keeps too). With
the `;` authored, the two agree and the comment stays inside on both sides, which is what
`input.svelte` pins.

## Reason

Stable quirk, and prettier's own answer is not uniform: with the `;` authored it keeps the
comment inside the declaration, without one it moves it past a `;` that no longer exists in
the source — postcss ends the declaration at the value and re-parents the trailing comment
as a sibling node. tsv is uniform, and the `;` it prints beside the comment is one tsv
emits rather than one the author wrote, so carrying the comment across it would relocate it
against [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
See [conformance_prettier_css.md §CSS: Comments](../../../../../docs/conformance_prettier_css.md#css-comments).

## Related

- [in_property_value](../../tokens/comments/in_property_value/) — the same trailing position with the `;` authored, where both formatters agree
