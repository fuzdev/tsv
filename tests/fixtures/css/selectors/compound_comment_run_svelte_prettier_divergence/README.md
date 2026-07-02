# compound_comment_run_svelte_prettier_divergence

A run of two or more **glued** comments between compound members
(`.a/* c *//* d */.b`) is inter-token trivia per [CSS Syntax 3 §comments](https://www.w3.org/TR/css-syntax-3/#comment-diagram):
with no whitespace anywhere, the comments are removed at tokenization and the
class selectors are adjacent, so the selector is a **compound** (`.a.b`), not a
descendant (`.a .b`). tsv keeps it a compound and emits the glued comment run
**verbatim** (never inserting a space between the two comments — that space would
tokenize as whitespace and turn the compound into a descendant on re-parse).

This is the multi-comment generalization of the single glued comment covered by
[combinator_comment](../combinator_comment_svelte_prettier_divergence/) (`.a/* c */.b`);
a single glued comment has no interior to normalize, so it needs no special case.

## Svelte Behavior

Svelte's `parseCss` rejects a comment at a compound-member boundary (its selector
scanner does not tokenize comments there): `css_expected_identifier`. tsv follows
the CSS spec where Svelte's parser is incomplete — the canonical-fails-tsv-ok
pattern.

## Prettier divergence

Prettier keeps the compound and the glued comment run intact (it agrees the
selector is a compound), but relocates the opening `{` onto its own line — a
prettier quirk triggered by the adjacent comments (`output_prettier.svelte`). tsv
keeps `{` inline on the selector line. The comment content and the
compound-vs-descendant distinction are identical under both; only the brace
position differs.

See [conformance_prettier.md §CSS: Comments](../../../../../docs/conformance_prettier.md#css-comments)
and [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections).
