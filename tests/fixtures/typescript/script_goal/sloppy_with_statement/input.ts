// block body
with (scope) {
	fn();
}

// an empty block body does NOT collapse
with (scope) {
}

// single-statement body
with (scope) fn();

// empty body
with (scope);

// nested
with (outer) with (inner) fn();

// member-expression object
with (a.b.c) fn();

// the `!(logical)` hug keeps the redundant parens
with (!(a || b)) fn();

// an assignment object takes clarity parens
with ((x = fn())) {
	fn();
}

// comments inside the head
with (
	// before object
	scope // inline with object
	// trailing after object
) {
	a();
}

with (/* before */ scope /* inline */) {
	b();
}

// a block comment between `)` and the body stays inline
with (scope) /* comment */ {
}

// a line comment trailing `)` keeps its line
with (scope) // trailing
{
	fn();
}

// an own-line comment before `{` keeps its line
with (scope)
// own line
{
	fn();
}

// a glued pair below the header keeps the one line the author gave it
with (scope)
/* b1 */ /* b2 */
{
	fn();
}

// a run glued onto the header's line moves down to the body whole
with (scope)
	/* b1 */ // b2
	fn();

// 100 chars exactly at the head line - the head stays inline
with (condA && condB && condC && condD && condE && condF && condG && condH && condIIIIIIIIIIIIIII) {
	fn();
}

// 101 chars at the head line - the head wraps
with (
	condA && condB && condC && condD && condE && condF && condG && condH && condIIIIIIIIIIIIIIII
) {
	fn();
}

// 100 chars exactly - the whole statement stays on one line
with (condA && condB && condC && condD && condE && condF && condG && condH && condIIIIIIIIIIIIII) a;

// 100 chars for the head alone - the body goes to the next line
with (condA && condB && condC && condD && condE && condF && condG && condH && condIIIIIIIIIIIIIIIII)
	a;

// 101 chars for the head alone - the head wraps
with (
	condA && condB && condC && condD && condE && condF && condG && condH && condIIIIIIIIIIIIIIIIII
)
	a;
