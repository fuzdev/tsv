<script>
	do {
		a();
	} while ( // after open paren
		x
	);

	do {
		a();
	} while ( /* after open paren */
		x
	);

	do {
		a();
	} while ( /* c1 */ /* c2 */
		x
	);

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	do {
		a();
	} while ( // blank below

		x
	);
</script>
