<script lang="ts">
	// state 0: flat at exactly 100 chars
	const a = await import('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', {with: {type: 'json'}});

	// state 1: flat at 101 chars -> expand-last
	const b = await import('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', {
		with: {type: 'json'},
	});

	// state 1: expand-last first line at exactly 100 chars
	const c = await import('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', {
		with: {type: 'json'},
	});

	// state 2: expand-last first line at 101 -> expand-all
	const d = await import(
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		{with: {type: 'json'}}
	);
</script>
