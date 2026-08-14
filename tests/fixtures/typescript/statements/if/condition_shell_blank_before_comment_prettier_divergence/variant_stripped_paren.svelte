<script>
	// The blank the author wrote ahead of the comment survives the shell's erasure
	if (
		/* c1 */
		/* c2 */
		x
	) {
		a();
	}

	// The same gap in a while head
	while (
		/* c1 */
		/* c2 */
		x
	) {
		b();
	}

	// The same gap in a do-while tail, which reaches it through its own emitter
	do {
		c();
	} while (
		/* c1 */
		/* c2 */
		x
	);
</script>
