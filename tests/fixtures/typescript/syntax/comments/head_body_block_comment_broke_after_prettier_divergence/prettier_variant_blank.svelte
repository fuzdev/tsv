<script>
	// If - block comment trailing the anchor, `{` on the line the author gave it
	if (a) /* b */

	{
		fn();
	}

	// While - the same
	while (a) /* b */

	{
		fn();
	}

	// For - the same
	for (;;) /* b */

	{
		fn();
	}

	// For-of - the same
	for (x of y) /* b */

	{
		fn();
	}

	// For-in - the same
	for (x in y) /* b */

	{
		fn();
	}

	// For-await-of - the same
	async function f() {
		for await (const x of y) /* b */

		{
			fn();
		}
	}

	// Do - the same
	do /* b */

	{
		fn();
	} while (a);

	// Else - the same
	if (a) {
	} else /* b */

	{
		fn();
	}

	// A run the author glued keeps its line, and the `{` still drops below it
	if (a) /* b1 */ /* b2 */

	{
		fn();
	}

	// A `{` the author glued to the `*/` stays glued (control)
	if (a) /* b */ {
		fn();
	}

	// Non-block body - no `{`, so the break collapses and the run leads the body
	// inline (control)
	while (a) /* b */ fn();

	// Else-if - no `{` either, so the break collapses there too (control)
	if (a) fn();
	else /* b */ if (b) fn();
</script>
