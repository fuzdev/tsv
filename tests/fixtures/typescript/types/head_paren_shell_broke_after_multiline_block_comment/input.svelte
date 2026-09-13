<script lang="ts">
	// A multi-line block the author broke after, inside a redundant paren shell: the
	// shell strips and the run lays out as the shell-free authoring does — the block
	// keeps the break after its `*/` at every seam.

	// alias `=`
	type A =
		/* c1
	d1 */
		B;

	// annotation `:`
	let c: /* c2
	d2 */
	D;

	// return type
	function e(): /* c3
	d3 */
	F {}

	// parameter annotation
	function g(
		h: /* c4
	d4 */
		I
	) {}

	// first of two type arguments
	type S = Map<
		/* c7
	d7 */
		T,
		U
	>;

	// the indentable kind
	type V =
		/**
		 * c8
		 */
		W;

	// a run: a single-line block glued ahead of the multi-line one
	type X =
		/* c9 */ /* c10
	d10 */
		Y;

	// control: a single-line block the author broke after collapses onto the value
	type Z = /* c11 */ AA;

	// control: a multi-line block glued to the value keeps its line
	type BB = /* c12
	d12 */ CC;
</script>
