# scss_directive_prelude_verbatim_prettier_divergence

The selector-branch sibling of
[scss_directive_number_preserved](../scss_directive_number_preserved_prettier_divergence/):
same divergence, same reason, a different normalization axis.

tsv treats an unrecognized at-rule's prelude as the opaque token stream CSS
Syntax 3 §5.4.2 says it is and preserves it byte-for-byte. Prettier re-parses
the prelude of any at-rule on its hardcoded SCSS list — and for `@at-root`,
`@extend` and `@nest` it parses it as a **selector**, so the whitespace runs
collapse, quotes normalize and combinators get spaced.

| Prelude                            | tsv                    | Prettier               |
| ---------------------------------- | ---------------------- | ---------------------- |
| `input[type="radio"]   >   .class1` | verbatim               | `input[type='radio'] > .class1` |
| `foo⏎bar`                          | newline kept           | `foo bar`              |
| `@unknown foo   bar`               | verbatim               | verbatim (**matches**) |

## The control names the mechanism

The third case is the null control, varying only the at-rule **name** while the
prelude text stays identical: `@unknown foo   bar` keeps its whitespace run in
*both* formatters. So this is not a general "prettier normalizes unknown
preludes" rule that tsv is missing — prettier's behavior is keyed entirely on
its own SCSS-directive name table (`parser-postcss.js`), and outside that table
the two formatters already agree.

## Reason

tsv's scope is standard CSS (plus Svelte/TypeScript). `@at-root` is an SCSS
directive, and adopting prettier's output here would mean hardcoding "an
`@at-root` prelude is a selector" — SCSS grammar knowledge, for a construct
that has no meaning in the language tsv formats.

The grammar-free alternative — collapsing whitespace *runs* in any raw prelude,
which CSS Syntax 3 §4.3.1 makes token-preserving — is declined for two reasons:
it would still diverge on the `@unknown` control above (trading an agreement for
a divergence), and a scan that collapses runs in text tsv has not tokenized
corrupts string interiors and escaped whitespace (`@foo "a   b"`, `@foo a\  b`)
— the escape/comment-opacity class that has bitten the value scanners.

Output stays valid CSS and is a fixed point in both formatters; the divergence
is one of scope, not correctness.

See
[conformance_prettier_css.md §CSS: At-Rules](../../../../../docs/conformance_prettier_css.md#css-at-rules)
(`SCSS directive preludes`, Design choice).
