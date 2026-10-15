# signature_before_generic_arrow

A blanked bodiless signature after a statement with no `;`, before a line opening with a generic arrow: the arrow's type parameters blank too, leaving its `(` the line's first token, so blanked plain the next line would continue the statement as a call (`b(z) => z`, a syntax error). The blank starts with a `;`, as `overload_before_paren`'s does before a `(`.

## Override

ts-blank-space blanks the signature with no `;` and emits `let a = b` then `(z   ) => z`, which does not parse. The expected output is reviewed rather than derived.

## Node

Node writes the same output as ts-blank-space.
