<script>
	// Inline: JSDoc cast before call as arg - exactly 100 chars
	fn(/** @type {Expression} */ call(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc));

	// Expanded: outer and inner call both break - inner line exceeds 100
	fn(
		/** @type {Expression} */ call(
			aaaaaaaaaaaaaaaaaaa,
			bbbbbbbbbbbbbbbbbbbbbb,
			cccccccccccccccccccc
		)
	);

	// Expanded: with preceding args - inner call line exceeds 100
	fn(
		aaaa,
		/** @type {Expression} */ call(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		)
	);

	// Expanded: JSDoc cast before ternary as call arg
	fn(
		a,
		/** @type {Expression} */ conddddd
			? aaaaaaaaaaaaaaaaaaaaaaaaaaaa
			: bbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);
</script>
