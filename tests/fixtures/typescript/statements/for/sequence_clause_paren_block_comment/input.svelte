<script lang="ts">
	// A block comment in the trailing gap of a for-header sequence clause's LAST operand's
	// redundant grouping shell. The shell strips and the comment stays inline before the
	// header's `;`.
	for (a, b /* c1 */; ;) {
		fn();
	}

	// The update clause answers the same gap the same way, up to the header's `)`.
	for (; ; a, b /* c2 */) {
		fn();
	}

	// An earlier operand's shell has the comma gap that follows it as its claimant.
	for (a /* c3 */, b, c /* c4 */; ;) {
		fn();
	}

	// A run of two blocks.
	for (a, b /* c5 */ /* c6 */; ;) {
		fn();
	}

	// A populated header, not only empty clauses.
	for (a, b /* c7 */; a < n; a++) {
		fn();
	}

	// A nested sequence operand's own required pair is not a shell: a comment past its
	// close was never inside one, so it stays behind the pair.
	for (a, (b, c) /* c8 */; ;) {
		fn();
	}
</script>
