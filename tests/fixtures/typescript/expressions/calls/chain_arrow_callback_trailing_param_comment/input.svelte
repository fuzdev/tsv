<script lang="ts">
	// Block body
	arr.map(
		(
			a: string // c1
		) => {}
	);

	// Call body - the forced break in the signature expands the call too
	arr.map(
		(
			a: string // c2
		) => call(a)
	);

	// Ternary body - keeps its parens
	arr.map(
		(
			a: string // c3
		) => (a ? a : a)
	);
	// A preceding argument does not restore the hug
	arr.reduce(
		x,
		(
			a: string // c4
		) => call(a)
	);
</script>
