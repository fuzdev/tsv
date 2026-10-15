# arrow_return_type_line_break_cr

The arrow return-type hazard with a CR-only line break: the `)` moves exactly as for LF.

## Divergence

ts-blank-space tests "spans lines" against LF alone, so it blanks the return type in place and leaves the CR before `=>`, a syntax error. Node moves the `)`, as tsv does.
