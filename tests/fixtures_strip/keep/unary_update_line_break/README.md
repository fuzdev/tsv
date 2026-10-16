# unary_update_line_break

A prefix operator over a postfix update, then a line that opens with `<` or `(`. A postfix update ends an `UpdateExpression`, which no call, member access, template or type argument list continues, so `x = !b++⏎<T>(z)` is the comparison `(!b++ < T) > (z)`, and `y = -b--⏎(z)` and `w = !b++⏎(a) => a` are two statements each. Nothing is type-only, so the output is the input; blanking `<T>` would emit `x = !b++` and a separate `(z)`.
