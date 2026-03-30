<script>
	// Short - fits inline, no break needed
	a.map(/* comment */ (x) => fn(x, y));

	// Inline at 100 - stays on one line
	a.b.c.map(/* comment */ (source) => get_relative_aaaaaaaaaaaaaaaaaaaaaaaaa(source_aaaaa, source));

	// Long at 101 - leading comment blocks expand-last-arg, breaks at open paren
	a.b.c.map(
		/* comment */ (source) => get_relative_aaaaaaaaaaaaaaaaaaaaaaaaa(source_aaaaaa, source),
	);
</script>
