<script lang="ts">
	// variable initializer: a multi-line comment glued to the value prints outside the
	// value's own group, so the value keeps its own layout
	const a1 = /* x */
	/* y
	 */ aaa ? bbb : ccc;

	// a preserved multi-line comment: same rule
	const a2 = /* x */
	/* y
z */ aaa ? bbb : ccc;

	// the break between the comments carries an author blank
	const a3 = /* x */

	/* y
	 */ aaa ? bbb : ccc;

	// a value that breaks on its own
	const a4 = /* x */
	/* y
	 */ {
		aaa: 1
	};

	// expression-statement assignment: same rule
	let a5;
	a5 = /* x */
	/* y
	 */ aaa ? bbb : ccc;

	// a line comment on the operator's line: the value still keeps its own layout
	const a11 = // c
	/* y
	 */ aaa ? bbb : ccc;
	let a12;
	a12 = // c
	/* y
	 */ aaa ? bbb : ccc;

	// arrow body: same rule
	const a6 = () => /* x */
	/* y
	 */ aaa ? bbb : ccc;

	// arrow body with an author blank between the comments
	const a7 = () => /* x */

	/* y
	 */ aaa ? bbb : ccc;

	// an arrow body that is not a ternary: same rule
	const a13 = () => /* x */
	/* y
	 */ aaa + bbb;

	// the same arrow as a call argument
	fn((x) => /* x */
	/* y
	 */ aaa ? bbb : ccc);

	// the same arrow as a `new` argument
	new C1((x) => /* x */
	/* y
	 */ aaa ? bbb : ccc);

	// the same arrow as a member-chain argument
	a.b.m1((x) => /* x */
	/* y
	 */ aaa ? bbb : ccc);

	// the same arrow as the last of several arguments
	fn(a, (x) => /* x */
	/* y
	 */ aaa ? bbb : ccc);
	new C2(a, (x) => /* x */
	/* y
	 */ aaa ? bbb : ccc);
	a.b.m2(a, (x) => /* x */
	/* y
	 */ aaa ? bbb : ccc);

	// spread argument: the first comment keeps the dots' line
	const a8 = [.../* x */
	/* y
	 */ fn(aaa, bbb)];

	// object property value
	const a9 = {
		k:
			/* x */
			/* y
			 */ aaa ? bbb : ccc
	};

	// class property
	class A {
		b =
			/* x */
			/* y
			 */ aaa ? bbb : ccc;
	}

	// class property with a line comment on the operator's line, and on a line of its own
	class B {
		c1 = // x
			/* y
			 */ aaa ? bbb : ccc;
		c2 =
			// x
			/* y
			 */ aaa ? bbb : ccc;
	}

	// variable type annotation: the first comment keeps the head line
	let a10: /* x */
	/* y
	 */ B;

	// function return type: same as the annotation
	function f2(): /* x */
	/* y
	 */ B {}

	// type alias: the break between the comments carries an author blank
	type A1 = /* x */

	/* y
	 */ B;
</script>
