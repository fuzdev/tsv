<script>
	// Logical expression wrapping in call args (PR #18205)
	// fn() and new Fn() wrap consistently when line exceeds print width
	fn(
		a && a_long_long_long_long_long_long_long_long_long_long_long_long_long_long_long_condition
			? consequent
			: alternate
	);

	new Fn(
		a && a_long_long_long_long_long_long_long_long_long_long_long_long_long_long_long_condition
			? consequent
			: alternate
	);
</script>
