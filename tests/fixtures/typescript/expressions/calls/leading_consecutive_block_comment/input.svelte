<script>
	// Multiple inline block comments before first arg stay on one line
	fn(/* c1 */ /* c2 */ a.b.start, /* c1 */ /* c2 */ a.b.end);

	// Single inline block comment before first arg (existing behavior)
	fn(/* comment */ a, b);
</script>
