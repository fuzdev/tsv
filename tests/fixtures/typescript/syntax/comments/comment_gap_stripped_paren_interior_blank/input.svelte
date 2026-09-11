<script lang="ts">
	// variable initializer: an own-line comment run, then parens that are stripped — a
	// blank line inside them goes with the parens
	const a1 =
		/* x */
		/* y */
		b;

	// a glued pair on its own line
	const a2 =
		/* x */
		/* y */
		/* z */ /* w */
		b;

	// a value that breaks on its own
	const a3 =
		/* x */
		{
			aaa: 1
		};

	// expression-statement assignment
	a4 =
		/* x */
		{
			aaa: 1
		};

	// arrow body
	const a5 = () =>
		/* x */
		/* y */
		b;

	// the same arrow as a call argument, a `new` argument and the last of several arguments
	fn1(
		(x) =>
			/* x */
			/* y */
			b
	);
	new C1(
		(x) =>
			/* x */
			/* y */
			b
	);
	fn2(
		a,
		(x) =>
			/* x */
			/* y */
			b
	);

	// spread argument
	const a6 = [
		.../* x */
		{
			aaa: 1
		}
	];

	// call argument: the comment stays glued to the argument
	fn3(/* x */ b);
	fn4(/* x */ /* y */ b);
	fn5(/* x */ b, c);
	new C2(/* x */ b);
	a.b.m1(/* x */ b);

	// an author blank between the comments survives
	fn6(
		/* x */

		/* y */ /* z */
		b
	);

	// unary operand, `if` condition, template interpolation
	a7 = !(/* x */ b);
	if (/* x */ b) {
	}
	a8 = `${/* x */ b}`;

	// conditional branch: a comment glued to the parens
	a9 = a ? /* x */ b : c;

	// binary operand after a line comment
	a10 =
		a + // x
		/* y */ /* z */
		b;
</script>
