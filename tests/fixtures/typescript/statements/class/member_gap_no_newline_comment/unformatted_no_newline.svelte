<script>
	class A {
		// A block comment sharing its line with the next member's start leads it.
		a = 1; /* c1 */ b = 2;

		// A glued pair leads together, on the next member's line.
		c = 3; /* c2 */ /* c3 */ m1() {}

		// A comment that ends its authored line trails the previous member.
		d = 4; /* c4 */
		e = 5;

		// A glued pair whose tail is a multi-line block still leads.
		f = 6; /* c5 */ /* x
		y */ g = 7;

		// The led member starts at its first modifier.
		h = 8; /* c6 */ static m2() {}
	}
</script>
