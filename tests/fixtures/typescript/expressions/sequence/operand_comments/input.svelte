<script lang="ts">
	// Trailing block comment on a sequence operand
	fn((a /* t1 */, b));

	// Leading block comment on a sequence operand
	fn((a, /* l2 */ b));

	// Middle operand carries both a leading and a trailing comment
	fn((a, /* l2 */ b /* t2 */, c));

	// Interior comments across every gap between operands
	fn((a /* t1 */, /* l2 */ b /* t2 */, /* l3 */ c));
</script>
