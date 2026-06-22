<script lang="ts">
	// 100 chars at 1-tab: all args stay inline
	const a = fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(arg, items, {
		x
	});

	// 101 chars at 1-tab: trailing object expands
	const b = fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(arg, items, {
		x
	});

	// 100 chars on opening line with {: object still expanded
	const c = fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(arg, items, {
		x
	});

	// 101 chars on opening line with {: all args break
	const d = fnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn(
		arg,
		items,
		{ x }
	);
</script>
