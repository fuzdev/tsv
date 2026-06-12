<script lang="ts">
	// Leading comment on the first operand floats before the sequence parens
	const a = /* c */ (x, y);

	// Trailing comment on the last operand stays before the semicolon
	const b = (x, y) /* c */;

	// Both edges float; the trailing one stays before the semicolon
	const c = /* c1 */ (x, y) /* c2 */;
</script>
