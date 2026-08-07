<script>
	// a block comment past the last element takes its own line before the closer
	[
		a,
		b
		/* c */
	] = x;

	// a line comment in the same position
	[
		a,
		b
		// c
	] = x;
</script>
