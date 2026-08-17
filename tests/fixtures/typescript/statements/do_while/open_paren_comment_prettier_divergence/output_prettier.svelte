<script>
	do {
		a();
	} while (x); // after open paren

	do {
		a();
	} /* after open paren */ while (x);

	do {
		a();
	} /* c1 */ /* c2 */ while (x);

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	do {
		a();
	} while (x); // blank below
</script>
