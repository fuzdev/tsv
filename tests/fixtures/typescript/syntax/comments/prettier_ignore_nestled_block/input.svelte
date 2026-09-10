<script>
	// A `prettier-ignore` that NESTLES freezes nothing, and the pair still prints welded.
	// The pair is ONE comment (nestled_block_run) whose text is no longer the directive
	// alone; independently, nestling glues a neighbour to one end of the directive, and a
	// directive only honors from its own line. Both readings agree, and so does prettier,
	// whose own merge loses the directive text the same way — so there is no spelling in
	// which merging changes what tsv freezes.

	// The directive opens the pair.
	/* prettier-ignore
	 *//** c
	 */
	const a = [1, 2];

	// The directive closes it.
	/** c
	 *//* prettier-ignore
	 */
	const b = [1, 2];

	// Control - the same multi-line directive alone still freezes its statement.
	/* prettier-ignore
	 */
	const c = [1,   2];

	// Control - a newline between the two, so nothing merges, nothing is glued, and the
	// directive freezes again. Adjacency is the whole difference.
	/** c
	 */
	/* prettier-ignore
	 */
	const d = [1,   2];
</script>
