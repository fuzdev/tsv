<script>
	// `@typeof` is not the `@type` tag (no word boundary), so the parens are
	// ordinary grouping and get stripped.
	const a = /** @typeof {A} */ x;

	// `@typeñ` ends `@type` at a non-ASCII letter, a word boundary, so it IS a
	// cast and the parens are kept.
	const b = /** @typeñ {B} */ (y);
</script>
