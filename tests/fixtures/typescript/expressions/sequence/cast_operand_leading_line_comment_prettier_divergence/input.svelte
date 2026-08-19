<script lang="ts">
	// A `//` glued to the pair's `(` keeps the `(` line and the pair expands.
	const a = ( // c
		x, y
	) as A;

	// An own-line comment keeps its own line inside the pair.
	const b = (
		// c
		x, y
	) as A;

	// `satisfies` takes the same shell.
	const c = ( // c
		x, y
	) satisfies A;
</script>
