<script>
	// One comment on the `(` line: no gap in the run to separate
	do {} while ( /* c1 */ a + b);

	// Two on the line — the condition's head owns the last, so the run holds one
	do {} while ( /* c1 */ /* c2 */ a + b);

	// Three: the run holds two, and the one gap between them takes a single space
	do {} while (  /* c1 */ /* c2 */ /* c3 */ a + b);

	// Four: two gaps, and the separator does not accumulate across them
	do {} while ( /* c1 */ /* c2 */ /* c3 */ /* c4 */ a + b);

	// Retained clarity parens own nothing, so the run holds both comments
	do {} while ( /* c1 */ /* c2 */ (a = b));
</script>
