# Line comment between method name and type params

Prettier relocates line comments from between the method name and type params to end of line:
`fn1 // c\n<T>() {}` → `fn1<T>() {} // c`.

We preserve the user's comment placement. The line comment forces a break, so type params go to the next line.

Covers: class method, object method, interface method signature.
