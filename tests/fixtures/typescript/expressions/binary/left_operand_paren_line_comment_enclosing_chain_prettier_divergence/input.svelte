<script lang="ts">
	// A line comment inside the grouping parens around the leftmost operand of an inner
	// chain whose enclosing chain prints no pair around it: the comment leads the whole
	// value and breaks only its own line, never the enclosing chain.
	// c1
	a >> b < c;

	x =
		// c2
		a >> b < c;

	const y =
		// c3
		a >> b < c;

	function f1() {
		return (
			// c4
			a + b < c
		);
	}

	f(
		g(
			// c5
			a >> b < c
		)
	);

	o = {
		k:
			// c6
			a >> b < c
	};

	// Through a call, a member and a tagged-template base.
	// c7
	a() < b >> c;

	// c8
	a.b < c >> d;

	// c9
	a`t` < b >> c;

	// An own-line block breaks its line the same way.
	/* c10 */
	a >> b < c;

	// A multi-line block breaks its line the same way.
	/* c13
	 */
	a >> b < c;

	// A glued single-line block breaks nothing: it stays inline.
	/* c11 */ a() < b >> c;

	// A statement header's test: the chain prints in the header's own group, and breaks in both
	// formatters.
	if (
		// c12
		a >> b <
		c
	) {
	}
</script>
