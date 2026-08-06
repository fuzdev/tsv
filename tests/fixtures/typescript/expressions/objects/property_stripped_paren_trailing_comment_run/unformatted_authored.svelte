<script>
	// a block comment glued to a stripped paren shell's closer trails the property
	const a = {a1: (
1
)/* c */, b1: 2};

	// a line comment in the same position defers past the comma
	const b = {a2: (
1
)// c
, b2: 2};

	// a block then a line comment each keep the side of the comma the author gave them
	const c = {a3: (
1
)/* c1 */ // c2
, b3: 2};

	// on the last property, whose comma is never emitted, the run trails it
	const d = {a4: 1, b4: (
2
)/* c */};

	// a line comment there defers past the property the same way
	const e = {a5: 1, b5: (
2
)// c
};

	// an own-line block comment with nothing before it on its line leads the next property
	const f = {a6: 1
/* c */, b6: 2};
</script>
