<script>
	// Arrow body comment in single-arg call (call body)
	{
		var x = fn1((value) => /* comment_long_enough_to_force_break_aaaaaaa */ transform(value));
	}

	// Arrow body comment in new expression
	{
		var x = new SomeClass((value) =>
			/* comment_long_enough_to_force_break_aaaaaaa */ transform(value)
		);
	}

	// Arrow body comment with ternary body
	{
		var x = fn2((value) =>
			/* comment_long_enough_to_force_break_aaaaaaa */ value ? aaaaaa : bbbbbb
		);
	}
</script>
