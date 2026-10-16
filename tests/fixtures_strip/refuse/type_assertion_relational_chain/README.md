# type_assertion_relational_chain

A comparison chain that reads like two type argument lists split by a line break: `f<T>`, then `<U>(z)` on the next line. A `<` after a would-be type-argument close refuses the list on any line, so `x = f<T>⏎<U>(z)` is `x = (f < T) > <U>(z)`, whose right operand is an angle-bracket type assertion — refused (`UnsupportedKind::TypeAssertion`), the span the assertion. Blanking both lists as type arguments would emit `f(z)`, a call: a different program.
