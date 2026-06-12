<script lang="ts">
	// a parenthesized arrow is callable; a bare block-bodied arrow is a complete
	// assignment expression and cannot be subscripted (see input_invalid_*)
	const a = (() => {})();
</script>
