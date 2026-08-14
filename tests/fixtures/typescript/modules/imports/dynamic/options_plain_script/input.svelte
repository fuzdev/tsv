<script>
	// Single argument
	const a = import('./a');

	// Identifier as the second argument
	const expr = { with: { type: 'json' } };
	const b = import('./b', expr);

	// Object literal as the second argument
	const c = import('./c', { with: { type: 'json' } });

	// With await
	const d = await import('./d', expr);
</script>
