<script>
	// Short first arg, long array - first arg stays inline with `[`
	const a = fn('x', [
		Item000000000000000000000,
		Item111111111111111111111,
		Item222222222222222222222
	]);

	// Short first arg, long object - first arg stays inline with `{`
	const b = fn('y', {
		property00000000000000000000000000: 1,
		property11111111111111111111111111: 2,
		property22222222222222222222222222: 3
	});

	// Short - stays inline (under width)
	const c = fn('x', [A, B]);

	// Short object - stays inline
	const d = fn('y', { a: 1, b: 2 });

	// Multiple short args, long array - all stay inline with `[`
	const e = fn('x', 'y', [
		Item000000000000000000000,
		Item111111111111111111111,
		Item222222222222222222222
	]);
</script>
