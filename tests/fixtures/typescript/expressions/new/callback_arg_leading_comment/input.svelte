<script>
	// Expression-body arrow with leading comment, last arg is block arrow
	new A(
		/* comment */ () => 'a',
		(b) => {
			return b;
		},
	);

	// Block-body arrow with leading comment, last arg is block arrow
	new A(
		/* comment */ (a) => {
			return a;
		},
		(b) => {
			return b;
		},
	);

	// Function expression with leading comment, last arg is block arrow
	new A(
		/* comment */ function () {
			return 'a';
		},
		(b) => {
			return b;
		},
	);

	// Multiple commented callbacks before last block-body arrow
	new A(
		/* c1 */ () => 'a',
		/* c2 */ () => 'b',
		(c) => {
			return c;
		},
	);

	// Short inline control - comment stays
	new A(/* comment */ () => 'a', b);
</script>
