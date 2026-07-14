<script>
	// Line comment between `=>` and a block body is kept before the block, on its
	// own line, with the block broken to the next line - same as an expression body.
	const a = () =>
		// c
		{};

	// Non-empty block body breaks the same way.
	const b = () =>
		// c
		{
			call();
		};
</script>
