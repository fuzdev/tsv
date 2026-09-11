<script lang="ts">
	// An own-line `//` in a function type's `=>`→return gap keeps the line the author gave it;
	// the return type hangs one level in below it.
	type A = () => (
		// c1
		B
	);

	// The constructor type shares the gap.
	type C = new () => (
		// c2
		B
	);

	// ...and so does the abstract constructor type.
	type D = abstract new () =>
		// c3
		B;

	// A run keeps one comment per line, in order.
	type E = () => (
		// c4
		// c5
		B
	);

	// A block ahead of the line comment keeps its own line too.
	type F = () =>
		/* c6 */
		// c7
		B;

	// A block on the `=>` line stays there, and the author blank after it survives.
	type G = () => /* c8 */

		// c9
		B;

	// A hugging union return hangs as a simple type does.
	type H = () => (
		// c10
		{ a: 1 } | null
	);

	type I = () =>
		// c11
		Map<string, number> | null;

	// A function type in a parameter annotation.
	type J = (
		a: () =>
			// c12
			B
	) => void;

	// Control: a comment the author put ON the `=>` line trails it, as before.
	type K = () => (// c13
		B);
</script>
