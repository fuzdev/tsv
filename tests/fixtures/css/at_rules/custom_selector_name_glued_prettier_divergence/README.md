# custom_selector_name_glued_prettier_divergence

A `@custom-selector` prelude whose `:--name` has selector tokens glued to it
(`:--a:hover h1,h2`, `:--b.class1 h1,h2`). tsv structures a prelude only when it is a
`:` glued to a `--`-led ident, **then whitespace or a comment**, then a selector list —
the gap is what tells a name from a list that opens with a pseudo-class, since
`:--a:hover` is one compound to the selector grammar. Anything else is kept as authored,
the way every unstructurable prelude is, so the list after the glued name is not
normalized either (`h1,h2` stays).

Prettier's arm takes the whole non-whitespace run as the name (`params.match(/:--\S+\s+/)`)
and parses the rest as the list, so it prints `:--a:hover h1, h2`. The two readings are
not both wrong: css-extensions-1's `<custom-selector>` is `: <extension-name>` with
optional `( <custom-arg>+# )` args and nothing else glued, and its grammar puts no
required whitespace before the `<selector-list>`, so `:--a:hover, .b` could as well be the
name `:--a` and the list `:hover, .b`. Neither reading is implemented anywhere tsv can
measure; the input is ambiguous, and tsv declines to restructure it.

## Reason

◆design_choice. Stable on both formatters; prettier-only (parseCss stores the prelude
raw). See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
(**A third reader: the selector list**).

## Related

- [custom_selector_unroutable](../custom_selector_unroutable_prettier_divergence/) — the shapes prettier throws on
- [custom_selector](../custom_selector/) — the routable prelude (a match)
