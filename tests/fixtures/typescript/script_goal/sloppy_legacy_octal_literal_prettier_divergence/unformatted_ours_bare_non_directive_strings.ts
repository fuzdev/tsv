// a template literal is not a StringLiteral, so it is not a directive candidate:
// the program's prologue closes on it, and no string statement below can reopen one
`use strict`;
'use strict';

const a = 010;
const b = 08;
const c = 089;
const d = 08.5;
const e = 0777;

const obj = { 010: 'value1', 08: 'value2', 0777: 'value3' };

type A = 010;
type B = 08.5;

let f: { 010: string; 089: number };

const g = a[010] + 08.5e1;

// a parenthesized string is never a directive, at a function body's head either
function paren() {
	('use strict');
	return 010;
}

// a Use Strict Directive is the exact code point sequence, so an escaped spelling
// is an ordinary directive and this body stays sloppy
function esc() {
	'use\x20strict';
	return 010;
}

// a function body's prologue is its own strictness scope, restored on the way out
function fn() {
	'use strict';
	return 1;
}
const h = 010;

// a class is strict by construction, and that is restored on the way out too
class C {}
const i = 010;

// a plain nested block carries no directive prologue at all
{
	'use strict';
	const j = 010;
}
