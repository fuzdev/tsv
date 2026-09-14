<script>
	// An indentable block comment the author broke after, FIRST in a ternary branch run that
	// a `//` forces open, re-indents its continuation lines to the branch's indent
	const x1 = a
		? /* y
			 */
			// c
			b
		: c;

	// the `:` branch, a JSDoc-style block
	const x2 = a
		? b
		: /** y
			 * z
			 */
			// c
			d;

	// later in the run
	const x3 = a
		? // c
			/* y
			 */
			b
		: c;

	// a nested conditional's branch, one level deeper
	const x4 = a
		? b
		: c
			? /* y
				 */
				// c
				d
			: e;

	// control: no line comment, the block alone breaks the branch
	const x5 = a
		? /* y
			 */
			b
		: c;
</script>
