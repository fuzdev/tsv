<script>
	// A block comment leading the parameter, glued to it
	try {
		a();
	} catch (
		/* c1 */
		e
	) {
		b();
	}

	// A trailing block on the parameter's line
	try {
		c();
	} catch (e /* c2 */) {
		d();
	}
</script>
