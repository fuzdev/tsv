<script lang="ts">
	// Leading comment on the first operand floats before the sequence parens
	fn(((/* c */ x), y));

	// Trailing comment on the last operand floats after the sequence parens
	fn((x, (y /* d */)));

	// Both edges float out of the sequence parens
	fn(((/* c1 */ x), (y /* c2 */)));
</script>
