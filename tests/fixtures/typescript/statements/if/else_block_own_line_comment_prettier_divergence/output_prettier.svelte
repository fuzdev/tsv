<script>
	// Own-line block comment - we preserve position, prettier moves inside else
	if (a) {
		fn1();
	} else {
	/* own line */
		fn2();
	}

	// With blank line before comment
	if (a) {
		fn1();
	} else {

	/* blank before */
		fn2();
	}
</script>
