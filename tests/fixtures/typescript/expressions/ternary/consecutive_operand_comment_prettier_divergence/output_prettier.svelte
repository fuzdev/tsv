<script lang="ts">
	// Two line comments between the test and `?`: the first trails the test, the
	// second drops to its own line aligned with `?`. A trailing comment on the
	// consequent (`// c3`) stays put. Neither merges nor reverses.
	const a = cond // c1
		? // c2
			x // c3
		: y;

	// Three line comments between the test and `?`: each after the first gets its
	// own line.
	const b = cond // c1
		? // c2
			// c3
			x
		: y;

	// Two line comments between the consequent and `:`: the first trails the
	// consequent, the second drops to its own line aligned with `:`.
	const c = cond
		? x // c1
		: // c2
			y;

	// Boundary (a MATCH, not a divergence): a block + line comment authored on the
	// test's own line both stay trailing — only later-line comments move. A `//`
	// ends its line, so it is always last in a same-line run.
	const d = cond /* c1 */ // c2
		? x
		: y;
</script>
