<script>
	/* Short method - stays inline (25 chars) */
	const short = {foo() {}};

	/* Long method - wraps (103 chars with indent) */
	const long = {
		methodWithAVeryVeryVeryLongNameThatDefinitelyExceedsPrintWidthBoundary0000() {
			return 1;
		},
	};
</script>
