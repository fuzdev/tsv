<script>
	// Non-block body - the blank between the comment and the body survives
	if (a) fn();
	else
		// c1

		fn();

	// else-if - the blank survives there too
	if (a) fn();
	else
		// c2

		if (b) fn();

	// Block body - the blank is dropped so the body's { never sits below one
	if (a) fn();
	else
	// c3
	{
		fn();
	}
</script>
