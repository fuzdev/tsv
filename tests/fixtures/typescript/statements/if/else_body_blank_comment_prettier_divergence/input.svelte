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

	// else-if with a comment already trailing the else - flush run, blank survives
	if (a) fn();
	else // trailing1
	// note1

	if (b) fn();

	// else-if with the blank straight off the trailing comment - no own-line comment
	// between it and the body
	if (a) fn();
	else // trailing2

	if (b) fn();

	// Block body - the blank is dropped so the body's { never sits below one
	if (a) fn();
	else
	// c3
	{
		fn();
	}
</script>
