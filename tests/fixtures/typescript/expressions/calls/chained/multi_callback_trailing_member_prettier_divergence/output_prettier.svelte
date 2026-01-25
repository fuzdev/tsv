<script lang="ts">
	// Multiple callbacks + trailing member: chain breaks, .length stays with last call
	// We put .length on its own line; Prettier keeps it with .map(...)
	const a = items
		.filter((x) => x.aaaaaaaaaaaaaaaaaaaaaa)
		.map((x) => x.bbbbbbbbbbbbbbbbbbbbbb).length;
</script>
