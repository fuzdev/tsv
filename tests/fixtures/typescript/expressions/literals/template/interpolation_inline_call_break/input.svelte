<script lang="ts">
	// Call breaks in inline quasi after multiline quasi - code context indent preserved
	const a = () => {
		return `prefix
text text text text text text text text text text text text text text: ${x}, ${fn(
			aaaaaaaaaa,
			bbb
		)}`;
	};

	// Deeper nesting - 3 tab code context
	const b = () => {
		if (cond) {
			return `prefix
text text text text text text text text text text text text text text: ${x}, ${fn(
				aaaaaaaaaa,
				bbb
			)}`;
		}
	};
</script>
