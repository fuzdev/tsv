<script>
	// Multiple arrow function arguments are always expanded to multiple lines
	// (Prettier always expands 2+ arrow args, regardless of source formatting)
	const entry = contextmenu.add_entry(() => run, () => disabled_prop);

	// Arrow callbacks in standalone call - always expanded
	fn(() => x, () => y);

	// Arrow with expression body
	process((item) => item.value, (item) => item.key);

	// Three arrow args - always expanded
	setup(() => a, () => b, () => c);
</script>
