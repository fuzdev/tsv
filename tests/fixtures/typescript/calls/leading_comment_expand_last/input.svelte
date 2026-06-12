<script>
	// Block comment before first arg, callback last arg
	fn1(/* comment */ a, () => {
		a = null;
	});

	// JSDoc block comment before first arg
	fn2(/** @type {T} */ a, () => {
		a = null;
	});
</script>
