<script lang="ts">
	// An own-line comment before a statement's `;` drops to its own line after it,
	// and a block comment glued to the next statement stays with that statement —
	// even when that statement's own parens are the ones the comment is glued to.
	const a = 1;
	// c
	/* b */ ({});

	// The same gap, with a call statement ahead of it.
	fn();
	// d
	/* e */ ({});

	// Control: a next statement that needs no parens.
	fn();
	// g
	/* h */ fn();
</script>
