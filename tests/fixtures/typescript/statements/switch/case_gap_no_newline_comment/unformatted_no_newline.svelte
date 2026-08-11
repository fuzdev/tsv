<script>
	switch (a) {
		// A block comment sharing its line with the next case's start leads it.
		case 1:
			b1(); /* c1 */ case 2:
			b2();
			break;
		// Also from a fallthrough case's bare label.
		case 3: /* c2 */ case 4:
			b3();
			break;
		// A comment that ends its authored line trails the previous statement.
		case 5:
			b4(); /* c3 */
		case 6:
			b5();
		// A glued pair whose tail is a multi-line block still leads.
		case 7:
			b6(); /* c4 */ /* x
		y */ case 8:
			b7();
		// The same rule toward a default clause.
		case 9:
			b8(); /* c5 */ default:
			b9();
	}
</script>
