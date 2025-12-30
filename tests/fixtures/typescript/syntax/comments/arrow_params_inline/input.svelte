<script>
	// Leading and trailing comments on params
	const a = (/* c */ a, b /* c */) => {};
	const b = (/* c */ x /* c */) => {};

	// Leading comment only on first param
	const c = (/* c */ x) => x;

	// Trailing comment only on last param
	const d = (x /* c */) => x;

	// Both outer and inner comments
	const e = /* outer */ (/* inner */ x) => x;
</script>
