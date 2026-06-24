# Line comment between declaration name and type params

Prettier relocates line comments from between the name and type params:
`class A // c\n<T> {}` → `class A<T> {} // c` (end of the declaration line). For a
type alias prettier instead floats it just past the `=` (`type D // c\n<T> = T`
→ `type D<T> = // c\nT`), since there is no statement tail to trail.

We preserve the user's comment placement. The line comment forces a break, so
the type params go to the next line.

Covers: class declaration, class expression, interface, type alias, function.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
