<script>
	// For-loop multi-declarator stays compact (uses soft break)
	for (let i = 0, len = arr.length; i < len; i++) {
		expr;
	}

	// Multiple inits also stay compact
	for (let i = 0, j = 0, k = 0; i < n; i++) {
		expr;
	}
</script>
