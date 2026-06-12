<script>
	// Basic regex literal
	const basic = /hello/;

	// Regex with flags
	const withFlags = /world/gi;

	// Regex with character class
	const charClass = /[a-z]/;

	// Regex with escaped slash
	const escapedSlash = /a\/b/;

	// Regex with slash inside character class
	const slashInClass = /a[/]b/;

	// Regex with various escapes
	const escapes = /\d+\.\w*/;

	// Regex with anchors
	const anchors = /^start.*end$/m;

	// Regex in array
	const patterns = [/foo/, /bar/gi];

	// Division vs regex (division should still work)
	const division = 10 / 2;

	// Nested brackets in character class
	const nestedBrackets = /[[\]]/;

	// Escaped backslash
	const escapedBackslash = /a\\b/;

	// All standard flags
	const allFlags = /test/gimsuy;

	// Regex as function argument
	const match = str.match(/\d+/g);
</script>
