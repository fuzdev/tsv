# modifier_generator

A blanked modifier that opens a generator method after a field with no `;` becomes a `;`, or the `*` would multiply the previous initializer (`a = 1 * gen() {}`, a syntax error) — the computed-key hazard of `modifier_computed_key` with a `*` for the `[`.

## Override

ts-blank-space writes no `;` here and emits `a = 1` then `*gen() {}`, which does not parse. The expected output is reviewed rather than derived: the `;` at the modifier, ts-blank-space's own placement for the computed key. Node writes exactly this.
