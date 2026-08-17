<script>
	// A `//` the author glued to the comment-holder `(` keeps that line
	const a = !(
		// c1
		x || y
	);

	// The keyword operators answer the same way
	const b = typeof (
		// c2
		x || y
	);

	const c = void (
		// c3
		x || y
	);

	// So does a sign operator, whose parens the operand requires
	const d = -(
		// c4
		x + y
	);

	// Only the glued comment claims the line - the rest of the run keeps its own
	const e = !(
		// c5
		// c6
		x || y
	);

	// A block glued to the `(` claims nothing, so the whole run stays below it
	const f = !(
		/* c7 */ // c8
		x || y
	);

	// An author blank below the glued comment survives it
	const g = !(
		// c9

		x || y
	);

	// Control - written on its own line the comment keeps its own line, and
	// prettier agrees
	const h = !(
		// c10
		x || y
	);

	// Control - a run of blocks the author glued owns no line, so the parens
	// still decide their own width
	const i = !(/* c11 */ /* c12 */ x);

	// The comment-holder's gap opens at the OPERATOR, so the author may put the `(` on a
	// line of its own; the break it takes there is structure, and no blank is invented
	const j = !!(
		// c13
		x || y
	);

	// The same, from an authoring that also left a blank BELOW that `(` — that one sits
	// against the delimiter and stays erased
	const k = !!(
		// c14
		x || y
	);

	// A blank the author wrote between the comment and the `(` is authorship and rides
	const l = !!(
		// c15

		x || y
	);
</script>
