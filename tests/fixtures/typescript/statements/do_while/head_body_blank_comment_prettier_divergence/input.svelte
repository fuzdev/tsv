<script>
	// Block body - the blank is dropped so the body's { never sits below one
	do
	// note1
	{
		fn();
	} while (a);

	// Non-block body - the blank between the comment and the body survives
	do
		// note2

		fn();
	while (a);
</script>
