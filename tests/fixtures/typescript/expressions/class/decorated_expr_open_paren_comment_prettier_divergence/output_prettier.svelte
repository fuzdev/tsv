<script lang="ts">
	// own-line line comment after the open paren of a bare parenthesized decorated class expression
	// c
	(
		@decorator
		class {}
	);

	// own-line block comment after the open paren — same divergence
	/* d */
	(
		@decorator
		class {}
	);
</script>
