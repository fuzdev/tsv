<script lang="ts">
	// variable initializer: the break after a multi-line comment is forced by the comment
	// itself, so an author blank after it survives
	const a1 =
		/* x
y */

		b;

	// the indentable flavor
	const a2 =
		/* x
		 * y */

		b;

	// a value too wide for the line
	const a3 =
		/* x
y */

		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb +
		cccccccccccccccccccccccccccccccccc +
		dddddddddddddddddddddddddddd;

	// a conditional value
	const a4 =
		/* x
y */

		aaa ? bbb : ccc;

	// expression-statement assignment
	a5 =
		/* x
y */

		b;

	// variable type annotation: the type drops below the comment, and the blank survives
	let a6: /* x
y */

	B;
	let a7: /* x
y */
	B;

	// a union too wide for the line
	let a8:
		/* x
y */

		| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		| Cccccccccccccccccccccccccccccccccccc
		| Dddddddddddddddddddddddddddd;

	// function return type
	function f1(): /* x
y */

	B {}
	function f2(): /* x
y */
	B {}

	// arrow body and type alias: the same rule
	const a9 = () =>
		/* x
y */

		b;
	type A1 =
		/* x
y */

		B;
</script>
