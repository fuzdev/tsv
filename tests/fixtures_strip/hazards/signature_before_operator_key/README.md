# signature_before_operator_key

A blanked bodiless member (here an abstract method) between a field whose initializer has no `;` and a member named `in` or `instanceof`: blanked plain, the key would continue the initializer as the operator (`a = x in y`). The blank starts with a `;`, as before a computed key (`signature_before_computed_key`).

## Override

ts-blank-space blanks a bodiless member with no `;` ever, emitting `a = x` then `in`, a different program. The expected output is reviewed rather than derived.

## Divergence

Node refuses the input: after a bodiless member it reads the `in` member as a variance modifier (`ERR_INVALID_TYPESCRIPT_SYNTAX`), which tsc does not.
