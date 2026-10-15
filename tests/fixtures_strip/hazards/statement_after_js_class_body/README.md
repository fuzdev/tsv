# statement_after_js_class_body

The same hazard in a class body: a blanked `declare` field after a field with no `;` starts with a `;`, or the computed key that follows would index the previous initializer (`1[k] = 2`).

## Divergence

Node writes no `;` here and emits `a = 1[k] = 2`, a different program. tsv follows ts-blank-space.
