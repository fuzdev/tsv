<script lang="ts">
	// Block body
	fn(
		(
			a: string // c1
		) => {}
	);

	// Call body - the forced break in the signature expands the call too
	fn(
		(
			a: string // c2
		) => call(a)
	);

	// Object body
	fn(
		(
			a: string // c3
		) => ({ a })
	);

	// Ternary body - keeps its parens
	fn(
		(
			a: string // c4
		) => (a ? a : a)
	);

	// A `new` expression's arguments take the same path
	new Comp(
		(
			a: string // c5
		) => call(a)
	);

	new Comp(
		(
			a: string // c6
		) => (a ? a : a)
	);
	// A preceding argument does not restore the hug
	fn(
		x,
		(
			a: string // c7
		) => call(a)
	);
</script>
