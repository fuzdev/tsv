<script lang="ts">
	// A comment after the `;` when the deferring statement is the LAST of its body: no
	// following statement leads it, so the body's own end-of-body run is what prints it.

	// Function body.
	function fn() {
		const a = 1;
		// c
		/* b */
	}

	// Bare block.
	{
		const a = 1;
		// d
		/* e */
	}

	// Program body — the last statement of the file.
	const a = 1;
	// g
	/* h */
</script>
