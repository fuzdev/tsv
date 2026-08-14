<script>
	if (
		// before condition
		x // inline with condition
		// trailing after condition
	) {
		a();
	}

	// Block comment variant
	if (
		/* before */ x /* inline */
		/* trailing */
	) {
		b();
	}

	// A glued pair in the trailing gap keeps the one line the author gave it
	if (
		(
			x
		)
		/* c1 */ /* c2 */
	) {
		c();
	}

	// The trailing gap holds no blank line of its own
	if (
		(
			x

		)
		/* c1 */
	) {
		d();
	}

	// Two own-line comments lead the condition, and the gap holds no blank of its own
	if (
		/* c1 */
		(
			/* c2 */
			x
		)
	) {
		e();
	}

	// An authored blank between two leading comments survives
	if (
		/* c1 */

		(
			/* c2 */
			x
		)
	) {
		f();
	}

	// An authored blank above the first trailing comment survives
	if (
		(
			x
		)

		/* c1 */
	) {
		g();
	}

	// An authored blank between two trailing comments survives
	if (
		(
			x
		)
		/* c1 */

		/* c2 */
	) {
		h();
	}

	// Own-line comments in the trailing gap keep their separate lines
	if (
		(
			x
			/* c1 */)
		/* c2 */
		// c3
	) {
		i();
	}
</script>
