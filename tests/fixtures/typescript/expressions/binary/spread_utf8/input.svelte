<script lang="ts">
	// Spread operator with UTF-8 context (position + 2 boundary check)
	// Before fix: panicked when peek_chars used byte arithmetic
	const a = [1, 2, 3];
	const b = {x: 1, ...a};
	const c = '⟳'; // 3-byte UTF-8
	const d = {...b};
</script>
