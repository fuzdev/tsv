<script>
	// Stranded after-comma block (a newline before the next declarator): stays on the comma line
	let a1 = 1 /* c1 */,
		b1 = 2;

	// Before-comma block plus a stranded after-comma block: each stays on its side of the comma
	let a2 = 1 /* c1 */ /* c2 */,
		b2 = 2;
</script>
