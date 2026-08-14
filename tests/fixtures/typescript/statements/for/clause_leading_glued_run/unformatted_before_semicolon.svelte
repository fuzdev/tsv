<script>
	// A pair the author glued onto one line owns no line of its own - the header
	// still decides its own width, at each of the three clause gaps.
	for (/* a1 */ /* a2 */ x = 0; b; c++) {}

	for (
		x = 0
		/* b1 */ /* b2 */;
		b;
		c++
	) {}

	for (
		x = 0;
		b
		/* c1 */ /* c2 */;
		c++
	) {}

	// Once the header is too wide the break arrives anyway, and a run glued to its
	// clause travels with it.
	for (
		index = 0;
		/* comment one */ /* comment two */ index < collection.length && index >= 0;
		index += 1
	) {}

	// A run the author DID leave a line of its own keeps it.
	for (
		index = 0;
		/* comment one */ /* comment two */
		index < collection.length && index >= 0;
		index += 1
	) {}

	// A single own-line block owns its line and opens the header on its own.
	for (
		x = 0;
		/* c */
		b;
		c++
	) {}

	// So does a run the author ended with a line comment.
	for (
		x = 0;
		/* c1 */ // c2
		b;
		c++
	) {}
</script>
