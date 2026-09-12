<script lang="ts">
	// a retained shell around a union in an array type: the comment keeps its own line
	// and the parens stay open
	type A = (
		/* c1 */
		B | C
	)[];

	// the same shell over an indexed access
	type D = (
		/* c2 */
		E | F
	)['k'];

	// a multiline block in the same gap
	type G = (
		/* c3
		d */
		H | I
	)[];

	// the pair `keyof` requires, which keeps the run inside it
	type J = keyof (
		/* c4 */
		K | L
	);

	// a redundant shell strips, so the run lands in the `=` gap and hangs there
	type M =
		/* c5 */
		N;

	// the same at an annotation `:`
	let o:
		/* c6 */
		P | Q;

	// only a comment the author isolated on its own line takes one: a glued block
	// stays inline
	type R = /* c7 */ S;

	// and so does one the author merely broke after
	type T = /* c8 */ U;

	// in a run, each comment answers for itself
	type V =
		/* c9 */
		/* c10 */ W;
</script>
