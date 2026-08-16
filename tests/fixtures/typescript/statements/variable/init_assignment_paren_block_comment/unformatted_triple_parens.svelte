<script lang="ts">
	// A block comment trailing a parenthesized assignment operand in a value position stays
	// INSIDE the parens: the value position re-adds that pair, so the comment would have to
	// cross a `)` the output still prints to reach the statement's `;`.
	const a = (((b = c)) /* t */);

	// Every assignment flavour answers alike.
	const d = (((e += f)) /* t */);

	const g = ((([h] = i)) /* t */);

	const j = (((k = l = m)) /* t */);

	// A run collapses in source order, still inside.
	const n = (((o = p)) /* t1 */ /* t2 */);

	// A non-last declarator has no terminator to defer past, and already answered so.
	const q = (((r = s)) /* t */), u = 1;
</script>
