<script>
	// line comment after init clause
	for (
		a; // after init
		b;
		c
	) {
		d();
	}

	// line comment after test clause
	for (
		a;
		b; // after test
		c
	) {
		d();
	}

	// line comment after update clause
	for (
		a;
		b;
		c // after update
	) {
		d();
	}
</script>
