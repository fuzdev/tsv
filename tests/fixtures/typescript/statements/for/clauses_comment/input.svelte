<script>
	for (
		// before init
		let i = 0; // inline with init
		// before test
		i < 10; // inline with test
		// before update
		i++ // inline with update
	) {
		a();
	}

	// Block comment variant
	for (/* before init */ let j = 0; /* before test */ j < 10; /* before update */ j++) {
		b();
	}
</script>
