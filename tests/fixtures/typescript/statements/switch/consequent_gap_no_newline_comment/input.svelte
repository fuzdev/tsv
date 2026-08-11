<script>
	switch (a) {
		// A block comment sharing its line with the next statement's start leads it.
		case 1:
			b1();
			/* c1 */ let d1 = 1;
			break;
		// A comment that ends its authored line trails the previous statement.
		case 2:
			b2(); /* c2 */
			b3();
			break;
	}
</script>
