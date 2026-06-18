<script lang="ts">
	// blank line between the source and options args is preserved
	const a = import(
		'./a',

		x
	);

	// a leading blank line before the first arg is trimmed (collapses inline)
	const b = import('./b', y);
</script>
