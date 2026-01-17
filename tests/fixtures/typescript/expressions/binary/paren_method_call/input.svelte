<script>
	// Binary in template literal - should stay inline
	const a = `${(x / 1000).toFixed(y)} text1`;

	// Longer but still fits
	const b = `${(x / y).toFixed(2)} text2`;

	// Parenthesized binary as method receiver
	const c = (x + y).toString();
</script>
