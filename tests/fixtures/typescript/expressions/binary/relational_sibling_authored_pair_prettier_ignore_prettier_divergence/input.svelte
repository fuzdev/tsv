<script lang="ts">
	// A frozen paired element prints verbatim, its pair included
	fn(
		// prettier-ignore
		(a  <  b),
		c > (d, e)
	);
	const a1 = [
		// prettier-ignore
		(a  <  b),
		c > (d, e)
	];
	for (
		a,
			// prettier-ignore
			(x  <  y),
			c > (d, e);
		;
	) {}
	// The authored pair is also the one a bare `in` under a `for` header needs: printed once
	for (
		a,
			// prettier-ignore
			(x < y || p in q),
			c > (d, e);
		;
	) {}
</script>
