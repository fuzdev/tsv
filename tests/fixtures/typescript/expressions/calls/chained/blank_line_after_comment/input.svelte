<script lang="ts">
	// Blank line after comments in chain (preserved)
	const a = fn()
		// comment 1
		// comment 2
		// comment 3

		.method1(arg)
		.method2(arg);

	// Blank line after single comment
	const b = fn()
		// comment

		.method1(arg)
		.method2(arg);

	// No blank line after comment (no blank line)
	const c = fn()
		// comment
		.method1(arg)
		.method2(arg);
</script>
