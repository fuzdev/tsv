# type_parameters_line_break_async

The type-parameter hazard on an `async` arrow: a line break after `async` is not allowed, so the `(` moves to the start of the blanked `<…>`.

## Divergence

Node blanks the type parameters in place, leaving `async` alone on its line, a syntax error. tsv follows ts-blank-space.
