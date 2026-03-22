<script>
	// Consecutive line comments after : (crypto.js case)
	const a = b
		? c
		: // comment 1
			// comment 2
			d;

	// Consecutive line comments after ?
	const e = f
		? // comment 1
			// comment 2
			g
		: h;

	// Three consecutive line comments
	const i = j
		? k
		: // comment 1
			// comment 2
			// comment 3
			l;

	// Mixed: line then block comment after :
	const m = n
		? o
		: // comment 1
			/* comment 2 */
			p;

	// Mixed: block then line comment after :
	const q = r
		? s
		: /* comment 1 */
			// comment 2
			t;
</script>
