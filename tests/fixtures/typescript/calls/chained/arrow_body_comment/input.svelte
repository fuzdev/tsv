<script>
	// Block comment between => and body, body breaks to new line
	{
		var result =
			a === null ||
			!a.items?.every((x) => /* comment_that_is_long_enough_to_break */ aaaaaaaaaaaaa.method(x));
	}
</script>
