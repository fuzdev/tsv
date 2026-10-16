// a `typeof` query's type arguments open only at a lone `<`, so a `<<` after the query is the
// shift operator, its left operand the whole assertion
const a1 = (a as typeof b) << c;
const a2 = (a satisfies typeof b) << c;
const a3 = (a as typeof b.c) << d;
const a4 = (a as typeof b<T>) << c;

// the other operators after the query
const b1 = (a as typeof b) >> c;
const b2 = (a as typeof b) + c;
