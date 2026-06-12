# scss_directive_number_preserved_prettier_divergence

SCSS-family at-rule directives (`@include`, `@mixin`, `@if`, `@for`, `@each`,
`@while`, `@function`, `@return`, `@debug`) are not standard CSS. tsv treats
their preludes as opaque raw text and preserves them verbatim; Prettier
recognizes the directive family, parses the params as a CSS value list, and
number-normalizes them.

tsv: `@include foo(.5s)` — prelude preserved verbatim
Prettier: `@include foo(0.5s)` — leading zero added; trailing zeros stripped

| Number | tsv    | Prettier |
| ------ | ------ | -------- |
| `.5s`  | `.5s`  | `0.5s`   |
| `1.50` | `1.50` | `1.5`    |

## Reason

tsv's scope is standard CSS (plus Svelte/TypeScript). `@include`/`@mixin`/etc.
are SCSS/Sass preprocessor directives (mixins are only a Level-1 CSS draft), so
tsv does not parse their preludes into a value AST and preserves them
byte-for-byte. Prettier maintains an SCSS-directive list (`parser-postcss.js`)
whose params it re-parses with `parseValue`, then runs `adjustNumbers` over the
result. tsv applies that same number normalization only to standard-CSS
contexts — declaration values and `@media`/`@supports` preludes — leaving
unrecognized directive preludes untouched. See conformance_prettier.md
§CSS: At-Rules.
