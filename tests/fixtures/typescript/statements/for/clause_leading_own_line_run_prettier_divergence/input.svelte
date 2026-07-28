<script>
	// An own-line block comment before a clause keeps its line and expands the
	// header; a run the author glued onto that line stays glued
	for (
		/* a1 */ /* a2 */
		x = 0;
		b;
		c++
	) {}

	for (
		x = 0;
		/* b1 */ /* b2 */
		b;
		c++
	) {}
</script>
