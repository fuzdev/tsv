<script lang="ts">
	// A block comment inside the grouping shell the parser erased from a `for` header
	// sequence clause's LAST operand, where that operand is ITSELF a sequence. The one
	// pair the operand prints stands in for every shell that collapsed into it, so the
	// comment stays inside that pair.
	for (a, (b, c /* c1 */); ;) {
		fn();
	}

	// The shell may be the one the author wrote around the operand's own pair — the same
	// collapsed run, so the same answer.
	for (a, (b, c /* c2 */); ;) {
		fn();
	}

	// Two shells with a comment each collapse in source order.
	for (a, (b, c /* c3 */ /* c4 */); ;) {
		fn();
	}
</script>
