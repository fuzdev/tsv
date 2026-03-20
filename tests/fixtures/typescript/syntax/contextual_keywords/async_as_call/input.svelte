<script>
	// async used as a function name (call expression, not async arrow)
	{
		function async(x) {
			return x;
		}

		// These are calls to `async`, not async arrow functions
		async(1);
		async(1, 2);
		async();

		// Call with member access on result
		async(1).then((x) => x);
	}
</script>
