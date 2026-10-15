# signature_before_computed_key

A blanked bodiless member (here an abstract method) between a field whose initializer has no `;` and a member opening with `[`: blanked plain, the key would index the initializer (`a = 1[k] = 2`). The blank starts with a `;`, as a blanked `declare` field's does (`statement_after_js_class_body`). The same holds for an overload signature or an index signature, and for a following generator's `*`.

## Override

ts-blank-space blanks an abstract method, an overload signature and an index signature with no `;` ever, emitting `a = 1` then `[k] = 2`, a different program. The expected output is reviewed rather than derived.

## Divergence

Node writes no `;` here either, emitting the same different program.
