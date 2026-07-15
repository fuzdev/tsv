<script>
	// Blank line before a glued block comment - preserved; the comment leads the next element
	const a = [
		'aaaa', // x

		/* c */ 'bbbb',
		'cccc'
	];

	// No blank line - the glued comment leads the next element
	const b = [
		'aaaa', // x
		/* c */ 'bbbb',
		'cccc'
	];

	// A newline after the comment unglues it - it takes its own line
	const c = [
		'aaaa', // x

		/* c */
		'bbbb',
		'cccc'
	];

	// Nothing expands the array - it fits on one line, so the blank line collapses
	const d = ['aaaa', /* c */ 'bbbb', 'cccc'];

	// A JSDoc cast owns its comment too - the cast prints it, so the blank line
	// belongs before the comment, not before the `(`
	const e = [
		'aaaa', // x

		/** @type {T} */ ('bbbb'),
		'cccc'
	];
</script>
