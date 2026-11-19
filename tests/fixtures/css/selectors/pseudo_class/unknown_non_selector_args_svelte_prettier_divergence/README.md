# unknown_non_selector_args_svelte_prettier_divergence

An unknown functional pseudo-class or pseudo-element whose argument is not a selector
list (`a:foo(1 + 2)`, `a::foo(1 + 2)`, `a:foo('x', 1 + 2)`, `a:foo(f(1 2) g())`). Per
[Selectors 4 §pseudo-classes](https://drafts.csswg.org/selectors/#pseudo-classes) a
functional pseudo is `: <function-token> <any-value> )`, and css-syntax-3's `<any-value>`
is one or more tokens, any but a bad-string or bad-url token and an unmatched `)`, `]` or
`}` — a top-level `;` or `!` included. tsv keeps the argument; an argument that parses as
a whole selector list still becomes one (`:foo(.a, .b)`, `:foo(2n + 1)`). A non-ASCII
space (`a:foo(<NBSP>)`) is an identifier code point, so that argument is not empty.

## Svelte divergence

Svelte's `parseCss` reads every unknown pseudo's argument as a selector list and rejects
anything else (`css_expected_identifier`), so `expected_svelte.json` records the parse
error. Only acceptance diverges: tsv's wire keeps Svelte's shape for the kept argument,
`args: null` on a pseudo-class and no `args` key on a pseudo-element, the node's span
already covering the parens. A comment inside the argument is on the root `comments`
list, as any other. The `@supports selector(…)` case reads the same pseudo through the
same parser. See
[conformance_svelte.md §CSS Corrections](../../../../../../docs/conformance_svelte.md#css-corrections).

The rejections stay rejections in both parsers (`input_invalid_*`): an empty,
whitespace-only or comment-only argument (`<any-value>` needs a token), an unmatched `]`,
`}` or `{`, a bad url (`url(x y)`) and a bad string (an unescaped newline).

## Prettier divergence

The argument has no grammar beyond its tokens, so tsv changes none of them. It takes the
whitespace and quote normalization a comment-bearing custom-property value takes:
whitespace runs collapse, the padding inside parens trims, strings requote, a comma takes
one space after it, and a comment takes one space on either side (`1/**/2` →
`1 /**/ 2`). Numbers, units and hash tokens stay as written (`.50em`, `#FFF`, `1.0`,
`a\ .5`), as prettier keeps them. A glued `1+2`, `1>2` or `1~2` stays glued, where
prettier's selector parser reads the operator as a combinator and spaces it
(`output_prettier.svelte`): `+2` is one number token, so spacing `1+2` retokenizes it, and
tsv inserts no space between tokens the author glued (`1>2`, `1~2`) beyond the comment
spacing it shares with declaration values. Where prettier's selector parser cannot read the
argument (a bare `( … )`, a `;` or `!`, a comment) it keeps the whole argument as authored,
padding and space runs included, and it keeps a function's inner padding everywhere; tsv
normalizes both. `divergent_variant_spaces` is prettier's stable form of
`unformatted_ours_spaces`; tsv takes that variant to a third form. See
[conformance_prettier_css.md §CSS: Selectors](../../../../../../docs/conformance_prettier_css.md#css-selectors).

## Related

- [unknown_functional_args](../unknown_functional_args/) — selector-shaped arguments of an unknown pseudo, which structure
- [variables](../../../values/variables/) — the custom-property values whose whitespace and quote normalization this argument shares
