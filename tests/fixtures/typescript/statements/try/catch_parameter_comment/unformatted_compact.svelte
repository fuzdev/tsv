<script>
	try {
		a();
	} catch (// before error
		e // inline with error
	) {
		b();
	}

	// Trailing after parameter
	try {
		c();
	} catch (err
		// trailing after error
	) {
		d();
	}
</script>
