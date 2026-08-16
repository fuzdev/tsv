# shorthand_comment_prettier_divergence

The attribute shorthand collapse (`a={a}` → `{a}`, `class:a={a}` → `class:a`) has nowhere to
put a comment written inside the braces, so performing it **deletes** the comment. tsv treats
the collapse as a layout choice a commented value declines; prettier collapses and drops.

tsv: `<Comp a={/* c */ a} />` (declined, comment preserved)
Prettier: `<Comp {a} />` (collapsed, comment stripped)

The declined form is exactly what the same value already prints when the name and the
identifier differ (`b={/* c */ a}`) — the refusal routes the value through the ordinary
`{…}` path, which preserves the leading, trailing and `//` positions alike (the `//` keeps
the `}` on its own line, as in
[expr_trailing_line](../../syntax/comments/expr_trailing_line_prettier_divergence/)).

Affected: plain attributes on components and HTML elements, and the four shorthand-bearing
directives `class:` / `bind:` / `let:` / `style:`. An **uncommented** shorthand still
collapses (the in-fixture controls, one of them beside a declining sibling in the same tag),
and the `bind:` `{getter, setter}` sequence is never a shorthand, so it is untouched —
prettier keeps that comment too. The quoted spelling `a="{a}"` reaches the same site and
declines to the same unquoted form (`unformatted_ours_quoted`).

A comment inside an *authored* shorthand (`<Comp {/* c */ a} />`) is a parse error in both
Svelte and tsv, so the explicit `a={…}` form is the only shape this rule can reach.

The question is asked **on the page**, not on the emit axis: a block comment glued to the
identifier is *owned* by it and rides inside the expression's own doc, so an emit-keyed scan
reports the braces comment-free and every **leading** case here collapses and drops.

## Reason

User comments are valuable and shouldn't be silently removed, and the shorthand is a pure
layout choice — the same refusal shape as a commented last argument defeating a call's hug.
See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [expr_trailing](../../syntax/comments/expr_trailing_prettier_divergence/) — the same content loss at the non-shorthand `{…}` value, which this fixture completes
- [shorthand_basic](../shorthand_basic/) — the uncommented collapse
