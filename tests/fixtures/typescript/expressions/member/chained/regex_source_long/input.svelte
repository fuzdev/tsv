<script lang="ts">
	// Short regex .source - stays inline (under 100 chars)
	const a = /aaaaaaaaaaaaaaaaaaaaaaaaa/.source + 'b';

	// Long regex .source in assignment - wraps at assignment, .source stays inline
	const b =
		/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/.source +
		'b';

	// Long regex .source exceeding print width - .source breaks to new line
	const c =
		/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/
			.source + 'b';
</script>
