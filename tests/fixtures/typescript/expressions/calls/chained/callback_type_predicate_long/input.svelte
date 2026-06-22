<script lang="ts">
	// 100 chars at 1-tab: entire chain stays on one line
	const a = items.filter((item): item is A & { x: B } => Boolean(item.xxxxxxx)).map((item) => item);

	// 101 chars at 1-tab: chain breaks to multiple lines
	const b = items
		.filter((item): item is A & { x: B } => Boolean(item.xxxxxxxx))
		.map((item) => item);

	// 100 chars at 2-tab: filter callback stays on one line
	const c = items
		.filter((item): item is A & { data: { values: Array<unk> } } => Boolean(item.data.values?.leng))
		.map((item) => item);

	// 101 chars at 2-tab: arrow body breaks to new line
	const d = items
		.filter((item): item is A & { data: { values: Array<unk> } } =>
			Boolean(item.data.values?.lengt)
		)
		.map((item) => item);
</script>
