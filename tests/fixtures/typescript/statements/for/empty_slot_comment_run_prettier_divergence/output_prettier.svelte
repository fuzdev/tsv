<script>
	// Two comments in one empty slot both stay in it, separated by the slot's own
	// line — a space while the header fits
	for (a /* c1 */ /* c2 */; ; c++) fn();

	// The line spelling, which forces the header open and gives each its own line
	for (
		a;
		;
		// c3
		// c4
		c++
	) {
		fn();
	}

	// The init slot holds a run the same way
	for (; /* c5 */ /* c6 */ b; c++) fn();
</script>
