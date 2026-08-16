<script lang="ts">
	// A line comment in the trailing gap of a ternary ALTERNATE's grouping shell.
	// The shell is retained and the comment stays inside it — ONE pair, the same one
	// the branch's clarity parens supply.
	const a = cond
		? 0
		: (
				b = c // c1
			);

	// A `??` branch takes the same single pair.
	const d = cond
		? 0
		: (
				e ?? f // c2
			);

	// An `as` branch too.
	const g = cond
		? 0
		: (
				h as T // c3
			);

	// A run of two stays inside the one shell.
	const i = cond
		? 0
		: (
				j = k // c4
				// c5
			);

	// A sequence supplies its own pair, so a trailing BLOCK comment stays inside it
	// rather than crossing a `)` the output still prints.
	const l = cond ? 0 : (m, n /* c6 */);
</script>
