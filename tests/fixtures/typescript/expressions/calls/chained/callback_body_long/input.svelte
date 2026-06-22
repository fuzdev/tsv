<script lang="ts">
	// ours: 100 chars on callback line (at boundary)
	const x = items
		.map(
			(a) => 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
		)
		.join(';');

	// ours: 101 chars on callback line (exceeds by 1)
	const y = items
		.map(
			(a) =>
				'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
		)
		.join(';');
</script>
