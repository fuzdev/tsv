<script lang="ts">
	// A block comment between a prefix update's operand and its `)` stays inside the parens.
	++(a as T /* c1 */);

	// Parens that would otherwise be redundant are retained by the comment.
	++(b /* c2 */);

	// `--` reaches the same gap.
	--(c as T /* c3 */);

	// A line comment cannot trail before the `)`, so the shell expands around it.
	++(
		d
		// c4
	);

	// Both of the shell's gaps, each emitted where it was written.
	++(/* c5 */ e as T /* c6 */);

	// No comment: a required pair stays, a redundant one is stripped.
	++(f as T);
	++g;
</script>
