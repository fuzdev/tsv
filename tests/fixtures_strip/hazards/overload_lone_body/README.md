# overload_lone_body

A statement blanked whole as the lone body of an `if` (or a loop, or a label) becomes an empty statement, `;`, so the statement after it cannot become the body: blanked plain, `if (a) function f(): void` then `g()` reads `if (a) g()`. An overload signature is the one such statement tsc's grammar allows there; tsc reports only the checker error TS2391 (no implementation follows), so the shape parses. A type alias, an interface, a `declare` statement, a type-only import or export and a namespace in that position are grammar errors.

## Override

ts-blank-space writes no `;` here and emits `if (a)` then `g()`, a different program. The expected output is reviewed rather than derived.

## Divergence

Node refuses the input (`ERR_INVALID_TYPESCRIPT_SYNTAX`).
