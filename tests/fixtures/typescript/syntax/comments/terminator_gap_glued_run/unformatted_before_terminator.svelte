<script lang="ts">
	// A pair the author GLUED onto one line keeps that line in a terminator's
	// content-to-`;` gap too, the same rule every other comment RUN reads
	// (separator_glued_run, trailing_gap_glued_run, body_trailing_glued_run). One gap
	// emitter serves all three positions below, so the question is asked once.

	// Statement terminator.
	function fn1() {
		const a = x
		/* c1 */ /* c2 */;
	}

	// Class member terminator.
	class A {
		b = x
		/* c1 */ /* c2 */;
	}

	// Type member terminator.
	type T = {
		c: C
		/* c1 */ /* c2 */;
		d: D;
	};

	// Control - two lines the author gave two comments stay two lines.
	function fn2() {
		const e = x
		/* c1 */
		/* c2 */;
	}
</script>
