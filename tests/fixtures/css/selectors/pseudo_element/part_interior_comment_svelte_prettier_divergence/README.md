# part_interior_comment_svelte_prettier_divergence

Comments *between* two `::part()` identifiers are preserved — the interior
positions the edge-only [part_comment](../part_comment_prettier_divergence/)
fixture leaves out. A comment is inter-token trivia (removed at tokenization,
producing no token, not even whitespace), so it separates the identifier run
without joining the names: `name1/* c */name2` reads as two part names, exactly
like `name1 name2`. tsv preserves a comment in every interior gap — between the
first two names, between later names, several in one gap, and interleaved with
the edge comments.

## Svelte Behavior

Svelte's `parseCss` rejects an interior comment: its `::part()` identifier
scanner does not tokenize comments there and reports "Expected a valid CSS
identifier" (`css_expected_identifier`). This is the canonical-fails-tsv-ok
pattern — tsv follows the CSS spec where Svelte's parser is incomplete, the same
shape as [combinator_comment](../../combinator_comment_svelte_prettier_divergence/)
(the same rejection at a selector combinator boundary).

## Prettier divergence

tsv normalizes the gap spacing around each interior comment to a single space —
the same rule as every other selector-comment position (`:is()`/`:nth-*()`
argument comments, `::part()` edge comments) — while prettier freezes the source
whitespace verbatim. `prettier_variant_spaces` pins the padded forms and
`prettier_variant_compact` the glued forms that prettier keeps stable; tsv
normalizes both to `input.svelte`. In the compact form two adjacent comments in
one gap keep the space between them (`/* c3 */ /* c4 */`) — gluing them would
make prettier relocate the rule's `{` onto its own line (a separate quirk), so
only the outer edges glue.

## tsv Behavior

tsv registers these gap comments at parse time and re-emits them through
`comments_in_range`, interleaving each at its authored position between the
identifiers, so the surrounding whitespace normalizes uniformly (the shared
selector-comment doc path). The identifier run is otherwise unchanged.

See [conformance_prettier.md §CSS: Comments](../../../../../../docs/conformance_prettier.md#css-comments)
and [conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

## Related

- [part_comment](../part_comment_prettier_divergence/) — the `::part()` *edge* comments (before/after the run; parseCss accepts these)
- [combinator_comment](../../combinator_comment_svelte_prettier_divergence/) — the same rejection + single-space normalization at a combinator boundary
- [nth_comment](../../pseudo_class/nth_comment_svelte_prettier_divergence/) — `:nth-*()` argument comments
