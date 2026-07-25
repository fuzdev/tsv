<script lang="ts">
	fn(() => {
		// Regex body ending in an escaped slash — the `//` is regex, not a line comment
		if (/\//.test(a)) return 'x';

		/* c1 */
		return null;
	});

	fn(() => {
		// `//` inside a character class
		if (/[//]/.test(a)) return 'x';

		/* c2 */
		return null;
	});

	fn(() => {
		// Regex body containing `/*` — not a block comment
		if (/\/*/.test(a)) return 'x';

		/* c3 */
		return null;
	});

	fn(() => {
		// Escaped slash away from the delimiters — no comment bytes in the body
		if (/a\/b/.test(a)) return 'x';

		/* c4 */
		return null;
	});
</script>
