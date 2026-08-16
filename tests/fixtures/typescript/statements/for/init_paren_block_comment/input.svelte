<script lang="ts">
	// A block comment in the trailing gap of a for-header init declarator's redundant
	// grouping shell. The shell strips and the comment stays inline before the
	// header's `;`.
	for (let i = a /* c1 */; ;) {
		fn();
	}

	// A sequence keeps its own required parens; the comment stays inside them.
	for (let i = (a, b /* c2 */); ;) {
		fn();
	}

	// Any declarator index.
	for (let i = 0, j = b /* c3 */; ;) {
		fn();
	}

	// A run of two blocks.
	for (let i = a /* c4 */ /* c5 */; ;) {
		fn();
	}

	// A populated header, not only empty clauses.
	for (let i = a /* c6 */; i < n; i++) {
		fn();
	}
</script>
