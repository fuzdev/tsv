<script>
	// Inline block comment before object body - stays hugged
	const a = fn(() => /* comment */ ({x: 1}));

	// Inline block comment before array body - stays hugged
	const b = fn(() => /* comment */ [a, b, c]);

	// Inline block comment before nested arrow - stays hugged
	const c = fn(() => /* comment */ () => a);

	// Own-line block comment - breaks (different from inline)
	const d = fn(() =>
		/* comment */
		({x: 1}),
	);

	// Line comment - always breaks
	const e = fn(() =>
		// comment
		({x: 1}),
	);

	// Inline block comment before non-huggable body - normal break
	const f = fn(() => /* comment */ a + b);

	// Multiple inline block comments
	const g = fn(() => /* c1 */ /* c2 */ ({x: 1}));
</script>
