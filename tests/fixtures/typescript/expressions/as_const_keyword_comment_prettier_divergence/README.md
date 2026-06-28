# as_const_keyword_comment_prettier_divergence

A comment between the `as` keyword and `const` in a const assertion. tsv keeps it
where the author wrote it; prettier relocates it.

A **block comment** (`1 as /* c */ const`) stays between `as` and `const` for tsv
(`1 as /* c */ const;`); prettier moves it before the whole expression
(`1 /* c */ as const;`).

A **line comment** (`1 as // c` then `const`) trails the keyword for tsv, with
`const` dropped to the next line — emitting it inline would swallow the `;`
(`1 as const // c;`, a content loss); prettier floats it out past the statement
(`1 as const; // c`).

Per Comment Position Philosophy, tsv preserves comment position except around
pure separators (`;`, `,`); the `as`…`const` gap is not a separator, so the
comment stays put. This is consistent with the regular
[as_satisfies_keyword_comment](../as_satisfies_keyword_comment/) (block comment in
`x as /* c */ T`, preserved by both) and
[as_satisfies_value_line_comment](../as_satisfies_value_line_comment_prettier_divergence/)
(line comment after the cast keyword). The `const` assertion is the lone cast type
where prettier relocates a block comment; `as <literal>` (`x as /* c */ 5`) and
`satisfies const` (`x satisfies /* c */ const`) stay put in both.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
