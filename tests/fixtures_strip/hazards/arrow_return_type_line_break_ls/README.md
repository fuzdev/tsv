# arrow_return_type_line_break_ls

The arrow return-type hazard with a U+2028 line break: the `)` moves exactly as for LF.

## Divergence

ts-blank-space tests "spans lines" against LF alone, so it blanks the return type in place and leaves the U+2028 before `=>`, a syntax error. Node moves the `)`, as tsv does.
