# svelte_element_this_string_prettier_divergence

prettier-plugin-svelte 4.x ignores `singleQuote` for a brace-wrapped string literal
in `<svelte:element this={…}>`, always emitting double quotes. tsv honors
`singleQuote` (and escapes the literal), matching how prettier formats the same
string anywhere else.

tsv:

```svelte
<svelte:element this={'hello'}></svelte:element>
```

Prettier:

```svelte
<svelte:element this={"hello"}></svelte:element>
```

The plain (unbraced) `this="hello"` HTML attribute and non-literal expressions
(`this={tag}`) are unaffected — only the brace-wrapped string literal diverges.
Those non-diverging forms are covered by [svelte_element_this](../svelte_element_this/)
(both formatters keep them stable).

## Reason

**Prettier bug.** prettier-plugin-svelte's `this={…}` printer hardcodes the literal
as `"${value}"` (a regression in the 4.x modern-ast migration). This both ignores
`singleQuote` and skips escaping, so a value containing a quote or backslash is
mangled regardless of options — `this={'a"b'}` is emitted as the invalid
`this={"a"b"}`, and `this={'a\\b'}` corrupts to `this={"a\b"}` (a backspace). tsv
delegates the literal to the normal string printer, so quote choice and escaping
always match prettier's own JS string output.

The trigger is narrow: only a *directly* brace-wrapped string `Literal` hits the
hardcoded path. Concatenations (`this={'a' + 'b'}`), template literals
(`` this={`hello`} ``), ternaries, and every other expression delegate to the JS
printer and format identically in both tools — covered by
[svelte_element_this](../svelte_element_this/). One adjacent quirk *is* encoded
here: a *parenthesized* literal `this={('hello')}` fails prettier's
`{`-precedes-literal check, so prettier takes the unbraced branch and collapses
the whole binding to the plain attribute `this="hello"` — a structural rewrite,
not just a quote swap. tsv strips the redundant parens but keeps the expression
form, `this={'hello'}`. `unformatted_ours_paren.svelte` holds the paren input
(tsv normalizes it to `input`); `variant_paren_collapse.svelte` holds prettier's
collapsed target (both formatters keep it stable), pinning prettier's output via
the cross-path discovery rule. The escaping cases above, by contrast, stay
prose-only — their output is invalid/non-convergent, so no stable oracle exists.

A fix has been prepared for prettier-plugin-svelte that restores delegation to the
JS printer (the pre-modern-ast behavior). Once it releases and tsv's formatting
oracle is re-pinned, prettier and tsv will agree again and this divergence can be
retired.

See [conformance_prettier.md §Svelte: Elements](../../../../../docs/conformance_prettier.md#svelte-elements).
