<script>
	// Inline: JSDoc cast before call as arg - exactly 100 chars
	fn(/** @type {Expression} */ call(aaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc));

	// Expanded: outer and inner call both break - inner line exceeds 100
	fn(
		/** @type {Expression} */ call(
			aaaaaaaaaaaaaaaaaaa,
			bbbbbbbbbbbbbbbbbbbbbb,
			ccccccccccccccccccc,
		),
	);

	// Expanded: with preceding args - inner call line exceeds 100
	fn(
		aaaa,
		/** @type {Expression} */ call(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		),
	);

	// Expanded: JSDoc cast before ternary as call arg
	fn(
		a,
		/** @type {Expression} */ conddddd
			? aaaaaaaaaaaaaaaaaaaaaaaaaaaa
			: bbbbbbbbbbbbbbbbbbbbbbbbbbbb,
	);
</script>
