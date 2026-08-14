<script>
	// For - glued pair below the header keeps the one line the author gave it
	for (;;)
	/* b1 */   /* b2 */
	{
		break;
	}

	// For - non-block body
	for (;;)
		/* b1 */   /* b2 */
		fn();

	// For-in - glued pair below the header
	for (const k in o)
	/* b1 */   /* b2 */
	{
		fn();
	}

	// For-of - a run ending in a line comment stays glued and still breaks
	for (const k of o)
	/* b1 */   /* b2 */ // b3
	{
		fn();
	}

	// For-of - control, comments the author gave separate lines keep them
	for (const k of o)
	/* b1 */
	/* b2 */
	{
		fn();
	}
</script>
