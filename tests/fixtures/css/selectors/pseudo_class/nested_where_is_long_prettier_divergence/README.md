# nested_where_is_long_prettier_divergence

A nested `:where(:is(...):not(...):not(...))` whose inner `:is()` argument list
breaks. The `:is()` sits in a compound of more than two simple selectors
(`:is`, `:not`, `:not`), so prettier indents its broken args an extra level. tsv
indents them one level, uniformly.

tsv: inner `:is()` args one level in
Prettier: inner `:is()` args two levels in

## Reason

Design choice — the same rule as
[compound_args_indent_long](../compound_args_indent_long_prettier_divergence/):
tsv keys a complex selector's indent on combinator presence, not on a flat
simple-selector count, so a single compound (here `:is():not():not()`, which has no
combinator) does not push its pseudo args an extra level deeper than the rule body.
See
[conformance_prettier.md §CSS: Selectors](../../../../../../docs/conformance_prettier.md#css-selectors).
