<script lang="ts">
	// A block comment sharing its line with the next member's start leads it.
	interface I1 {
		a: A;
		/* c1 */ b: B;
	}

	// A glued pair leads together, even when its tail is a multi-line block.
	interface I2 {
		c: C;
		/* c2 */ /* x
		y */ d: D;
	}

	// A comment before the member's separator trails it however the lines fall.
	interface I3 {
		e: E /* c3 */;
		f: F;
	}

	// A comment that ends its authored line trails the previous member.
	interface I4 {
		g: G; /* c4 */
		h: H;
	}
</script>
