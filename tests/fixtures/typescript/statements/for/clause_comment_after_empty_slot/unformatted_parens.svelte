<script>
	// The update clause's leading comment, with both earlier slots empty
	for (;; (
	// c1
	a++)) {
		fn();
	}

	// The test clause's leading comment, with the init slot empty
	for (; (
	// c2
	b); c++) {
		fn();
	}

	// An inline block comment before the update stays on the header line
	for (;; (/* c3 */ d++)) fn();
</script>
