<script lang="ts">
	// Short - inline
	fn((x) => ({a: x}));

	// Multiline object body - hugged
	fn(({a}) => ({
		b: a,
		c: a,
	}));

	// Block body - hugged
	fn(({a}) => {
		const b = a;
		return {b};
	});

	// Multiple arrow args - breaks (different pattern)
	fn(
		({a}) => ({b: a}),
		({c}) => ({d: c}),
	);

	// Trailing simple arg after multiline - breaks
	fn(
		({a}) => ({
			b: a,
		}),
		opts,
	);

	// Short object body - stays inline
	fn(({a}) => ({b: a, c: a}));

	// Short with trailing arg - stays inline
	fn(({a}) => ({b: a}), opts);
</script>
