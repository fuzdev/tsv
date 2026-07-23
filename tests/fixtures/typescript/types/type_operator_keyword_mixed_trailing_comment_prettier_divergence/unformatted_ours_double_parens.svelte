<script lang="ts">
	// Mixed leading run: a block comment then a line comment before the operand.
	type A = keyof ((/* b */ // c
		B));

	// Trailing: a leading line comment plus a block comment after the operand.
	type C = keyof ((// c
		D /* t */));
</script>
