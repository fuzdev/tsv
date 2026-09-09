// a string statement heading the prologue is a directive whatever its content, and only
// the exact code point sequence `use strict` turns strict mode on — so this one leaves
// the script sloppy and every legacy escape below is legal
'\7';

// LegacyOctalEscapeSequence: one to three octal digits, read in base 8
const a = '\7';
const b = '\101';
const c = '\00';
const d = '\377';

// NonOctalDecimalEscapeSequence: `\8` and `\9` stand for the digit itself
const e = '\8';
const f = '\9';

// `\0` followed by a decimal digit is the legacy form too — the NUL escape is `\0` with
// no digit after it, and stays legal in strict code
const g = '\08';

// at most three digits, and three only when the first is `0`-`3`: `\400` is `\40` then a
// literal `0`, and `\412` is `\41` then a literal `2`
const h = '\400';
const i = '\412';

const j = 'a\1b';

const obj = { '\7': 'value1', '\8': 'value2' };

type A = '\101';

let k: { '\7': string };

let m: import('\101').Foo;

const l = obj['\7'] + '\9';
