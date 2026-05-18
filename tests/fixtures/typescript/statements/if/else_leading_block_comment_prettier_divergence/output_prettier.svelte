<script>
	// Block comment leading else if - comment on own line before keyword
	if (cond) {
		fn1();
	} else if (cond) {
	/* b */
		fn2();
	} else {
	/* c */
		fn3();
	}

	// With blank line before comment
	if (cond) {
		fn1();
	} else {

	/* b */
		fn2();
	}

	// Non-block consequent - comment preserved before else
	if (cond) a();
	/* b */ else if (cond) a();
	/* c */ else a();
</script>
