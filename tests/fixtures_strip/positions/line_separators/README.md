# line_separators

U+2028 and U+2029 inside a blanked range stay verbatim, as every line terminator does, so V8's line numbers after them hold; one outside a blanked range stays too.

## Divergence

ts-blank-space blanks U+2028 / U+2029 inside a blanked range to a space (it keeps only LF and CR), which moves every later line up. Node keeps them, as tsv does.
