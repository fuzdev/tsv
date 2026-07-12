<script>
	// A keyword operator must be separated from a numeric literal — an abutting
	// IdentifierStart can never follow a number (it reads as one malformed token).
	const a = 5 in obj;
	const b = 1.5 * 2;
</script>
