<script lang="ts">
	// Comment on its own line (#17723)
	if (true&&
// comment
		a.b.c?.d) {
		fn1();
	}

	// Trailing comment after operator (also preserved)
	if (true&&// comment
		a.b.c?.d) {
		fn2();
	}

	// Inline block comment between operands
	const x = a &&/* comment */b;

	// Comment in call argument (must preserve multiline structure)
	fn(a&&
// comment
		b);

	// Multiple consecutive line comments (must stay on separate lines)
	fn(a&&// comment1
// comment2
		b);
</script>
