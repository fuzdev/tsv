<script>
	// Inline block comment before first arg, object last - expand-last hugs object
	const a = fn(/** @type {A} */ a, b({c: d.e}), {
		x: a,
		y: b,
	});

	// Short enough to stay inline
	fn(/** @type {A} */ a, b, {c: d});
</script>
