<script>
	// object terminal at exactly 100 - the whole call fits, so it stays inline
	obj.method((a) => (b) => ({ c: 1, d: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// object terminal at 101 - hugs `.method(`, the object expands internally
	obj.method((a) => (b) => ({ c: 1, d: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// array terminal at 101 - hugs the same way, with no parens added around the array
	obj.method((a) => (b) => [c, d, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]);

	// a preceding argument does not change the hug
	obj.method('first', (a) => (b) => ({ c: 1, d: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// the heads plus the object opener fit the `.method(` line (exactly 100) - the hug holds
	obj.method((argument1) => (argument2) => (argument3) => (argument4) => (argument5aaaaaaaaaa) => ({ c: 1 }));

	// one char over (101) - the argument breaks out and the chain progressive-indents
	obj.method((argument1) => (argument2) => (argument3) => (argument4) => (argument5aaaaaaaaaaa) => ({ c: 1 }));

	// a comment elsewhere in the call does not change which state wins: destructured heads
	// force the chain layout open, and heads this wide still break every argument out
	obj.method('first' /* c */, ({ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }) => ({ bbbbbbbbbbbbbbbbbbbbbb }) => ({ c: 1 }));
</script>
