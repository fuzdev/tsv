<script lang="ts">
	// 100 chars - stays inline
	const x = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbb.map((r) => fn({prop1: r, prop2: dir}));

	// 101 chars - breaks arrow body, not orphan closing paren
	const y = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbbb.map((r) => fn({prop1: r, prop2: dir}));
</script>
