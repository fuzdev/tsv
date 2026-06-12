<script lang="ts">
	// 98 content + 1 tab = 100 visual - stays inline (at boundary)
	const a = items.filter((x) => x.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa).length;

	// 99 content + 1 tab = 101 visual - breaks callback arg, NOT chain (exceeds by 1)
	const b = items.filter(
		(x) => x.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	).length;

	// Same pattern with trailing non-null assertion - still breaks callback arg
	const c = items.filter(
		(x) => x.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	).length!;
</script>
