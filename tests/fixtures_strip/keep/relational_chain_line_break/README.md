# relational_chain_line_break

A comparison chain that reads like type arguments followed by a line break: `f<T>`, then `+c` on the next line. A `+` or `-` after a would-be type-argument close is a unary operator, so tsc refuses the list on any line and `x = f<T>⏎+c` is `x = (f < T) > +c`, after a name, a member and a `new` callee alike. Nothing is type-only, so the output is the input; blanking `<T>` would emit `f + c`, a different program.
