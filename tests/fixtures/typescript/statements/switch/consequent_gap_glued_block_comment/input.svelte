<script>
	switch (a) {
		// Blocks glued to each other lead the first statement together.
		case 1:
			/* block1 */ /* block2 */
			b();
			break;
		// Blocks glued to each other stay together between statements.
		case 2:
			b();
			/* block1 */ /* block2 */
			c();
			break;
		// Three glued blocks stay on one line.
		case 3:
			b();
			/* block1 */ /* block2 */ /* block3 */
			c();
			break;
		// Blocks on their own lines keep them (control).
		case 4:
			b();
			/* block1 */
			/* block2 */
			c();
			break;
	}
</script>
