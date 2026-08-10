<script lang="ts">
	// A pair the author GLUED onto one line keeps that line wherever a comment RUN is
	// emitted, not only at a container's end (body_trailing_glued_run,
	// trailing_gap_glued_run). The separator between two comments is one question.

	// Export specifier list, after the last item.
	const a = 1;
	export {
		a
		/* c1 */ /* c2 */
	};

	// Own-line blocks a spread's stripped grouping parens left behind.
	fn(
		...b
		/* c1 */ /* c2 */
	);

	// Binary chain, after the operator.
	const c =
		aaaa +
		/* c1 */ /* c2 */
		// c3
		bbbb;

	// Ternary branch, after the `?`.
	const d = cond
		? /* c1 */ /* c2 */
			// c3
			e
		: f;

	// Type alias value, after the `=`.
	type T =
		/* c1 */ /* c2 */
		// c3
		A;

	// Leading run before a non-first parameter.
	function fn2(
		g,
		/* c1 */ /* c2 */
		// c3
		h
	) {}

	// Array element - a last spread's stripped-paren interior, past the `]` scan.
	const i = [
		...j
		/* c1 */ /* c2 */
	];

	// Control - two lines the author gave two comments stay two lines.
	type U =
		/* c1 */
		/* c2 */
		// c3
		B;
</script>
