<script>
	// Block comment leading else if - comment on own line before keyword
	if (cond) {
		fn1();
	}
	/* b */
	else if (cond) {
		fn2();
	}
	/* c */
	else {
		fn3();
	}

	// With blank line before comment
	if (cond) {
		fn1();
	}

	/* b */
	else {
		fn2();
	}

	// Non-block consequent - comment on own line before else
	if (cond) a();
	/* b */
	else if (cond) a();
	/* c */
	else a();
</script>
