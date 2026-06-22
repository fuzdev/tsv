<script>
	// Expression-body arrow with leading comment, last arg is block arrow
	fn(
		/* comment */ () => 'a',
		(b) => {
			return b;
		}
	);

	// Block-body arrow with leading comment, last arg is block arrow
	fn(
		/* comment */ (a) => {
			return a;
		},
		(b) => {
			return b;
		}
	);

	// Function expression with leading comment, last arg is block arrow
	fn(
		/* comment */ function () {
			return 'a';
		},
		(b) => {
			return b;
		}
	);

	// Multiple commented callbacks before last block-body arrow
	fn(
		/* c1 */ () => 'a',
		/* c2 */ () => 'b',
		(c) => {
			return c;
		}
	);

	// Short inline control - comment stays (no expand-last trigger)
	fn(/* comment */ () => 'a', b);
</script>
