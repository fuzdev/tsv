<script>
	// Try - block comment trailing the keyword, `{` on the line the author gave it
	try /* b */
	{
		fn();
	} catch (e) {}

	// Catch with param - the same after `)`
	try {
	} catch (e) /* b */
	{
		fn();
	}

	// Bare catch - the same after the keyword
	try {
	} catch /* b */
	{
		fn();
	}

	// Finally - the same
	try {
	} finally /* b */
	{
		fn();
	}

	// Switch - the same after `)`
	switch (a) /* b */
	{
		case 1:
			break;
	}
</script>
