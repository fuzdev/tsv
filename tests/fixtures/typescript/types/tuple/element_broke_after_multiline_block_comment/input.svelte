<script lang="ts">
	// A multi-line block the author broke after, ahead of a tuple element: the block
	// keeps the break after its `*/`, the element dropping below it. Prettier prints the
	// first element the same way; a later element's block it moves across the comma to
	// trail the previous element.

	// first element
	type A = [
		/* c1
	d1 */
		B,
		C
	];

	// a later element: the block leads its element
	type D = [
		E,
		/* c2
	d2 */
		F
	];

	// an object element
	type G = [
		/* c3
	d3 */
		{ a: 1 },
		H
	];

	// the indentable kind
	type I = [
		/**
		 * c4
		 */
		J,
		K
	];

	// a run: a single-line block glued ahead of the multi-line one
	type L = [
		/* c5 */ /* c6
	d6 */
		M,
		N
	];

	// control: a single-line block the author broke after collapses onto the element
	type O = [/* c7 */ P, Q];

	// control: a multi-line block glued to the element keeps its line
	type R = [
		/* c8
	d8 */ S,
		T
	];
</script>
