# Line comment between declaration name and type params

Prettier relocates line comments from between the name and type params to end of line:
`class A // c\n<T> {}` → `class A<T> {} // c`.

We preserve the user's comment placement. The line comment forces a break, so type params go to the next line.

Covers: class declaration, class expression, interface, type alias, function.
