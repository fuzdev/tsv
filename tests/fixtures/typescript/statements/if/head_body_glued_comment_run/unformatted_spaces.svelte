<script>
	// Glued pair below the header keeps the one line the author gave it
	if (a)
	/* b1 */   /* b2 */
	{
		fn();
	}

	// Non-block body - the pair stays glued above the body
	if (a)
		/* b1 */   /* b2 */
		fn();

	// A run ending in a line comment stays glued and still breaks before the body
	if (a)
	/* b1 */   /* b2 */ // b3
	{
		fn();
	}

	// A blank inside the run survives; the pair after it stays glued
	if (a)
	/* b1 */

	/* b2 */   /* b3 */
	{
		fn();
	}

	// Control - comments the author gave separate lines keep them
	if (a)
	/* b1 */
	/* b2 */
	{
		fn();
	}

	// Control - the } before else splits a glued pair under both formatters
	if (a) {
		fn();
	}
	/* b1 */
	/* b2 */
	else {
		fn();
	}
</script>
