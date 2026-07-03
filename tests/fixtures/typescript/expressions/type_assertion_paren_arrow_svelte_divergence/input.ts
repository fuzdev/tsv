// A type assertion over a parenthesized arrow keeps the assertion reading
// (TypeScript's parse; acorn-typescript reads a generic arrow's type
// parameters instead).
<any>(() => {});
<T>(() => {});

// Contrast: a type that can't parse as type parameters is an assertion in
// both parsers.
<any[]>(() => {});
