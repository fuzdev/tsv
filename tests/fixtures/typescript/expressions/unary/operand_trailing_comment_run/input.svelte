<script>
	// A pair the author GLUED onto one line keeps that line after the operand.
	!(
		x
		/* c1 */ /* c2 */
	);

	// The glue holds when the run ends in a line comment.
	!(
		x
		/* c1 */ // c2
	);

	// A multiline block ahead of the pair puts it on a line the operand never touched.
	!(
		x /* c1
c2 */ /* c3 */
	);

	// Two lines the author gave two comments stay two lines.
	!(
		x
		/* c1 */
		/* c2 */
	);

	// The same when the run mixes a block with a line comment.
	!(
		x
		/* c1 */
		// c2
	);

	// Two own-line line comments stay two lines - gluing them would make the second
	// one text inside the first.
	!(
		x
		// c1
		// c2
	);

	// An own-line line comment keeps its own line.
	!(
		x
		// c
	);

	// A comment the author glued to the operand trails it.
	!(
		x // c1
		/* c2 */
	);

	// An author blank line above the run is preserved, and one inside it.
	!(
		x

		/* c1 */

		/* c2 */
	);

	// Control - the leading gap already reads each comment's own line.
	!(
		// c1

		/* c2 */ x
	);
</script>
