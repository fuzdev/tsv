<script lang="ts">
	// the `//` behind the stripped `(` joins the block comment on the keyword's line, so an
	// author blank between the two has no line left to sit on and is erased
	async function fn() {
		await /* c1 */ // c2
			a;

		new /* c3 */ // c4
			A();

		await /* c5 */ // c6

			// c7
			b;
	}

	switch (x) {
		case /* c8 */ // c9
			y:
			break;
	}

	type K = keyof /* c10 */ (

		// c11
		T
	);
	type R = readonly /* c12 */ (

		// c13
		string[]
	);
	type F = () => /* c14 */ (

		// c15
		T
	);
	type N = keyof /* c19 */ // c20

		// c21
		T;

	// control: a glued run below the `//` keeps a line of its own
	type G = keyof // c16
		/* c17 */ /* c18 */ T;
</script>
