<script>
	// What a NESTLED pair's text looks like coming out. The pair is one comment
	// (nestled_block_run), so its interior is reindented as one - the join line is the
	// first comment's last line, its `*/`, the second's `/*`, and that comment's first
	// line, all as a single continuation line taking a single leading space.

	// The join line carries content on both sides of the delimiters.
	/** a
	 * z *//** b
	 */
	const a = 1;

	// ... and on neither.
	/**
	 * a
	 *//**
	 * b
	 */
	const b = 1;

	// Nothing but the stars.
	/**
	 *//**
	 */
	const c = 1;

	// A blank `*` line on each side of the join.
	/** a
	 *
	 *//** b
	 *
	 */
	const d = 1;

	// Ragged authored indentation on the join line reindents like any other.
	/** e
	 *//** f
	 */
	const e = 1;

	// Several lines each side, so the join is one continuation among many.
	/** g
	 * l1
	 * l2
	 *//** h
	 * m1
	 * m2
	 */
	const f = 1;

	// A multi-byte character ahead of the join - the spans are byte offsets, the widths
	// are not.
	/** 日本語
	 *//** g
	 */
	const g = 1;

	// Control - the same interiors with one byte of gap stay two comments, each
	// reindented on its own.
	/** a
	 * z */ /** b
	 */
	const h = 1;
</script>
