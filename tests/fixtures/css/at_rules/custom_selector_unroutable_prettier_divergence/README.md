# custom_selector_unroutable_prettier_divergence

The preludes prettier's `@custom-selector` arm cannot route. A `@custom-selector` prelude that is not `:--name`, whitespace, then a selector list —
a name with nothing after it (`@custom-selector :--a;`) and a list with no name
(`@custom-selector h1,h2;`). Per CSS Syntax 3 an at-rule prelude is consumed as
component values whatever they are, and parseCss stores it raw, so both parse; tsv keeps
the prelude verbatim, as it does every at-rule prelude it cannot structure (the
`@supports` / `@container` raw fallback). Stable under tsv.

The next two lines are the opposite: `@custom-selector :-- h1, h2;` **is** the shape, with
the shortest `<extension-name>` css-extensions-1 allows (any identifier starting with two
dashes — "or even exotic names like `--` or `------`"), and so is `:\-\-b h1, h2` — the
name is the **decoded** identifier (an escape's payload is ident content, the crate-wide
rule), while the printed name keeps the author's spelling. tsv structures both like any
other (`unformatted_ours_compact` pins `h1,h2` → `h1, h2` on each). The last case is the
block form css-extensions-1's own issue text sketches (`@custom-selector :--c { expansion:
h1, h2; }`): a bare name, then a block, which tsv formats as the declaration block it is.

Prettier's `custom-selector` arm splits the name off with `params.match(/:--\S+\s+/)[0]`
and indexes the match unconditionally, so a prelude the regex does not match — the
first two lines, the bare `:--` name (`\S+` wants a character after the dashes), the
escaped spelling (the regex wants literal dashes), and the block form (no whitespace
after the name once postcss trims the params) — makes it **throw**:

```
Cannot read properties of null (reading '0')
```

so there is no formatting oracle. `prettier_rejects.txt` pins the error; rule F6
live-verifies that prettier still rejects the input.

See [conformance_prettier_ts.md §Prettier rejects valid input](../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input).

## Related

- [custom_selector](../custom_selector/) — the routable prelude (a match)
