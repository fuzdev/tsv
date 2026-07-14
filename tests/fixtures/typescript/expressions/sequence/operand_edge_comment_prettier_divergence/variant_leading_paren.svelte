<script lang="ts">
	// Leading comment glued inside the sequence parens, before the first operand
	fn(/* c */ (x, y));

	// Trailing comment after the sequence parens (matches prettier — both float it out)
	fn((x, y) /* d */);

	// Both edges: leading glued inside, trailing after `)`
	fn(/* c1 */ (x, y) /* c2 */);
</script>
