<script lang="ts">
	// A trailing run's blank-line scan reads the SOURCE, so it steps over the bytes of a
	// comment another emitter printed. A multiline block holding a blank line of its own
	// must not hand those newlines to the scan as an author blank.

	// Array literal - the block trails the element, the line comment ends the array.
	const arr = [
		1 /* c1

c2 */,
		// c3
	];

	// Call argument - the block trails the argument, the line comment dangles below it.
	fn(
		a /* c1

c2 */,
		// c3
	);

	// New expression.
	new A(
		a /* c1

c2 */,
		// c3
	);

	// Object literal - the same scan at an end-of-body run.
	const obj = {
		a: 1 /* c1

c2 */,
		// c3
	};

	// Control - a blank line the author DID write is preserved.
	const arr2 = [
		1 /* c1 */

		// c2
	];
</script>
