# custom_selector_comment_prettier_divergence

Comments in a `@custom-selector` prelude are preserved at every gap — leading
(`@custom-selector /* c */ :--a h1`), after the name (`:--b /* c */ h1`), before and
after a list comma (`h1 /* c */, h2`, `h1, /* c */ h2`), in a combinator gap
(`h1 /* c */ > h2`) and trailing before the `;` (`h2 /* c */;`) — each padded by one
space on each side, the rule every other selector-comment position takes.

Prettier **drops every one of them**: its `custom-selector` arm reads `node.params`,
postcss's comment-stripped copy of the prelude (the other prelude readers take
`node.raws.params.raw`), so the comments never reach its printer. On a spelling where a
comment is glued to a selector on both sides (`h1/* c3 */,h2`, `h1/* c5 */>h2`) it goes
further and drops the **name** as well (`@custom-selector h1/* c3 */,h2;`), output its own
parser then throws on — the `_compact` variant keeps one side of each of those gaps open so
prettier's chain stays expressible (it lands on `output_prettier.svelte`).

parseCss accepts every line and strips the comments from the wire `prelude` string, so
this is a prettier-only divergence. `unformatted_ours_compact` / `unformatted_ours_spaces`
pin tsv's normalization of the glued and padded spellings.

## Reason

◆content_preservation ◆prettier_bug. Silently dropping authored content is never the
defensible side. See
[conformance_prettier_css.md §CSS: Comments](../../../../../docs/conformance_prettier_css.md#css-comments)
(`@custom-selector prelude comments`), and the routing itself under
[§CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
(**The two prelude readers**).

## Related

- [custom_selector](../custom_selector/) — the comment-free prelude (a match)
- [scope_prelude_gap_comment](../scope_prelude_gap_comment_prettier_divergence/) — the same gap-comment rule on `@scope`, where prettier keeps the comments but freezes their spacing
