# modifier_operator_key

A blanked modifier that opens a member named `in` or `instanceof` after a field with no `;` becomes a `;`, or the key would continue the previous initializer as the operator: `a = x` then `in` then `y` reads `a = x in y`, a different program that still parses. The same hazard as `modifier_generator`, with the key itself for the `*`.

## Override

ts-blank-space writes no `;` here and emits `a = x` then `in`, silently changing the program. The expected output is reviewed rather than derived: the `;` at the modifier, ts-blank-space's own placement for a computed key. Node writes exactly this.
