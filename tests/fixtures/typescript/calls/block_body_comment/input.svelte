<script>
	// Single-arg block arrow with comment between => and {
	fn1((x) => /* comment */ {
		return x;
	});

	// Single-arg block arrow with long comment
	fn1((x) => /* comment_long_enough_to_force_break_aaaaaaa */ {
		return x;
	});

	// Multi-arg, last arg is block arrow with comment
	fn1(a, (x) => /* comment */ {
		return x;
	});
</script>
