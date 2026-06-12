<script>
	// Block body — comment preserved between `for` and `(`
	for /* k */ (a; b; c) {
		d();
	}

	// Empty body — attaches directly: `);` (no space before the semicolon)
	for /* k */ (a; b; c);

	// Non-block body — stays inline
	for /* k */ (a; b; c) d();
</script>
