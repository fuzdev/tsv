<script lang="ts">
	// An own-line comment inside the grouping shell the parser erases ahead of a for-header
	// sequence clause's FIRST operand. The shell is gone, so the comment leads the clause.
	for (
		// c1
		(a in b),
			c;
		;
	) {
		fn();
	}

	// A block comment answers the same way.
	for (
		/* c2 */
		(a in b),
			c;
		;
	) {
		fn();
	}

	// The update clause reads the same gap.
	for (
		;
		;
		// c3
		a,
			c
	) {
		fn();
	}
</script>
