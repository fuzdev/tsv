<script>
	// While - glued pair below the header keeps the one line the author gave it
	while (a)
	/* b1 */ /* b2 */
	{
		fn();
	}

	// While - non-block body
	while (a)
		/* b1 */ /* b2 */
		fn();

	// While - a run ending in a line comment stays glued and still breaks
	while (a)
	/* b1 */ /* b2 */ // b3
	{
		fn();
	}

	// While - a blank inside the run survives; the pair after it stays glued
	while (a)
	/* b1 */

	/* b2 */ /* b3 */
	{
		fn();
	}

	// While - control, comments the author gave separate lines keep them
	while (a)
	/* b1 */
	/* b2 */
	{
		fn();
	}

	// Do - glued pair between the keyword and the body
	do
	/* b1 */ /* b2 */
	{
		fn();
	} while (a);

	// Do - non-block body
	do
		/* b1 */ /* b2 */
		fn();
	while (a);
</script>
