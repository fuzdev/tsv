<script lang="ts">
	// Function composition: call with callback inside preceding arg (member expression callee)
	assert.deepEqual(
		Array.from(timings.entries(), (e) => e[0]),
		['foo', 'foo_2', 'foo_3']
	);

	// Object last arg with call-with-callback preceding
	assert.deepStrictEqual(
		mapRecord({ a: 1, b: 2 }, (v, k) => v + k),
		{ a: '1a', b: '2b' }
	);

	// call with function expression in preceding arg
	x.y(
		foo(function () {}),
		{ a: '1a' }
	);

	// Short enough to stay inline (no callback in args)
	x.y(foo(bar), { a: '1a' });

	// Simple identifier callee with composition - also expands
	fn(
		Array.from(timings.entries(), (e) => e[0]),
		['foo', 'foo_2', 'foo_3']
	);
</script>
