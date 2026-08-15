<script>
	// The continuation keyword hugs a block comment trailing the `}`, whatever line
	// the author gave it
	if (a) {
		fn1();
	} /* b1 */ else {
		fn2();
	}

	// Same gap, a do-while's `while`
	do {
		fn1();
	} /* b2 */
	while (a);
</script>
