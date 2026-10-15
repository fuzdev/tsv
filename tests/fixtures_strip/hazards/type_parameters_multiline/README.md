# type_parameters_multiline

Type parameters broken across lines, as a formatter writes a long list: a line break between the `<` and the parameters' `(` moves the `(` to the start of the blanked `<…>` and blanks the original, as in `type_parameters_line_break`, whether or not ASI could reach it.

## Divergence

Node blanks the type parameters in place and keeps the `(`. The same program; tsv follows ts-blank-space.
