<script lang="ts">
	// Call with callback as non-last arg - Prettier expands
	fn(
		x.map((e) => e[0]),
		['foo']
	);

	// Call with callback as last arg - ALSO expands (callback is in inner call)
	fn(
		'foo',
		x.map((e) => e)
	);

	// Multiple nested callback calls
	fn(
		arr.filter((x) => x > 0).map((x) => x * 2),
		options
	);

	// Three args with nested callback - all expand
	fn(
		a.map((x) => x),
		b,
		c
	);

	// NO callback in inner call - stays inline
	fn('foo', x.map(y));

	// Direct callback (not in inner call) - stays inline
	fn('foo', (e) => e);
</script>
