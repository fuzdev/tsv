<script>
	// Block comment before the comma trails the previous initializer (inline).
	for (let i = 0 /* c1 */, j = 1; i < 9; i++) {}

	// Block comment after the comma leads the next declarator (inline).
	for (let k = 0, /* c2 */ l = 1; k < 9; k++) {}

	// Line comment after the comma forces the declarators onto their own lines.
	for (
		let m = 0, // c3
			n = 1;
		m < 9;
		m++
	) {}

	// Own-line block between declarators leads the next one on its own line.
	for (
		let p = 0,
			/* c4 */
			q = 1;
		p < 9;
		p++
	) {}
</script>
