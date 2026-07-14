<script>
	// A leading comment inside a ternary operand's grouping parens survives when the
	// parens are stripped and re-added as clarity parens around `??` — in the TEST
	// position as well as in the branches.
	//
	// The comment lands OUTSIDE the re-added parens (`/* c */ (x ?? y)`, not
	// `(/* c */ x ?? y)`), which is what every other operand position already does —
	// branches, member base, `||` operand, sequence — and what prettier does. The
	// authored position is inside them, so this is a relocation across a paren
	// boundary; it is pinned here as the established behavior, not endorsed. Whether
	// tsv should instead keep such a comment glued to the operand it leads is the same
	// question the JSDoc-cast rule answers "yes" to for a *semantic* paren, and is
	// tracked separately — see conformance_prettier.md §Comment Position Philosophy.
	const a = /* c */ (x ?? y) ? p : q;
	const b = cond ? /* c */ (x ?? y) : q;
	const c = cond ? p : /* c */ (x ?? y);

	// every operand at once
	const d = /* c1 */ (x ?? y) ? /* c2 */ (p ?? q) : /* c3 */ (r ?? s);
</script>
