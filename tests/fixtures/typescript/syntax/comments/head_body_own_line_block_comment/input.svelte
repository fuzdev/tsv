<script>
	// If - own-line block comment, non-block body
	if (a)
		/* b */
		fn();

	// If - own-line block comment, block body
	if (a)
	/* b */
	{
		fn();
	}

	// While - own-line block comment, non-block body
	while (a)
		/* b */
		fn();

	// For - own-line block comment, non-block body
	for (;;)
		/* b */
		fn();

	// For-of - own-line block comment, non-block body
	for (x of y)
		/* b */
		fn();

	// For-in - own-line block comment, non-block body
	for (x in y)
		/* b */
		fn();

	// For-of - own-line block comment, block body
	for (x of y)
	/* b */
	{
		fn();
	}

	// For-of - line comment then own-line block comment
	for (x of y)
		// l
		/* b */
		fn();

	// If - two own-line block comments
	if (a)
		/* b1 */
		/* b2 */
		fn();

	// Do - own-line block comment, block body
	do
	/* b */
	{
		fn();
	} while (a);

	// Else - own-line block comment, block body
	if (a) {
	} else
	/* b */
	{
		fn();
	}

	// Else - own-line block comment, non-block body
	if (a) {
	} else
		/* b */
		fn();

	// If - same-line block comment stays trailing (control)
	if (a) /* b */ fn();
</script>
