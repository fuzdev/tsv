<script lang="ts">
	// a comment written last inside a redundant pair down the right edge of a `case` test stays
	// after the test, ahead of a comment written after the pair, each on a line of its own, and
	// the `:` drops one level in
	switch (x) {
		case a + b: // c1
			// d1
			fn1();
			break;
		case a + b: // c2
			// d2
			fn2();
			break;
		case a + (b + c): // c3
			// d3
			fn3();
			break;
		case a + b: // c4
			// d4
			fn4();
			break;
		case a + b: // c5
			// d5
			fn5();
			break;
		// a block alone after the pair stays after the comment, in the order written
		case a + b: // c7
			/* d7 */ fn7();
			break;
		// with nothing written after the pair, the comment trails the label
		case a + b: // c6
			fn6();
	}
</script>
