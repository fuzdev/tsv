<script lang="ts">
	// an own-line line comment before the close paren of a parenthesized decorated class
	// expression stays inside the pair: past the `;` it would merge with a trailing comment
	(
		@dec
		class {}
		// c1
	); // c2

	// a line comment glued to the class stays inside the pair too
	(
		@dec
		class {} // c3
	); // c4

	// with a block comment after the pair, the line comment stays inside it as well, ahead
	// of the block, in authored order
	(
		@dec
		class {}
		// c5
	); /* c6 */
</script>
