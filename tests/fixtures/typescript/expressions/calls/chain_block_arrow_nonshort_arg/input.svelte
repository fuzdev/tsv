<script>
	// Block-body arrow with non-short preceding arg (call with 2+ args)
	a.b(fn(a, b), () => {
		return x;
	});
	a00000000000000000000000000000.b0000000000000(fn(a, b), () => {
		return x;
	});

	// Block-body arrow with short preceding arg (should still hug)
	a.b(arg, () => {
		return x;
	});

	// Function expression with non-short preceding arg
	a.b(fn(a, b), function () {
		return x;
	});
</script>
