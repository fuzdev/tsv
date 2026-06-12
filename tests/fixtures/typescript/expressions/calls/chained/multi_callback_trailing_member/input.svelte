<script lang="ts">
	// Multiple callbacks + trailing member: chain breaks, .length stays with last call
	const a = items
		.filter((x) => x.aaaaaaaaaaaaaaaaaaaaaa)
		.map((x) => x.bbbbbbbbbbbbbbbbbbbbbb).length;
</script>
