<script lang="ts">
	// A line comment in the trailing gap of a for-header sequence clause's LAST operand's
	// grouping shell. The shell is RETAINED and the comment stays inside it — deferring it
	// past the header's `;` would carry it out of the construct it was written in.
	for (
		a,
			(
				b // c1
			);
		;
	) {
		fn();
	}

	// A run of two keeps both inside the same shell.
	for (
		a,
			(
				b // c2
				// c3
			);
		;
	) {
		fn();
	}

	// An own-line block comment needs the shell for the same reason.
	for (
		a,
			(
				b
				/* c4 */
			);
		;
	) {
		fn();
	}

	// The update clause answers the same gap the same way.
	for (
		;
		;
		a,
			(
				b // c5
			)
	) {
		fn();
	}

	// A nested sequence operand prints a pair of its own, and the shell collapses into it:
	// ONE pair, opened by the comment, whichever of the two shells the author wrote it in.
	for (
		a,
			(
				b, c // c6
			);
		;
	) {
		fn();
	}

	for (
		a,
			(
				b, c // c7
			);
		;
	) {
		fn();
	}
</script>
