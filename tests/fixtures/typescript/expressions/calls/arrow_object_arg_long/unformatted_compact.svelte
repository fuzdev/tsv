<script lang="ts">
	// 100 chars - stays inline
	const x = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbb.map((r) => fn({prop1: r, prop2: dir}));

	// 101 chars - breaks arrow body, not orphan closing paren
	const y = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbb.map((r) => fn({prop1: r, prop2: dir}));
</script>
