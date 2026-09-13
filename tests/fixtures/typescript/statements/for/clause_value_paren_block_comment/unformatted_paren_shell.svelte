<script lang="ts">
	// A block comment in the trailing gap of a for-header CLAUSE-TERMINAL value's redundant
	// grouping shell — here the init clause's assignment RHS. The shell strips and the
	// comment stays inline before the header's `;`, which SEPARATES clauses rather than
	// terminating a statement.
	for (a = (b /* c1 */); ;) {}

	// A compound assignment is the same host.
	for (a += (b /* c2 */); ;) {}

	// A chain: the innermost `=` owns the shell, and it ends where the clause does.
	for (a = b = (c /* c3 */); ;) {}

	// A destructuring target changes nothing.
	for ([a] = (b /* c4 */); ;) {}

	// The `[~In]` pair is the printer's own, so the comment lands outside it.
	for (a = ('k' in o /* c5 */); ;) {}

	// A sequence clause's LAST operand is an assignment: the shell is that operand's RHS.
	for (fff, a = (b /* c6 */); ;) {}

	// An earlier operand's shell has the comma gap behind it as its claimant.
	for (a = (b /* c7 */), c; ;) {}

	// The update clause runs to the header's `)`.
	for (; ; a = (b /* c8 */)) {}

	// A ternary alternate ends at the clause too, the other value host.
	for (a ? b : (c /* c9 */); ;) {}

	// So does an arrow body's assignment; the body's clarity parens are the printer's and
	// the comment stays inside them.
	for (a = () => x = (b /* c10 */); ;) {}

	// A run of two blocks.
	for (a = (b /* c11 */ /* c12 */); ;) {}

	// A populated header, not only empty clauses.
	for (a = (b /* c13 */); a < n; a++) {}
</script>
