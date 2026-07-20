<script>
	// Own-line comment before catch/finally: keeps its own line
	try {
		a()}
	// c1
	catch (e) {
		b()}
	// c2
	finally {
		c();
	}

	// Own-line block comment: also keeps its own line
	try {
		a()}
	/* c3 */
	catch (e) {
		b();
	}

	// Trailing line comment: stays trailing, `catch` drops to the next line
	try {
		a()}// c4
	catch (e) {
		b();
	}

	// Trailing block comment: `catch` stays on the line
	try {
		a()}/* c5 */catch (e) {
		b();
	}

	// A blank line above an own-line comment is preserved
	try {
		a()}


	// c6
	catch (e) {
		b();
	}

	// Mixed: a trailing comment keeps the brace line, an own-line one follows it
	try {
		a()}// c7
	// c8
	catch (e) {
		b();
	}
</script>
