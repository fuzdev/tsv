# Object property `:`→value gap, line comment on the `:` line

A line comment authored on an object property's `:` line, ahead of the value
(`k: // c⏎b`). A `//` runs to end-of-line, so the value must move to a new line either
way; the question is where the comment goes.

tsv keeps it where the author put it — trailing the `:` — and hangs the value one level
in below it, the answer the declarator `=` already gives (`const a = // c⏎\tb`):

```
k: // c1                 // c1
	b,                   k: b,
```

Prettier **hoists** the comment above the whole property (right), re-binding it from
the value to the property. The **own-line** authoring (`k:⏎\t// c⏎\tb`) keeps its own
line in both formatters — the plain
[property_line_comment_after_colon](../property_line_comment_after_colon/) fixture — so
under tsv both placements are stable and neither is moved onto the other's line.

## Reason

Per the [Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy),
trailing comments stay trailing and a dual-stable authoring is not collapsed to one
canonical form. A comment written after the `:` refers to the value side of the
property; hoisting it above the key changes that association, and pushing it down onto
its own line moves it off the line the author chose.

What the cases pin:

- **c1** — the plain case, in an expanded object beside another property.
- **c2/c3** — a run: the first comment stays on the `:` line, the rest keep their own
  lines in order, and an author blank before the value survives. Prettier hoists only
  the first comment and leaves the rest below the `:`, splitting one authored run
  across two positions.
- **c4/c5** — a block ahead of the line comment on the `:` line stays there with it.
- **c6/c7** — a computed key and a quoted key take the same gap.
- **c8/c9** — an object value and a function value hang below the comment too.
- **c10** — a property of an object passed as a call argument.

`unformatted_ours_compact` authors every case unspaced and flush; tsv reaches the same
fixed point. The import attribute's `:` takes the same answer
([attributes_value_colon_line_comment](../../../modules/imports/attributes_value_colon_line_comment_prettier_divergence/)).

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
