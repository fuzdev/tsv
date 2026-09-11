<script lang="ts">
	// A block comment trailing the keyword whose break is forced — by a line comment below it, or
	// by the comment itself spanning lines — keeps the author blank after it
	async function f1() {
		new /* x */

			// y
			C1();

		new /* x
		y */

			C2();

		await /* x */

			// y
			b;

		await /* x
		y */

			b;
	}

	// switch label `case`→test
	switch (a) {
		case /* x */

			// y
			b:
		case /* x
		y */

			c:
	}

	// `satisfies`
	x2 = a satisfies /* x
	y */

		B;

	// prefix type operators, type query and `infer`
	type T3 = keyof /* x */

		// y
		B;
	type T4 = typeof /* x
	y */

		b;
	type T5 = A extends infer /* x */

		// y
		U
		? U
		: never;

	// function type return type
	type T6 = () => /* x
	y */

		B;

	// `export default`
	export default /* x */

		// y
		a;
</script>
