<script lang="ts">
	// expandable object, long source - outer import breaks
	const a = await import('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', {
		with: {type: 'json'},
	});

	// expandable object + block comment, long source
	const b = await import(
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		{with: {type: 'json'}} /* comment */
	);

	// single arg + block comment, long call arg
	const c = await import(
		fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa') /* comment */
	);

	// short controls - everything fits
	const d = await import('./a', {with: {type: 'json'}});
	const e = await import('./a', {with: {type: 'json'}} /* comment */);
	const f = await import(fn('./a') /* comment */);
</script>
