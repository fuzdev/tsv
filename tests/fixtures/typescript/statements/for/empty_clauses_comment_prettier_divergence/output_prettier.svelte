<script>
	// Line comment on the same line as ( — only the comment forces a break,
	// the empty clauses stay together
	for (;;) // c
	{
		a();
	}

	// A trailing line comment on each clause forces a break after each
	for (;;) // after empty init
	// after empty test
	// before empty update
	{
		a();
	}
</script>
