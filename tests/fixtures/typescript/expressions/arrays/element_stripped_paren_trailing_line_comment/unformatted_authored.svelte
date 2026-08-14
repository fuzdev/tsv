<script>
	// a line comment glued to a stripped paren shell's closer trails the element
	const a = [(
x
)// c
, y];

	// a block before it keeps its side of the comma while the line comment defers past it
	const b = [(
x
)/* c1 */ // c2
, y];

	// on the last element, whose comma is never emitted, the line comment trails it
	const c = [x, (
y
)// c
];

	// a line comment that leads its own line leads the next element instead
	const d = [x
// c
, y];

	// the same behind a shell whose closer takes its own line — the `)` and the
	// comment sit on two lines the element's span excludes, and those are not a blank
	const e = [(
x
)
// c
, y];
</script>
