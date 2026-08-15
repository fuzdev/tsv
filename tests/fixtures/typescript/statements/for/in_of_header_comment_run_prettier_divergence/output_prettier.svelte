<script>
	// A run in the binding→keyword gap keeps each comment on its own line; the
	// keyword still takes its continuation indent.
	for (a of /* c2 */ // c1
	arr)
		expr;

	// A run the author glued onto one line keeps that line.
	for (b /* c1 */ /* c2 */ of arr) // c3
		expr;

	// A blank line between two comments in the gap is preserved.
	for (c of /* c2 */ // c1
	arr)
		expr;

	// So is one the author left between the binding and the run's first comment.
	for (m of // c1
	arr)
		expr;

	// A multiline block after a line comment takes its own line.
	for (d of /* c2 // c1
		c3 */
	arr)
		expr;

	// The keyword→iterable gap answers the same way (for-in shares the path).
	for (e in /* c2 */ // c1
	obj)
		expr;

	// So does the iterable→`)` gap.
	for (f of arr) // c1
		/* c2 */
		expr;

	// An own-line comment in the binding→keyword gap keeps its line once another
	// gap's line comment breaks the header.
	for (g of /* c1 */ // c2
	arr)
		expr;

	// The `(`→binding gap is the fourth: a run the author glued to the binding
	// leads it on one line.
	for (/* c1 */ /* c2 */ h of arr) // c3
		expr;

	// A blank line between two own-line comments there is preserved too.
	for (// c1

	// c2
	k of arr) // c3
		expr;
</script>
