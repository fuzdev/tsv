# overload_before_paren

A blanked overload signature after a statement with no `;`, before a line opening with `[` (or `(`, a template, an operator): blanked plain, the next line would continue the statement (`b[1].map(f)`). The blank starts with a `;`. An overload with no implementation after it is a checker error (TS2391), not a parse error, so the shape parses.

## Override

ts-blank-space blanks an overload signature with no `;` ever, emitting `let a = b` then `[1].map(f)`, a different program. The expected output is reviewed rather than derived; Node writes exactly this.
