<script>
	// An own-line block comment with the comma glued after leads the next element
	const a = [x
/* c */, y];

	// Glued to a stripped paren shell's closer instead, it trails the previous element
	const b = [(
x
)/* c */, y];

	// The run keeps its authored order - the second comment does not cross the first
	const c = [x
/* c1 */ /* c2 */, y];

	// Past the last element the whole run trails it, its comma never being emitted
	const d = [x, y
/* c */];

	// Same on the last element through a stripped shell
	const e = [x, (
y
)/* c */];

	// A line comment in the same position expands the array
	const f = [x
// c
, y];
</script>
