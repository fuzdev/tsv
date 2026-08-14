<script>
	// A pair the author glued onto one line owns no line of its own - the parens
	// still decide their own width.
	!(/* c1 */ /* c2 */ x);

	// A single own-line block owns its line, so the parens open around it.
	!(
		/* c */
		x
	);

	// So does a run the author ended with a line comment.
	!(
		/* c1 */ // c2
		x
	);

	// Two lines the author gave two comments stay two lines.
	!(
		/* c1 */
		/* c2 */
		x
	);

	// A blank line the author left inside the run survives with it.
	!(
		/* c1 */

		/* c2 */
		x
	);

	// The glued run is the same rule at a keyword operator.
	typeof (/* c1 */ /* c2 */ x);
</script>
