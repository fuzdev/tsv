# type_parameters_line_break_cr

The type-parameter hazard with a CR-only line break: the `(` moves exactly as for LF.

## Divergence

ts-blank-space tests "spans lines" against LF alone, so it blanks the type parameters in place and ASI ends the statement at `return`. Node moves the `(`, as tsv does.
